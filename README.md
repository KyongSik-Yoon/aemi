# AIMI

LLM CLI routing tool with Telegram and Discord bot integration.

Claude Code의 응답을 Telegram/Discord 봇을 통해 중계하는 CLI 도구입니다.

## Origin

이 프로젝트는 [kstost/aimi](https://github.com/kstost/aimi)의 fork입니다. 원본 프로젝트의 LLM CLI routing 부분을 기반으로 하며, TUI 파일 매니저 기능은 제거하고 봇 중계 기능에 집중하였습니다.

## Features

- **Claude Code Routing**: `--prompt`로 Claude Code에 질의하고 응답을 받음
- **Telegram Bot**: `--ccserver`로 Telegram 봇 서버를 실행하여 채팅으로 Claude Code 사용
- **Discord Bot**: `--ccserver-discord`로 Discord 봇 서버를 실행하여 채팅으로 Claude Code 사용
- **Multi-Bot**: 여러 Telegram 봇 토큰을 동시에 실행 가능
- **Access Control**: `--chat-id` / `--channel-id`로 특정 채팅/채널만 허용

## Usage

```bash
# Claude Code에 직접 질의
aimi --prompt "explain this code"

# Telegram 봇 서버 실행
aimi --ccserver <TELEGRAM_BOT_TOKEN>

# Telegram 봇 + 특정 채팅만 허용
aimi --ccserver <TOKEN> --chat-id <CHAT_ID>

# Discord 봇 서버 실행
aimi --ccserver-discord <DISCORD_BOT_TOKEN>

# Discord 봇 + 특정 채널만 허용
aimi --ccserver-discord <TOKEN> --channel-id <CHANNEL_ID>

# 여러 Telegram 봇 동시 실행
aimi --ccserver <TOKEN1> <TOKEN2> <TOKEN3>
```

## Installation

### Prerequisites

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 설치 필요

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

# 바이너리 위치
./target/release/aimi
```

크로스 컴파일 등 상세 빌드 방법은 [build_manual.md](build_manual.md)를 참고하세요.

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

**USE AT YOUR OWN RISK.**
