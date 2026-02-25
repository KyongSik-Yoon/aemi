# Gemini CLI Agent Integration Plan

## Goal

`--agent gemini`로 Gemini CLI를 통한 Telegram/Discord 라우팅 지원 추가.

## Architecture Decision: Parallel Module (No Trait Abstraction)

trait 추상화 대신 **`gemini.rs`를 `claude.rs`와 동일한 구조로 병렬 생성**하는 방식을 선택한다.

**이유:**
- `claude.rs`와 `gemini.rs`는 CLI 호출 방식, 인자 구성, 환경변수가 모두 다름
- trait으로 추상화하면 telegram.rs/discord.rs에서 dynamic dispatch가 필요해 복잡도만 증가
- `StreamMessage`, `CancelToken`만 공유 모듈로 분리하면 충분
- 향후 Codex 추가 시에도 같은 패턴으로 `codex.rs`만 추가하면 됨

## Changes (6 files)

### Step 1: `src/services/agent.rs` (NEW)

공유 타입을 `claude.rs`에서 분리.

```rust
// Move from claude.rs:
pub enum StreamMessage { Init, Text, ToolUse, ToolResult, TaskNotification, Done, Error }
pub struct CancelToken { cancelled: AtomicBool, child_pid: Mutex<Option<u32>> }
pub struct AgentResponse { success, response, session_id, error }  // rename from ClaudeResponse
```

### Step 2: `src/services/claude.rs` (MODIFY)

- `StreamMessage`, `CancelToken`, `ClaudeResponse` 정의 삭제 → `use super::agent::*`로 대체
- `ClaudeResponse` → `AgentResponse`로 rename
- 나머지 로직은 변경 없음

### Step 3: `src/services/gemini.rs` (NEW)

`claude.rs`와 동일한 구조로 Gemini CLI 래퍼 작성.

```
resolve_gemini_path()     → which gemini
get_gemini_path()         → OnceLock 캐싱
is_gemini_available()     → 바이너리 존재 확인

execute_command()         → gemini -p "prompt" --output-format json --yolo
execute_command_streaming() → gemini -p "prompt" --output-format stream-json --yolo
parse_stream_message()    → Gemini JSONL → StreamMessage 매핑
```

**Gemini CLI 인자 매핑:**

| Claude | Gemini | 비고 |
|--------|--------|------|
| `claude -p` | `gemini -p` | 동일 패턴 |
| `--output-format stream-json` | `--output-format stream-json` | 동일 |
| `--output-format json` | `--output-format json` | 동일 |
| `--allowedTools Bash,Read,...` | `--yolo` | Gemini는 tool allowlist 대신 auto-approve |
| `--append-system-prompt "..."` | stdin pipe | Gemini는 시스템 프롬프트를 프롬프트에 포함 |
| `--resume <session_id>` | (없음) | Gemini 비대화형은 single-turn only |
| `--verbose` | (불필요) | stream-json이면 자동 verbose |
| `CLAUDE_CODE_MAX_OUTPUT_TOKENS` | (불필요) | Gemini에 해당 환경변수 없음 |

**Gemini stream-json 이벤트 → StreamMessage 매핑:**

| Gemini event type | StreamMessage variant |
|---|---|
| `init` | `Init { session_id }` |
| `message` (role=assistant, type=text) | `Text { content }` |
| `tool_use` | `ToolUse { name, input }` |
| `tool_result` | `ToolResult { content, is_error }` |
| `result` | `Done { result, session_id: None }` |
| `error` | `Error { message }` |

**참고**: Gemini는 세션 유지가 없으므로 session_id는 항상 None. Init 이벤트에서 고유 ID를 생성하여 세션 추적에 사용.

### Step 4: `src/services/telegram.rs` (MODIFY)

- import 변경: `claude::{self, CancelToken, StreamMessage, ...}` → `agent::{CancelToken, StreamMessage, AgentResponse}`
- `handle_text_message()`에 agent 타입 파라미터 추가 (또는 봇 state에 agent 정보 포함)
- Claude/Gemini 분기:
  ```rust
  match agent_type {
      "claude" => claude::execute_command_streaming(...),
      "gemini" => gemini::execute_command_streaming(...),
  }
  ```
- `run_bot()` 시그니처에 `agent: &str` 파라미터 추가

### Step 5: `src/services/discord.rs` (MODIFY)

telegram.rs와 동일한 패턴의 변경.

### Step 6: `src/services/mod.rs` (MODIFY)

```rust
pub mod agent;   // NEW: shared types
pub mod claude;
pub mod gemini;  // NEW
pub mod telegram;
pub mod discord;
pub mod session;
pub mod formatter;
```

### Step 7: `src/main.rs` (MODIFY)

```rust
match agent.as_str() {
    "claude" => match routing.as_str() {
        "telegram" => handle_telegram_server(tokens, chat_id, "claude"),
        "discord" => handle_discord_server(token, channel_id, "claude"),
    },
    "gemini" => match routing.as_str() {
        "telegram" => handle_telegram_server(tokens, chat_id, "gemini"),
        "discord" => handle_discord_server(token, channel_id, "gemini"),
    },
    other => eprintln!("unsupported agent"),
}
```

- `handle_telegram_server()`, `handle_discord_server()` 시그니처에 `agent: &str` 추가
- `run_bot()` 호출 시 agent 전달

## Risk & Mitigation

| Risk | Mitigation |
|------|-----------|
| Gemini stream-json 포맷이 문서와 다를 수 있음 | debug_log로 raw JSON 로깅, 실제 테스트 시 파서 조정 |
| `-p` flag deprecation | positional arg도 지원하도록 구현 (향후 전환 용이) |
| session continuity 없음 | session_id를 자체 생성하여 aemi 레벨에서 대화 이력 관리 |
| `--yolo` 보안 리스크 | system prompt에서 보안 규칙 강제, 기존 Claude와 동일 수준 |

## Implementation Order

1. `agent.rs` 생성 (공유 타입 분리)
2. `claude.rs` 수정 (import 변경, ClaudeResponse → AgentResponse)
3. `gemini.rs` 생성
4. `mod.rs` 수정
5. `telegram.rs` 수정 (agent 파라미터 추가, 분기 추가)
6. `discord.rs` 수정 (동일)
7. `main.rs` 수정 (gemini 라우팅 추가)
