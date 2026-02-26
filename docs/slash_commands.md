# Slash Commands

aemi 봇(Discord/Telegram)에서 사용할 수 있는 슬래시 커맨드 목록입니다.

## Session

| 커맨드 | 설명 |
|--------|------|
| `/start <path>` | 지정한 디렉토리에서 세션 시작 (예: `/start ~/project`) |
| `/start` | `~/.aemi/workspace`에 임의의 워크스페이스를 자동 생성하여 세션 시작 |
| `/resume` | 저장된 세션 목록 표시 (최근 10개) |
| `/resume <number>` | 번호를 지정하여 해당 세션 재개 (예: `/resume 3`) |
| `/pwd` | 현재 세션의 작업 디렉토리 표시 |
| `/clear` | AI 대화 기록 초기화 및 진행 중인 요청 취소 |
| `/stop` | 현재 진행 중인 AI 요청 중단 |

## File Transfer

| 커맨드 | 설명 |
|--------|------|
| `/down <file>` | 서버의 파일을 다운로드 (예: `/down ./output.log`) |
| 파일 전송 | 봇에게 파일을 직접 보내면 현재 세션 디렉토리에 업로드 |

`/down` 커맨드는 절대 경로와 상대 경로 모두 지원합니다. 상대 경로는 현재 세션 디렉토리 기준으로 해석됩니다.

## Shell

| 커맨드 | 설명 |
|--------|------|
| `!<command>` | 셸 커맨드 직접 실행 (예: `!ls -la`, `!git status`) |

`!` 접두사를 사용하면 현재 세션 디렉토리에서 셸 커맨드를 바로 실행할 수 있습니다.

## AI Chat

슬래시 커맨드나 `!`로 시작하지 않는 일반 메시지는 AI 에이전트에게 전달됩니다. AI는 세션 디렉토리 내의 파일을 읽고, 수정하고, 커맨드를 실행할 수 있습니다.

## Agent

| 커맨드 | 설명 |
|--------|------|
| `/agent` | 현재 사용 중인 AI 에이전트 표시 및 사용 가능한 에이전트 목록 |
| `/agent <name>` | AI 에이전트 전환 |

사용 가능한 에이전트: `claude`, `gemini`, `codex`, `opencode`

## Tool Management

| 커맨드 | 설명 |
|--------|------|
| `/availabletools` | 사용 가능한 모든 AI 도구 목록 표시 |
| `/allowedtools` | 현재 허용된 도구 목록 표시 |
| `/allowed +name` | 도구 추가 (예: `/allowed +Bash`) |
| `/allowed -name` | 도구 제거 (예: `/allowed -Bash`) |

## Help

| 커맨드 | 설명 |
|--------|------|
| `/help` | 도움말 표시 |
