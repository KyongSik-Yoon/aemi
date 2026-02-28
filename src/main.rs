mod services;

use std::env;

use crate::services::claude;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!("aemi {} - Agent Mirror", VERSION);
    println!();
    println!("USAGE:");
    println!("    aemi [OPTIONS]");
    println!("    aemi --agent <AGENT> --routing <PLATFORM> --token <TOKEN>... --chat-id|--channel-id <ID>");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help              Print help information");
    println!("    -v, --version           Print version information");
    println!("    --prompt <TEXT>         Send prompt to AI and print response");
    println!();
    println!("SERVER MODE:");
    println!("    --agent <AGENT>         AI agent to use (claude, gemini, codex, opencode, oh-my-pi)");
    println!("    --routing <PLATFORM>    Messaging platform (telegram, discord)");
    println!("    --token <TOKEN>...      Bot token(s). Telegram supports multiple tokens");
    println!("    --chat-id <ID>          Telegram chat ID (required for telegram routing)");
    println!("    --channel-id <ID>       Discord channel ID (required for discord routing)");
    println!();
    println!("EXAMPLES:");
    println!("    aemi --agent claude --routing telegram --token <TOKEN> --chat-id <ID>");
    println!("    aemi --agent claude --routing telegram --token <T1> <T2> --chat-id <ID>");
    println!("    aemi --agent claude --routing discord --token <TOKEN> --channel-id <ID>");
    println!();
    println!("INTERNAL:");
    println!("    --base64 <TEXT>         Decode base64 and print");
    println!("    --sendfile <PATH> --chat <ID> --key <HASH>");
    println!("                            Send file via Telegram bot (HASH = token hash)");
}

fn print_version() {
    println!("aemi {}", VERSION);
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

fn handle_telegram_server(tokens: Vec<String>, allowed_chat_id: i64, agent: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let title = format!("  aemi v{}  |  Telegram Bot Server  ", VERSION);
    let width = title.chars().count();
    println!();
    println!("  ┌{}┐", "─".repeat(width));
    println!("  │{}│", title);
    println!("  └{}┘", "─".repeat(width));
    println!();

    println!("  ▸ Agent        : {}", agent);
    println!("  ▸ Chat ID filter : {}", allowed_chat_id);

    if tokens.len() == 1 {
        println!("  ▸ Bot instance : 1");
        println!("  ▸ Status       : Connecting...");
        println!();
        rt.block_on(services::telegram::run_bot(&tokens[0], Some(allowed_chat_id), agent));
    } else {
        println!("  ▸ Bot instances : {}", tokens.len());
        println!("  ▸ Status        : Connecting...");
        println!();
        let agent_owned = agent.to_string();
        rt.block_on(async {
            let mut handles = Vec::new();
            for (i, token) in tokens.into_iter().enumerate() {
                let chat_id = allowed_chat_id;
                let agent = agent_owned.clone();
                handles.push(tokio::spawn(async move {
                    println!("  ✓ Bot #{} connected", i + 1);
                    services::telegram::run_bot(&token, Some(chat_id), &agent).await;
                }));
            }
            for handle in handles {
                let _ = handle.await;
            }
        });
    }
}

fn handle_discord_server(token: String, allowed_channel_id: u64, agent: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let title = format!("  aemi v{}  |  Discord Bot Server  ", VERSION);
    let width = title.chars().count();
    println!();
    println!("  ┌{}┐", "─".repeat(width));
    println!("  │{}│", title);
    println!("  └{}┘", "─".repeat(width));
    println!();

    println!("  ▸ Agent          : {}", agent);
    println!("  ▸ Channel ID filter : {}", allowed_channel_id);

    println!("  ▸ Bot instance : 1");
    println!("  ▸ Status       : Connecting...");
    println!();
    rt.block_on(services::discord::run_bot(&token, Some(allowed_channel_id), agent));
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        print_help();
        return;
    }

    // Handle standalone commands first
    match args[1].as_str() {
        "-h" | "--help" => {
            print_help();
            return;
        }
        "-v" | "--version" => {
            print_version();
            return;
        }
        "--prompt" => {
            if args.len() < 3 {
                eprintln!("Error: --prompt requires a text argument");
                eprintln!("Usage: aemi --prompt \"your question\"");
                return;
            }
            handle_prompt(&args[2]);
            return;
        }
        "--base64" => {
            if args.len() < 3 {
                std::process::exit(1);
            }
            handle_base64(&args[2]);
            return;
        }
        "--sendfile" => {
            // Parse: --sendfile <PATH> --chat <ID> --key <TOKEN>
            let mut file_path: Option<String> = None;
            let mut chat_id: Option<i64> = None;
            let mut key: Option<String> = None;
            let mut j = 2;
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
                    eprintln!("Usage: aemi --sendfile <PATH> --chat <ID> --key <HASH>");
                }
            }
            return;
        }
        _ => {}
    }

    // Parse server mode: --agent <AGENT> --routing <PLATFORM> --token <TOKEN>... [OPTIONS]
    let mut agent: Option<String> = None;
    let mut routing: Option<String> = None;
    let mut tokens: Vec<String> = Vec::new();
    let mut chat_id: Option<i64> = None;
    let mut channel_id: Option<u64> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--agent" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --agent requires a value (e.g., claude)");
                    return;
                }
                agent = Some(args[i + 1].clone());
                i += 2;
            }
            "--routing" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --routing requires a value (e.g., telegram, discord)");
                    return;
                }
                routing = Some(args[i + 1].clone());
                i += 2;
            }
            "--token" => {
                i += 1;
                while i < args.len() && !args[i].starts_with('-') {
                    tokens.push(args[i].clone());
                    i += 1;
                }
                if tokens.is_empty() {
                    eprintln!("Error: --token requires at least one value");
                    return;
                }
            }
            "--chat-id" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --chat-id requires a value");
                    return;
                }
                chat_id = args[i + 1].parse().ok();
                if chat_id.is_none() {
                    eprintln!("Error: --chat-id value must be a valid integer");
                    return;
                }
                i += 2;
            }
            "--channel-id" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --channel-id requires a value");
                    return;
                }
                channel_id = args[i + 1].parse().ok();
                if channel_id.is_none() {
                    eprintln!("Error: --channel-id value must be a valid integer");
                    return;
                }
                i += 2;
            }
            arg => {
                eprintln!("Unknown option: {}", arg);
                eprintln!("Use --help for usage information");
                return;
            }
        }
    }

    // Validate required server mode flags
    let agent = match agent {
        Some(a) => a,
        None => {
            eprintln!("Error: --agent is required");
            eprintln!("Usage: aemi --agent claude --routing telegram --token <TOKEN>");
            return;
        }
    };
    let routing = match routing {
        Some(r) => r,
        None => {
            eprintln!("Error: --routing is required");
            eprintln!("Usage: aemi --agent claude --routing telegram --token <TOKEN>");
            return;
        }
    };
    if tokens.is_empty() {
        eprintln!("Error: --token is required");
        eprintln!("Usage: aemi --agent {} --routing {} --token <TOKEN>", agent, routing);
        return;
    }

    // Dispatch based on agent and routing
    match agent.as_str() {
        "claude" | "gemini" | "codex" | "opencode" | "oh-my-pi" => match routing.as_str() {
            "telegram" => {
                let chat_id = match chat_id {
                    Some(id) => id,
                    None => {
                        eprintln!("Error: --chat-id is required for Telegram routing (security)");
                        eprintln!("Usage: aemi --agent {} --routing telegram --token <TOKEN> --chat-id <ID>", agent);
                        return;
                    }
                };
                handle_telegram_server(tokens, chat_id, &agent);
            }
            "discord" => {
                if tokens.len() > 1 {
                    eprintln!("Error: Discord supports only one token");
                    return;
                }
                let channel_id = match channel_id {
                    Some(id) => id,
                    None => {
                        eprintln!("Error: --channel-id is required for Discord routing (security)");
                        eprintln!("Usage: aemi --agent {} --routing discord --token <TOKEN> --channel-id <ID>", agent);
                        return;
                    }
                };
                handle_discord_server(tokens.into_iter().next().unwrap(), channel_id, &agent);
            }
            other => {
                eprintln!("Error: unsupported routing '{}'. Supported: telegram, discord", other);
            }
        },
        other => {
            eprintln!("Error: unsupported agent '{}'. Supported: claude, gemini, codex, opencode, oh-my-pi", other);
        }
    }
}
