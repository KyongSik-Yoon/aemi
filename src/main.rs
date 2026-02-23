mod services;

use std::env;

use crate::services::claude;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!("aimi {} - LLM CLI routing tool", VERSION);
    println!();
    println!("USAGE:");
    println!("    aimi [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help              Print help information");
    println!("    -v, --version           Print version information");
    println!("    --prompt <TEXT>         Send prompt to AI and print response");
    println!("    --base64 <TEXT>         Decode base64 and print (internal use)");
    println!("    --ccserver <TOKEN>... [--chat-id <ID>]");
    println!("                            Start Telegram bot server(s)");
    println!("                            --chat-id restricts access to a specific Telegram chat ID");
    println!("    --ccserver-discord <TOKEN> [--channel-id <ID>]");
    println!("                            Start Discord bot server");
    println!("                            --channel-id restricts access to a specific Discord channel");
    println!("    --sendfile <PATH> --chat <ID> --key <HASH>");
    println!("                            Send file via Telegram bot (internal use, HASH = token hash)");
}

fn print_version() {
    println!("aimi {}", VERSION);
}

fn handle_base64(encoded: &str) {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    match BASE64.decode(encoded) {
        Ok(decoded) => {
            if let Ok(text) = String::from_utf8(decoded) {
                print!("{}", text);
            } else {
                std::process::exit(1);
            }
        }
        Err(_) => {
            std::process::exit(1);
        }
    }
}

fn handle_sendfile(path: &str, chat_id: i64, hash_key: &str) {
    use teloxide::prelude::*;
    use crate::services::telegram::resolve_token_by_hash;
    let token = match resolve_token_by_hash(hash_key) {
        Some(t) => t,
        None => {
            eprintln!("Error: no bot token found for hash key: {}", hash_key);
            std::process::exit(1);
        }
    };
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        let bot = Bot::new(&token);
        let file_path = std::path::Path::new(path);
        if !file_path.exists() {
            eprintln!("Error: file not found: {}", path);
            std::process::exit(1);
        }
        match bot.send_document(
            ChatId(chat_id),
            teloxide::types::InputFile::file(file_path),
        ).await {
            Ok(_) => println!("File sent: {}", path),
            Err(e) => {
                eprintln!("Failed to send file: {}", e);
                std::process::exit(1);
            }
        }
    });
}

fn handle_prompt(prompt: &str) {
    // Check if Claude is available
    if !claude::is_claude_available() {
        eprintln!("Error: Claude CLI is not available.");
        eprintln!("Please install Claude CLI: https://claude.ai/cli");
        return;
    }

    // Execute Claude command
    let current_dir = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let response = claude::execute_command(prompt, None, &current_dir, None);

    if !response.success {
        eprintln!("Error: {}", response.error.unwrap_or_else(|| "Unknown error".to_string()));
        return;
    }

    let content = response.response.unwrap_or_default();
    print!("{}", content);
    if !content.ends_with('\n') {
        println!();
    }
}

fn handle_ccserver(tokens: Vec<String>, allowed_chat_id: Option<i64>) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let title = format!("  aimi v{}  |  Telegram Bot Server  ", VERSION);
    let width = title.chars().count();
    println!();
    println!("  ┌{}┐", "─".repeat(width));
    println!("  │{}│", title);
    println!("  └{}┘", "─".repeat(width));
    println!();

    if let Some(cid) = allowed_chat_id {
        println!("  ▸ Chat ID filter : {}", cid);
    }

    if tokens.len() == 1 {
        println!("  ▸ Bot instance : 1");
        println!("  ▸ Status       : Connecting...");
        println!();
        rt.block_on(services::telegram::run_bot(&tokens[0], allowed_chat_id));
    } else {
        println!("  ▸ Bot instances : {}", tokens.len());
        println!("  ▸ Status        : Connecting...");
        println!();
        rt.block_on(async {
            let mut handles = Vec::new();
            for (i, token) in tokens.into_iter().enumerate() {
                let chat_id = allowed_chat_id;
                handles.push(tokio::spawn(async move {
                    println!("  ✓ Bot #{} connected", i + 1);
                    services::telegram::run_bot(&token, chat_id).await;
                }));
            }
            for handle in handles {
                let _ = handle.await;
            }
        });
    }
}

fn handle_ccserver_discord(token: String, allowed_channel_id: Option<u64>) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let title = format!("  aimi v{}  |  Discord Bot Server  ", VERSION);
    let width = title.chars().count();
    println!();
    println!("  ┌{}┐", "─".repeat(width));
    println!("  │{}│", title);
    println!("  └{}┘", "─".repeat(width));
    println!();

    if let Some(cid) = allowed_channel_id {
        println!("  ▸ Channel ID filter : {}", cid);
    }

    println!("  ▸ Bot instance : 1");
    println!("  ▸ Status       : Connecting...");
    println!();
    rt.block_on(services::discord::run_bot(&token, allowed_channel_id));
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                print_version();
                return;
            }
            "--prompt" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --prompt requires a text argument");
                    eprintln!("Usage: aimi --prompt \"your question\"");
                    return;
                }
                handle_prompt(&args[i + 1]);
                return;
            }
            "--base64" => {
                if i + 1 >= args.len() {
                    std::process::exit(1);
                }
                handle_base64(&args[i + 1]);
                return;
            }
            "--ccserver" => {
                let mut tokens: Vec<String> = Vec::new();
                let mut allowed_chat_id: Option<i64> = None;
                let mut j = i + 1;
                while j < args.len() {
                    match args[j].as_str() {
                        "--chat-id" => {
                            if j + 1 < args.len() {
                                allowed_chat_id = args[j + 1].parse().ok();
                                if allowed_chat_id.is_none() {
                                    eprintln!("Error: --chat-id value must be a valid integer");
                                    return;
                                }
                                j += 2;
                            } else {
                                eprintln!("Error: --chat-id requires a value");
                                return;
                            }
                        }
                        arg if arg.starts_with('-') => {
                            j += 1;
                        }
                        _ => {
                            tokens.push(args[j].clone());
                            j += 1;
                        }
                    }
                }
                if tokens.is_empty() {
                    eprintln!("Error: --ccserver requires at least one token argument");
                    eprintln!("Usage: aimi --ccserver <TOKEN> [--chat-id <ID>]");
                    return;
                }
                handle_ccserver(tokens, allowed_chat_id);
                return;
            }
            "--ccserver-discord" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --ccserver-discord requires a token argument");
                    eprintln!("Usage: aimi --ccserver-discord <TOKEN> [--channel-id <ID>]");
                    return;
                }
                let token = args[i + 1].clone();
                let mut allowed_channel_id: Option<u64> = None;
                let mut j = i + 2;
                while j < args.len() {
                    match args[j].as_str() {
                        "--channel-id" => {
                            if j + 1 < args.len() {
                                allowed_channel_id = args[j + 1].parse().ok();
                                if allowed_channel_id.is_none() {
                                    eprintln!("Error: --channel-id value must be a valid integer");
                                    return;
                                }
                                j += 2;
                            } else {
                                eprintln!("Error: --channel-id requires a value");
                                return;
                            }
                        }
                        _ => { j += 1; }
                    }
                }
                handle_ccserver_discord(token, allowed_channel_id);
                return;
            }
            "--sendfile" => {
                // Parse: --sendfile <PATH> --chat <ID> --key <TOKEN>
                let mut file_path: Option<String> = None;
                let mut chat_id: Option<i64> = None;
                let mut key: Option<String> = None;
                let mut j = i + 1;
                while j < args.len() {
                    match args[j].as_str() {
                        "--chat" => {
                            if j + 1 < args.len() {
                                chat_id = args[j + 1].parse().ok();
                                j += 2;
                            } else {
                                j += 1;
                            }
                        }
                        "--key" => {
                            if j + 1 < args.len() {
                                key = Some(args[j + 1].clone());
                                j += 2;
                            } else {
                                j += 1;
                            }
                        }
                        _ if file_path.is_none() && !args[j].starts_with("--") => {
                            file_path = Some(args[j].clone());
                            j += 1;
                        }
                        _ => { j += 1; }
                    }
                }
                match (file_path, chat_id, key) {
                    (Some(fp), Some(cid), Some(k)) => {
                        handle_sendfile(&fp, cid, &k);
                    }
                    _ => {
                        eprintln!("Error: --sendfile requires <PATH>, --chat <ID>, and --key <HASH>");
                        eprintln!("Usage: aimi --sendfile <PATH> --chat <ID> --key <HASH>");
                    }
                }
                return;
            }
            arg => {
                eprintln!("Unknown option: {}", arg);
                eprintln!("Use --help for usage information");
                return;
            }
        }
    }

    // No arguments provided
    print_help();
}
