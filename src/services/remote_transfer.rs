use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc};
use std::io::{BufRead, BufReader};

use crate::services::file_ops::ProgressMessage;
use crate::services::remote::{RemoteAuth, RemoteProfile, SftpSession};

/// Transfer direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    LocalToRemote,
    RemoteToLocal,
}

/// Transfer configuration
#[derive(Debug, Clone)]
pub struct TransferConfig {
    pub direction: TransferDirection,
    pub profile: RemoteProfile,
    pub source_files: Vec<PathBuf>,
    pub source_base: String,
    pub target_path: String,
}

/// Check if rsync is available
fn has_rsync() -> bool {
    Command::new("rsync")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if sshpass is available (for password auth with rsync/scp)
fn has_sshpass() -> bool {
    Command::new("sshpass")
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Build SSH command option string for rsync/scp
fn build_ssh_option(profile: &RemoteProfile) -> String {
    let mut ssh_cmd = String::from("ssh");

    // Port
    if profile.port != 22 {
        ssh_cmd.push_str(&format!(" -p {}", profile.port));
    }

    // Key file
    if let RemoteAuth::KeyFile { ref path, .. } = profile.auth {
        let expanded = if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(path.trim_start_matches('~').trim_start_matches('/'))
                    .display()
                    .to_string()
            } else {
                path.clone()
            }
        } else {
            path.clone()
        };
        ssh_cmd.push_str(&format!(" -i '{}'", expanded));
    }

    // Disable strict host key checking for convenience
    ssh_cmd.push_str(" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR");

    ssh_cmd
}

/// Build remote path string for rsync/scp: user@host:/path
fn build_remote_spec(profile: &RemoteProfile, path: &str) -> String {
    format!("{}@{}:{}", profile.user, profile.host, path)
}

/// Wrap command with sshpass if needed for password auth
fn wrap_with_sshpass(mut cmd: Command, profile: &RemoteProfile) -> Command {
    if let RemoteAuth::Password { ref password } = profile.auth {
        if has_sshpass() {
            let mut sshpass_cmd = Command::new("sshpass");
            sshpass_cmd.arg("-p").arg(password);
            // Reconstruct: sshpass -p PASSWORD original_command args...
            let program = cmd.get_program().to_string_lossy().to_string();
            let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
            sshpass_cmd.arg(program);
            for arg in args {
                sshpass_cmd.arg(arg);
            }
            sshpass_cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            return sshpass_cmd;
        }
    }
    cmd
}

/// Transfer files using rsync with progress reporting
fn transfer_rsync(
    config: &TransferConfig,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    let ssh_option = build_ssh_option(&config.profile);

    for source_file in &config.source_files {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(ProgressMessage::Completed(0, 0));
            return Ok(());
        }

        let file_name = source_file.display().to_string();
        let _ = tx.send(ProgressMessage::FileStarted(file_name.clone()));

        let source_full = format!("{}/{}", config.source_base.trim_end_matches('/'), source_file.display());
        let target = &config.target_path;

        let (src, dst) = match config.direction {
            TransferDirection::LocalToRemote => {
                (source_full, build_remote_spec(&config.profile, target))
            }
            TransferDirection::RemoteToLocal => {
                (build_remote_spec(&config.profile, &source_full), target.clone())
            }
        };

        let mut cmd = Command::new("rsync");
        cmd.arg("-avz")
            .arg("--info=progress2")
            .arg("--no-inc-recursive")
            .arg("-e")
            .arg(&ssh_option)
            .arg(&src)
            .arg(&dst)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut cmd = wrap_with_sshpass(cmd, &config.profile);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to start rsync: {}", e))?;

        // Parse rsync progress output
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    let _ = child.wait(); // Reap zombie process
                    let _ = tx.send(ProgressMessage::Completed(0, 0));
                    return Ok(());
                }

                if let Ok(line) = line {
                    // rsync --info=progress2 output format: "  1,234,567  42%  1.23MB/s  0:01:23"
                    if let Some(progress) = parse_rsync_progress(&line) {
                        let _ = tx.send(ProgressMessage::FileProgress(progress.0, progress.1));
                    }
                }
            }
        }

        let status = child.wait().map_err(|e| format!("rsync wait failed: {}", e))?;

        if status.success() {
            let _ = tx.send(ProgressMessage::FileCompleted(file_name));
        } else {
            // Try to capture stderr
            let stderr_msg = if let Some(mut stderr) = child.stderr.take() {
                let mut buf = String::new();
                let _ = std::io::Read::read_to_string(&mut stderr, &mut buf);
                buf
            } else {
                format!("rsync exited with code {}", status.code().unwrap_or(-1))
            };
            let _ = tx.send(ProgressMessage::Error(file_name, stderr_msg.clone()));
            return Err(stderr_msg);
        }
    }

    Ok(())
}

/// Parse rsync --info=progress2 output line
/// Returns (transferred_bytes, total_bytes) if parseable
fn parse_rsync_progress(line: &str) -> Option<(u64, u64)> {
    // Format: "  1,234,567  42%  1.23MB/s  0:01:23"
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 2 {
        // First part: bytes transferred (with commas)
        let bytes_str = parts[0].replace(',', "");
        let transferred: u64 = bytes_str.parse().ok()?;

        // Second part: percentage
        let pct_str = parts[1].trim_end_matches('%');
        let pct: u64 = pct_str.parse().ok()?;

        if pct > 0 {
            let total = transferred * 100 / pct;
            return Some((transferred, total));
        } else if transferred > 0 {
            return Some((0, transferred));
        }
    }
    None
}

/// Transfer files using scp (fallback when rsync not available)
fn transfer_scp(
    config: &TransferConfig,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    for source_file in &config.source_files {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(ProgressMessage::Completed(0, 0));
            return Ok(());
        }

        let file_name = source_file.display().to_string();
        let _ = tx.send(ProgressMessage::FileStarted(file_name.clone()));

        let source_full = format!("{}/{}", config.source_base.trim_end_matches('/'), source_file.display());
        let target = &config.target_path;

        let (src, dst) = match config.direction {
            TransferDirection::LocalToRemote => {
                (source_full, build_remote_spec(&config.profile, target))
            }
            TransferDirection::RemoteToLocal => {
                (build_remote_spec(&config.profile, &source_full), target.clone())
            }
        };

        let mut cmd = Command::new("scp");
        cmd.arg("-r"); // Recursive for directories

        // Port
        if config.profile.port != 22 {
            cmd.arg("-P").arg(config.profile.port.to_string());
        }

        // Key file
        if let RemoteAuth::KeyFile { ref path, .. } = config.profile.auth {
            let expanded = if path.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    home.join(path.trim_start_matches('~').trim_start_matches('/'))
                        .display()
                        .to_string()
                } else {
                    path.clone()
                }
            } else {
                path.clone()
            };
            cmd.arg("-i").arg(expanded);
        }

        cmd.arg("-o").arg("StrictHostKeyChecking=no")
            .arg("-o").arg("UserKnownHostsFile=/dev/null")
            .arg("-o").arg("LogLevel=ERROR")
            .arg(&src)
            .arg(&dst)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut cmd = wrap_with_sshpass(cmd, &config.profile);

        let output = cmd.output().map_err(|e| format!("Failed to start scp: {}", e))?;

        if output.status.success() {
            let _ = tx.send(ProgressMessage::FileCompleted(file_name));
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = tx.send(ProgressMessage::Error(file_name.clone(), stderr.clone()));
            return Err(format!("scp failed for '{}': {}", file_name, stderr));
        }
    }

    Ok(())
}

/// Transfer files using SFTP (fallback when sshpass not available for password auth)
fn transfer_sftp(
    config: &TransferConfig,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    // Create a new SFTP session for this transfer
    let session = SftpSession::connect(&config.profile)
        .map_err(|e| format!("SFTP connection failed: {}", e))?;

    for source_file in &config.source_files {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(ProgressMessage::Completed(0, 0));
            return Ok(());
        }

        let file_name = source_file.display().to_string();
        let _ = tx.send(ProgressMessage::FileStarted(file_name.clone()));

        let source_full = format!("{}/{}", config.source_base.trim_end_matches('/'), source_file.display());
        let target = &config.target_path;

        match config.direction {
            TransferDirection::RemoteToLocal => {
                let target_path = format!("{}/{}", target.trim_end_matches('/'), source_file.display());

                if let Some(parent) = std::path::Path::new(&target_path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                // Check if source is a directory on remote
                match session.list_dir(&source_full) {
                    Ok(_) => {
                        let _ = std::fs::create_dir_all(&target_path);
                        sftp_download_dir_recursive(&session, &source_full, &target_path, cancel_flag, tx)?;
                    }
                    Err(_) => {
                        session.download_file(&source_full, &target_path)?;
                    }
                }
            }
            TransferDirection::LocalToRemote => {
                let target_path = format!("{}/{}", target.trim_end_matches('/'), source_file.display());
                let local_path = std::path::Path::new(&source_full);

                if local_path.is_dir() {
                    let _ = session.mkdir(&target_path);
                    sftp_upload_dir_recursive(&session, local_path, &target_path, cancel_flag, tx)?;
                } else {
                    session.upload_file(&source_full, &target_path)?;
                }
            }
        }

        let _ = tx.send(ProgressMessage::FileCompleted(file_name));
    }

    Ok(())
}

/// Recursively download a remote directory via SFTP
fn sftp_download_dir_recursive(
    session: &SftpSession,
    remote_dir: &str,
    local_dir: &str,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    let entries = session.list_dir(remote_dir)
        .map_err(|e| format!("Failed to list '{}': {}", remote_dir, e))?;

    for entry in entries {
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        let remote_path = format!("{}/{}", remote_dir.trim_end_matches('/'), entry.name);
        let local_path = format!("{}/{}", local_dir.trim_end_matches('/'), entry.name);

        if entry.is_directory {
            let _ = std::fs::create_dir_all(&local_path);
            sftp_download_dir_recursive(session, &remote_path, &local_path, cancel_flag, tx)?;
        } else {
            session.download_file(&remote_path, &local_path)?;
        }
    }

    Ok(())
}

/// Recursively upload a local directory via SFTP
fn sftp_upload_dir_recursive(
    session: &SftpSession,
    local_dir: &std::path::Path,
    remote_dir: &str,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ProgressMessage>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(local_dir)
        .map_err(|e| format!("Failed to read dir '{}': {}", local_dir.display(), e))?;

    for entry in entries {
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let local_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let remote_path = format!("{}/{}", remote_dir.trim_end_matches('/'), name);

        if local_path.is_dir() {
            let _ = session.mkdir(&remote_path);
            sftp_upload_dir_recursive(session, &local_path, &remote_path, cancel_flag, tx)?;
        } else {
            session.upload_file(&local_path.to_string_lossy().as_ref(), &remote_path)?;
        }
    }

    Ok(())
}

/// Main transfer function - tries rsync first, falls back to scp, then SFTP
pub fn transfer_files_with_progress(
    config: TransferConfig,
    cancel_flag: Arc<AtomicBool>,
    tx: Sender<ProgressMessage>,
) {
    let total_files = config.source_files.len();

    let _ = tx.send(ProgressMessage::Preparing(format!(
        "Transferring {} file(s)...",
        total_files
    )));

    // Check if we need sshpass for external tools (rsync/scp)
    let needs_sshpass = matches!(&config.profile.auth, RemoteAuth::Password { .. });
    let has_sshpass = needs_sshpass && has_sshpass();
    let use_external_tools = !needs_sshpass || has_sshpass;

    let _ = tx.send(ProgressMessage::PrepareComplete);

    let result = if use_external_tools {
        // Can use rsync/scp (either key auth or password with sshpass)
        if has_rsync() {
            transfer_rsync(&config, &cancel_flag, &tx)
        } else {
            transfer_scp(&config, &cancel_flag, &tx)
        }
    } else {
        // Password auth without sshpass — fall back to SFTP
        transfer_sftp(&config, &cancel_flag, &tx)
    };

    match result {
        Ok(_) => {
            let _ = tx.send(ProgressMessage::Completed(total_files, 0));
        }
        Err(ref msg) => {
            // Ensure error is sent via channel (some paths may not send it)
            let _ = tx.send(ProgressMessage::Error(String::new(), msg.clone()));
            let _ = tx.send(ProgressMessage::Completed(0, total_files));
        }
    }
}
