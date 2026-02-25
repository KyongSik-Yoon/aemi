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

#### Codex CLI

- **Non-interactive mode**: `codex exec "prompt"` — uses `exec` subcommand instead of `-p` flag
- **JSON output**: `codex exec --json "prompt"` → JSONL event stream to stdout
- **Event types**: `thread.started`, `turn.started`, `turn.completed`, `item.*`, `error`
- **Auto-approve**: `--full-auto` flag for non-interactive tool execution
- **Stdin piping**: Supported (`cat prompt.md | codex exec -`)
- **Session resume**: `codex exec --resume <SESSION_ID> "prompt"`
- **Extra**: `--output-schema` for schema-constrained responses

> Codex CLI uses a different event model (`item.started`/`item.completed` lifecycle) compared to Claude/Gemini,
> but maps cleanly to `StreamMessage` via `thread.started`→`Init`, `agent_message`→`Text`, `command_execution`→`ToolUse`/`ToolResult`.

#### OpenCode CLI

- **Non-interactive mode**: `opencode run "prompt"` — uses `run` subcommand
- **JSON output**: `opencode run --format json "prompt"` → JSONL event stream to stdout
- **Event types**: `step_start`, `step_finish`, `text`, `tool_use`, `reasoning`, `error`
- **Tool state tracking**: `tool_use` events include `state.status` (`pending`, `running`, `completed`, `error`)
- **Session resume**: `opencode run --session <SESSION_ID> "prompt"` / `--continue` for last session
- **Model selection**: `--model provider/model` for flexible LLM provider switching
- **File attachment**: `--file <path>` for attaching files to the prompt

> OpenCode uses a part-based event model where `tool_use` events carry state transitions (`pending`→`running`→`completed`).
> Maps to `StreamMessage` via `text`→`Text`, `tool_use(running)`→`ToolUse`, `tool_use(completed)`→`ToolResult`, `step_finish`→`Done`.

### Implementation Status

- [x] **Extract shared types** — `StreamMessage`, `CancelToken`, `AgentResponse` in `src/services/agent.rs`
- [x] **Add `src/services/gemini.rs`** — Gemini agent with `-p` and `--output-format stream-json`
- [x] **Map StreamMessage** — Gemini JSON events → `StreamMessage` enum (`message`→`Text`, `tool_use`→`ToolUse`, `result`→`Done`)
- [x] **Agent dispatch in bots** — `telegram.rs` and `discord.rs` branch on agent type
- [x] **Update routing in `main.rs`** — `--agent gemini` accepted alongside `claude`
- [x] **Add `src/services/codex.rs`** — Codex agent with `exec --json --full-auto`, session resume support
- [x] **Add `src/services/opencode.rs`** — OpenCode agent with `run --format json`, session resume support

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

**USE AT YOUR OWN RISK.**
