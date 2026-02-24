# AIMI

AI agent routing tool with Telegram and Discord bot integration.

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
aimi --prompt "explain this code"

# Start Telegram bot server (--chat-id required)
aimi --agent claude --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Start Discord bot server (--channel-id required)
aimi --agent claude --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Run multiple Telegram bots simultaneously
aimi --agent claude --routing telegram --token <TOKEN1> <TOKEN2> <TOKEN3> --chat-id <CHAT_ID>
```

## Installation

### Prerequisites

- Install the CLI tool for the agent you want to use (see [Agent Types](#agent-types))

### Build from source

```bash
# Clone
git clone https://github.com/KyongSik-Yoon/aimi.git
cd aimi

# Build
cargo build --release

# Binary location
./target/release/aimi
```

See [build_manual.md](build_manual.md) for detailed build instructions including cross-compilation.

## Agent Types

| Agent | CLI Flag | Status | Priority |
|-------|----------|--------|----------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `--agent claude` | Available | - |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | `--agent gemini` | Planned | 1st |
| [Codex CLI](https://github.com/openai/codex) | `--agent codex` | Planned | 2nd |

### Prerequisites per Agent

Each agent requires its own CLI tool to be installed:

- **Claude**: `npm install -g @anthropic-ai/claude-code`
- **Gemini**: `npm install -g @google/gemini-cli` (planned)
- **Codex**: `npm install -g @openai/codex` (planned)

### Integration Feasibility

Each agent CLI provides a non-interactive mode and structured JSON output, making subprocess integration possible.

#### Gemini CLI (Priority 1)

- **Non-interactive mode**: `gemini -p "prompt"` — identical pattern to Claude's `-p` flag
- **JSON output**: `--output-format json` (single JSON) / `--output-format stream-json` (JSONL stream)
- **JSON structure**: `{ "response": "...", "stats": {...}, "error": null }`
- **Stream-json events**: `init`, `message`, `tool_use`, `tool_result`, `error`, `result`
- **Stability**: Stable release channel (nightly → preview → stable), latest stable v0.29.x
- **Stdin piping**: Supported (`echo "text" | gemini`)
- **Auth for subprocess**: `GEMINI_API_KEY` env var (avoids interactive OAuth)
- **Limitations**: Non-interactive mode restricts tool execution (WriteFile, shell commands require `--yolo/-y`)
- **Session continuity**: Not supported in non-interactive mode (single-turn only)
- **Exit codes**: `0` (success), `1` (error), `42` (input error), `53` (turn limit exceeded)
- **Note**: `-p` flag is deprecated in favor of positional arg (`gemini "prompt"`), but still works

> Gemini CLI has the lowest integration barrier due to its CLI interface being nearly identical to Claude Code.
> Stream-json event types (`init`, `message`, `tool_use`, `tool_result`) map directly to Claude's `StreamMessage` enum.

#### Codex CLI (Priority 2)

- **Non-interactive mode**: `codex exec "prompt"` — uses `exec` subcommand instead of `-p` flag
- **JSON output**: `codex exec --json "prompt"` → JSONL event stream to stdout
- **Event types**: `thread.started`, `turn.started`, `turn.completed`, `item.*`, `error`
- **Stability**: **Alpha** (v0.105.0-alpha.16 as of 2025-02) — APIs may change without notice
- **Stdin piping**: Supported (`cat prompt.md | codex exec -`)
- **Session resume**: `codex exec resume --last "prompt"` / `codex exec resume <SESSION_ID>`
- **Extra**: `--output-schema` for schema-constrained responses

> Codex CLI has a clean subprocess interface, but its alpha status means breaking changes are likely. Will integrate after it stabilizes.

### Implementation Plan

1. **Create Agent trait** — abstract `execute_command`, `execute_command_streaming`, `resolve_binary` into a common interface
2. **Add `src/services/gemini.rs`** — implement Gemini agent with `-p` and `--output-format stream-json`
3. **Map StreamMessage** — convert Gemini's JSON events to existing `StreamMessage` enum
4. **Add `src/services/codex.rs`** — implement Codex agent with `exec --json` (after Codex stabilizes)
5. **Update routing in `main.rs`** — add `"gemini"` and `"codex"` match arms

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

**USE AT YOUR OWN RISK.**
