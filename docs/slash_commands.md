# Slash Commands

List of slash commands available in the aemi bot (Discord/Telegram).

## Session

| Command | Description |
|---------|-------------|
| `/start <path>` | Start a session in the specified directory (e.g., `/start ~/project`) |
| `/start` | Start a session with an auto-generated workspace in `~/.aemi/workspace` |
| `/resume` | Show saved session list (up to 10 most recent) |
| `/resume <number>` | Resume a session by number (e.g., `/resume 3`) |
| `/pwd` | Show the working directory of the current session |
| `/clear` | Clear AI conversation history and cancel any in-progress requests |
| `/stop` | Stop the currently running AI request |

## File Transfer

| Command | Description |
|---------|-------------|
| `/down <file>` | Download a file from the server (e.g., `/down ./output.log`) |
| File upload | Send a file directly to the bot to upload it to the current session directory |

The `/down` command supports both absolute and relative paths. Relative paths are resolved from the current session directory.

## Shell

| Command | Description |
|---------|-------------|
| `!<command>` | Execute a shell command directly (e.g., `!ls -la`, `!git status`) |

Use the `!` prefix to run shell commands directly in the current session directory.

## AI Chat

Any message that does not start with a slash command or `!` is sent to the AI agent. The AI can read, modify files, and execute commands within the session directory.

## Agent

| Command | Description |
|---------|-------------|
| `/agent` | Show the current AI agent and list available agents |
| `/agent <name>` | Switch to a different AI agent |

Available agents: `claude`, `gemini`, `codex`, `opencode`

## Tool Management

| Command | Description |
|---------|-------------|
| `/availabletools` | List all available AI tools |
| `/allowedtools` | List currently allowed tools |
| `/allowed +name` | Add a tool (e.g., `/allowed +Bash`) |
| `/allowed -name` | Remove a tool (e.g., `/allowed -Bash`) |

## Help

| Command | Description |
|---------|-------------|
| `/help` | Show help |
