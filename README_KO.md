# AEMI

Agent Mirror - AI 에이전트 라우팅 도구 (Telegram, Discord 봇 연동)

AI 에이전트 응답을 Telegram/Discord 봇을 통해 중계하는 CLI 도구입니다.

## 원본 프로젝트

이 프로젝트는 [kstost/cokacdir](https://github.com/kstost/cokacdir)에서 포크되었습니다. 원본 프로젝트의 LLM CLI 라우팅 부분을 기반으로 하며, TUI 파일 매니저를 제거하고 봇 중계 기능에 집중하도록 변경되었습니다.

## 주요 기능

- **AI 에이전트 라우팅**: `--prompt`를 통해 AI 에이전트에 질의하고 응답을 수신
- **Telegram 봇**: `--routing telegram`으로 Telegram을 통한 AI 에이전트 라우팅
- **Discord 봇**: `--routing discord`로 Discord를 통한 AI 에이전트 라우팅
- **멀티 봇**: 여러 Telegram 봇 토큰을 동시에 실행
- **접근 제어**: 라우팅 시 `--chat-id`(Telegram) / `--channel-id`(Discord) 필수

## 사용법

```bash
# Claude Code에 직접 질의
aemi --prompt "explain this code"

# Claude로 Telegram 봇 서버 시작 (--chat-id 필수)
aemi --agent claude --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Gemini로 Telegram 봇 서버 시작
aemi --agent gemini --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Discord 봇 서버 시작 (--channel-id 필수)
aemi --agent claude --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Codex로 Telegram 봇 서버 시작
aemi --agent codex --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# Gemini로 Discord 봇 서버 시작
aemi --agent gemini --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# Codex로 Discord 봇 서버 시작
aemi --agent codex --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# OpenCode로 Telegram 봇 서버 시작
aemi --agent opencode --routing telegram --token <TOKEN> --chat-id <CHAT_ID>

# OpenCode로 Discord 봇 서버 시작
aemi --agent opencode --routing discord --token <TOKEN> --channel-id <CHANNEL_ID>

# 여러 Telegram 봇 동시 실행
aemi --agent claude --routing telegram --token <TOKEN1> <TOKEN2> <TOKEN3> --chat-id <CHAT_ID>
```

## 설치

### 사전 요구사항

- 사용하려는 에이전트의 CLI 도구를 설치해야 합니다 ([에이전트 종류](#에이전트-종류) 참조)

### 소스에서 빌드

```bash
# 클론
git clone https://github.com/KyongSik-Yoon/aemi.git
cd aemi

# 빌드
cargo build --release

# 바이너리 위치
./target/release/aemi
```

크로스 컴파일 등 자세한 빌드 방법은 [build_manual.md](build_manual.md)를 참조하세요.

## 에이전트 종류

| 에이전트 | CLI 플래그 | 상태 | 우선순위 |
|---------|-----------|------|---------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `--agent claude` | 사용 가능 | - |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | `--agent gemini` | 사용 가능 | - |
| [Codex CLI](https://github.com/openai/codex) | `--agent codex` | 사용 가능 | - |
| [OpenCode](https://opencode.ai) | `--agent opencode` | 사용 가능 | - |

### 에이전트별 사전 요구사항

각 에이전트는 해당 CLI 도구가 설치되어 있어야 합니다:

- **Claude**: `npm install -g @anthropic-ai/claude-code`
- **Gemini**: `npm install -g @google/gemini-cli`
- **Codex**: `npm install -g @openai/codex`
- **OpenCode**: `npm install -g opencode` (또는 [opencode.ai](https://opencode.ai/docs/)에서 다른 설치 방법 참조)

## 슬래시 명령어

전체 봇 명령어 목록은 [docs/slash_commands_ko.md](docs/slash_commands_ko.md)를 참조하세요.

## 지원 플랫폼

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## 라이선스

MIT License

## 면책 조항

이 소프트웨어는 상품성, 특정 목적에의 적합성 및 비침해에 대한 보증을 포함하되 이에 국한되지 않는 어떠한 종류의 명시적 또는 묵시적 보증 없이 "있는 그대로" 제공됩니다.

어떠한 경우에도 저작자, 저작권 보유자 또는 기여자는 계약, 불법 행위 또는 기타 사유로 인한 청구, 손해 또는 기타 책임에 대해 소프트웨어 또는 소프트웨어의 사용 또는 기타 거래로 인해 발생하는 어떠한 책임도 지지 않습니다.

**사용에 따른 모든 책임은 사용자에게 있습니다.**
