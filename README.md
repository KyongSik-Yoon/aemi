# AEMI

Agent Mirror - AI agent routing tool with Telegram and Discord bot integration.

A CLI tool that relays AI agent responses through Telegram/Discord bots.

## Origin

This project is a fork of [kstost/cokacdir](https://github.com/kstost/cokacdir). It is based on the LLM CLI routing portion of the original project, with the TUI file manager removed to focus on bot relay functionality.

## Features

- **AI Agent Routing**: Query AI agents and receive responses via `--prompt`
- **Telegram Bot**: Route AI agent through Telegram with `--routing telegram`
- **Discord Bot**: Route AI agent through Discord with `--routing discord`
- **Multi-Bot**: Run multiple Telegram bot tokens simultaneously
- **Access Control**: `--chat-id` (Telegram) / `--channel-id` (Discord) required for routing

## Usage

```bash
# Query Claude Code directly
aemi --prompt "explain this code"

# Start Telegram bot server with Claude (--chat-id required)
aemi --agent claude --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Start Telegram bot server with Gemini
aemi --agent gemini --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Start Discord bot server (--channel-id required)
aemi --agent claude --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Start Telegram bot server with Codex
aemi --agent codex --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Start Discord bot server with Gemini
aemi --agent gemini --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Start Discord bot server with Codex
aemi --agent codex --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Start Telegram bot server with OpenCode
aemi --agent opencode --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Start Discord bot server with OpenCode
aemi --agent opencode --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Run multiple Telegram bots simultaneously
aemi --agent claude --routing telegram --token <TOKEN1> <TOKEN2> <TOKEN3> --chat-id <CHAT_ID>
```

## Installation

### Prerequisites

- Install the CLI tool for the agent you want to use (see [Agent Types](#agent-types))

### Build from source

```bash
# Clone
git clone https://github.com/KyongSik-Yoon/aemi.git
cd aemi

# Build
cargo build --release

# Binary location
./target/release/aemi
```

See [build_manual.md](build_manual.md) for detailed build instructions including cross-compilation.

## Agent Types

| Agent | CLI Flag | Status | Priority |
|-------|----------|--------|----------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `--agent claude` | Available | - |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | `--agent gemini` | Available | - |
| [Codex CLI](https://github.com/openai/codex) | `--agent codex` | Available | - |
| [OpenCode](https://opencode.ai) | `--agent opencode` | Available | - |

### Prerequisites per Agent

Each agent requires its own CLI tool to be installed:

- **Claude**: `npm install -g @anthropic-ai/claude-code`
- **Gemini**: `npm install -g @google/gemini-cli`
- **Codex**: `npm install -g @openai/codex`
- **OpenCode**: `npm install -g opencode` (or see [opencode.ai](https://opencode.ai/docs/) for other methods)

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

**USE AT YOUR OWN RISK.**
