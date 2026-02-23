# AIMI

LLM CLI routing tool with Telegram and Discord bot integration.

A CLI tool that relays Claude Code responses through Telegram/Discord bots.

## Origin

This project is a fork of [kstost/aimi](https://github.com/kstost/aimi). It is based on the LLM CLI routing portion of the original project, with the TUI file manager removed to focus on bot relay functionality.

## Features

- **Claude Code Routing**: Query Claude Code and receive responses via `--prompt`
- **Telegram Bot**: Run a Telegram bot server with `--ccserver` to use Claude Code through chat
- **Discord Bot**: Run a Discord bot server with `--ccserver-discord` to use Claude Code through chat
- **Multi-Bot**: Run multiple Telegram bot tokens simultaneously
- **Access Control**: Restrict access to specific chats/channels with `--chat-id` / `--channel-id`

## Usage

```bash
# Query Claude Code directly
aimi --prompt "explain this code"

# Start Telegram bot server
aimi --ccserver <TELEGRAM_BOT_TOKEN>

# Telegram bot with chat restriction
aimi --ccserver <TOKEN> --chat-id <CHAT_ID>

# Start Discord bot server
aimi --ccserver-discord <DISCORD_BOT_TOKEN>

# Discord bot with channel restriction
aimi --ccserver-discord <TOKEN> --channel-id <CHANNEL_ID>

# Run multiple Telegram bots simultaneously
aimi --ccserver <TOKEN1> <TOKEN2> <TOKEN3>
```

## Installation

### Prerequisites

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) must be installed

```bash
npm install -g @anthropic-ai/claude-code
```

### Build from source

```bash
# Clone
git clone https://github.com/KyongSik-Yoon/cokacdir.git
cd cokacdir

# Build
cargo build --release

# Binary location
./target/release/aimi
```

See [build_manual.md](build_manual.md) for detailed build instructions including cross-compilation.

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

**USE AT YOUR OWN RISK.**
