# SFTP 원격 패널 + rsync/scp 전송 기능 구현 문서

## 개요

cokacdir에 SFTP를 통한 원격 디렉토리 브라우징과 rsync/scp를 통한 파일 전송 기능을 추가하였다. 로컬 패널과 동일한 인터페이스로 원격 서버를 탐색하고, 파일을 주고받을 수 있다.

### 핵심 기능

- 원격 디렉토리를 로컬과 동일한 패널 UI로 표시
- Go to Path(`/`)에서 `user@host:/path` 입력으로 접속
- 저장된 원격 프로필을 북마크 목록에 혼합 표시 (색상 구분)
- SSH 인증: 비밀번호 또는 PEM 키 파일
- 파일 전송: rsync 우선, 없으면 scp 자동 fallback
- 원격 작업: 브라우징, 복사, 삭제, 이름변경, 디렉토리 생성

---

## 아키텍처

```
┌──────────────────────────────────────────────────────┐
│                    UI Layer                          │
│  panel.rs (헤더/푸터 표시)                             │
│  dialogs.rs (RemoteConnect 다이얼로그, 북마크 혼합)      │
│  app.rs (PanelState 확장, 원격 분기 로직)               │
│  theme.rs / theme_loader.rs (원격 색상 필드)            │
├──────────────────────────────────────────────────────┤
│                  Services Layer                      │
│  remote.rs (SSH 연결, SFTP 세션, 프로필 관리)           │
│  remote_transfer.rs (rsync/scp 파일 전송)              │
├──────────────────────────────────────────────────────┤
│                  Config Layer                        │
│  config.rs (remote_profiles 저장)                     │
├──────────────────────────────────────────────────────┤
│              External Dependencies                   │
│  russh 0.46 / russh-sftp 2.0 / russh-keys 0.46      │
│  (순수 Rust, C 라이브러리 의존성 없음)                   │
└──────────────────────────────────────────────────────┘
```

---

## 파일별 변경 상세

### 새로 생성된 파일

#### `src/services/remote.rs`

SSH/SFTP 연결 및 원격 파일 작업의 핵심 모듈.

**주요 타입:**

```rust
/// 인증 방식 (settings.json에 저장)
pub enum RemoteAuth {
    Password { password: String },
    KeyFile { path: String, passphrase: Option<String> },
}

/// 원격 서버 프로필
pub struct RemoteProfile {
    pub name: String,           // 표시 이름
    pub host: String,           // 호스트명/IP
    pub port: u16,              // 기본 22
    pub user: String,           // SSH 사용자명
    pub auth: RemoteAuth,       // 인증 방식
    pub default_path: String,   // 초기 원격 경로
}

/// 패널에 부착되는 원격 컨텍스트
pub struct RemoteContext {
    pub profile: RemoteProfile,
    pub session: SftpSession,
    pub status: ConnectionStatus,
}
```

**SftpSession 메서드:**

| 메서드 | 설명 |
|--------|------|
| `connect(profile)` | SSH 연결 + SFTP 채널 열기 |
| `list_dir(path)` | 디렉토리 목록 조회 → `Vec<SftpFileEntry>` |
| `remove(path, is_dir)` | 파일/디렉토리 삭제 (재귀) |
| `rename(old, new)` | 이름 변경 |
| `mkdir(path)` | 디렉토리 생성 |
| `create_file(path)` | 빈 파일 생성 |
| `disconnect()` | 연결 종료 |
| `is_connected()` | 연결 상태 확인 |

**헬퍼 함수:**

| 함수 | 설명 |
|------|------|
| `parse_remote_path(input)` | `user@host:/path` 또는 `user@host:port:/path` 파싱 → `(user, host, port, path)` |
| `format_remote_display(profile, path)` | 원격 경로 표시 문자열 생성 |
| `find_matching_profile(profiles, user, host, port)` | 저장된 프로필 중 매칭 검색 |
| `format_remote_permissions(mode)` | 모드 비트 → `rwxrwxrwx` 문자열 |

**SSH 연결 설정:**
- `inactivity_timeout`: 300초
- `keepalive_interval`: 30초
- `keepalive_max`: 3회
- 서버 키 검증: 모든 서버 키 허용 (StrictHostKeyChecking=no 동등)

---

#### `src/services/remote_transfer.rs`

rsync/scp를 래핑한 파일 전송 모듈. 기존 `ProgressMessage` 체계를 그대로 활용.

**전송 흐름:**

```
transfer_files_with_progress()
    ├── has_rsync()? → transfer_rsync() (진행률 파싱 포함)
    └── fallback   → transfer_scp()  (완료/실패만 보고)
```

**rsync 명령 생성 예시:**
```bash
# 키 파일 인증
rsync -avz --info=progress2 --no-inc-recursive \
  -e "ssh -i /path/to/key -p 22 -o StrictHostKeyChecking=no ..." \
  /local/path user@host:/remote/path

# 비밀번호 인증 (sshpass 필요)
sshpass -p 'password' rsync -avz --info=progress2 ...
```

**rsync 진행률 파싱:**
- `--info=progress2` 출력 형식: `  1,234,567  42%  1.23MB/s  0:01:23`
- 전송 바이트와 퍼센트에서 전체 크기 계산
- 기존 `ProgressMessage::FileProgress(transferred, total)` 변환

**scp fallback:**
- rsync가 없는 환경에서 자동 전환
- `-r` 옵션으로 재귀 복사 지원
- 진행률 없이 완료/실패만 보고

**비밀번호 인증 시 sshpass 필요:**
- `sshpass` 미설치 시 에러 메시지: "Password auth requires 'sshpass'. Install: sudo apt install sshpass"

---

### 수정된 파일

#### `Cargo.toml`

추가된 의존성:

```toml
russh = "0.46"          # 순수 Rust SSH 클라이언트
russh-sftp = "2.0"      # SFTP 프로토콜 구현
russh-keys = "0.46"     # SSH 키 파일 처리
async-trait = "0.1"     # russh Handler 트레이트용
```

---

#### `src/services/mod.rs`

```rust
pub mod remote;           // NEW
pub mod remote_transfer;  // NEW
```

---

#### `src/config.rs`

`Settings` 구조체에 추가:

```rust
/// Remote server profiles for SSH/SFTP connections
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub remote_profiles: Vec<RemoteProfile>,
```

- `serde(default)`: 기존 settings.json에 필드 없어도 빈 벡터로 로드
- `skip_serializing_if`: 프로필 없으면 JSON에 포함하지 않음

---

#### `src/ui/app.rs`

가장 많은 변경이 발생한 파일. 핵심 변경사항:

##### 1. PanelState 확장

```rust
pub struct PanelState {
    // ... 기존 필드 ...
    /// Remote context — None means local panel
    pub remote_ctx: Option<Box<RemoteContext>>,
}
```

새 메서드:

```rust
pub fn is_remote(&self) -> bool   // 원격 패널인지 확인
pub fn display_path(&self) -> String  // 원격이면 user@host:/path, 로컬이면 path
```

##### 2. load_files() 리팩터링

```
load_files()
    ├── is_remote()? → load_files_remote()  // SFTP list_dir() 호출
    └── else        → load_files_local()   // 기존 fs::read_dir() 로직
```

공통 로직 추출:
- `sort_items()`: 정렬 로직 (디렉토리 우선, SortBy/SortOrder 적용)
- `finalize_load()`: pending_focus 처리, selected_index 범위 보정

`load_files_remote()` 동작:
1. SFTP `list_dir()` 호출
2. `SftpFileEntry` → `FileItem` 변환
3. 공통 정렬 적용
4. 성공 시 `ConnectionStatus::Connected` 업데이트
5. 실패 시 `ConnectionStatus::Disconnected(error)` 설정
6. disk_total/disk_available = 0 (원격은 디스크 정보 없음)

##### 3. DialogType 추가

```rust
pub enum DialogType {
    // ... 기존 ...
    RemoteConnect,       // 원격 연결 다이얼로그 (인증 정보 입력)
    RemoteConnecting,    // 연결 중 스피너
    RemoteProfileSave,   // 프로필 저장 확인
}
```

##### 4. RemoteConnectState

원격 연결 다이얼로그의 상태를 관리하는 구조체:

```rust
pub struct RemoteConnectState {
    pub selected_field: usize,  // 0=host ~ 5=passphrase
    pub host: String,
    pub port: String,           // "22"
    pub user: String,
    pub auth_type: usize,       // 0=password, 1=key_file
    pub password: String,
    pub key_path: String,       // "~/.ssh/id_rsa"
    pub passphrase: String,
    pub remote_path: String,    // "/"
    pub profile_name: String,
    pub error: Option<String>,
    pub cursor_pos: usize,
}
```

주요 메서드:
- `from_parsed(user, host, port, path)`: 파싱된 원격 경로로 초기화
- `to_profile()`: RemoteProfile로 변환
- `field_count()`: 인증 방식에 따라 5 또는 6 반환
- `active_field_mut()`: 현재 선택된 필드의 가변 참조

##### 5. execute_goto() 원격 확장

```
execute_goto(path_str)
    ├── parse_remote_path() 성공? → execute_goto_remote()
    ├── 현재 패널이 원격?
    │   ├── 절대 로컬 경로(/, ~)? → disconnect + 로컬 goto
    │   └── 상대 경로? → execute_goto_remote_relative()
    └── 로컬 goto (기존 로직)
```

##### 6. 원격 연결/해제

```rust
// 저장된 프로필로 연결
fn execute_goto_remote(&mut self, user, host, port, remote_path) {
    // 매칭 프로필 있으면 → 저장된 인증으로 연결
    // 없으면 → RemoteConnect 다이얼로그 표시
}

// SFTP 연결 실행
pub fn connect_remote_panel(&mut self, profile, path) {
    // SftpSession::connect() → RemoteContext 생성 → 패널에 부착
}

// 원격 해제 → 로컬로 복귀
pub fn disconnect_remote_panel(&mut self) {
    // remote_ctx.take() → session.disconnect() → 홈 디렉토리로 이동
}
```

##### 7. 원격 파일 작업 분기

모든 파일 작업 메서드에 `is_remote()` 체크 추가:

| 메서드 | 원격 동작 |
|--------|-----------|
| `execute_delete()` | `ctx.session.remove(path, is_dir)` |
| `execute_mkdir()` | `ctx.session.mkdir(path)` |
| `execute_mkfile()` | `ctx.session.create_file(path)` (에디터 열기 없음) |
| `execute_rename()` | `ctx.session.rename(old, new)` |
| `enter_selected()` | 디렉토리만 진입, 파일 열기 차단 |

##### 8. 원격 파일 전송

`execute_copy_to_with_progress_internal()` 변경:

```
소스 또는 대상이 원격?
    ├── Yes → remote_transfer::transfer_files_with_progress()
    │        (rsync 우선, scp fallback)
    └── No  → file_ops::copy_files_with_progress() (기존 로컬 복사)
```

기존 `ProgressMessage` 체계와 `DialogType::Progress` 다이얼로그를 그대로 활용.

##### 9. show_copy_dialog() / show_move_dialog()

대상 패널이 원격이면 `user@host:/path/` 형태로 input 프리필.

##### 10. goto_home()

원격 패널에서 홈 키 누르면 자동 disconnect 후 로컬 홈으로 이동.

---

#### `src/ui/panel.rs`

##### 헤더 표시

- 원격 패널: `[SSH] user@host:/path` 형태로 경로 표시
- `remote_prefix`: `is_remote()` 일 때 `"[SSH] "` 추가
- 타이틀 색상: 원격이면 `theme.panel.remote_indicator` 사용

```rust
.title_style(if panel.is_remote() && is_active {
    Style::default().fg(theme.panel.remote_indicator).add_modifier(Modifier::BOLD)
} else if is_active {
    Style::default().fg(theme.panel.border_active).add_modifier(Modifier::BOLD)
} else if panel.is_remote() {
    Style::default().fg(theme.panel.remote_indicator)
} else {
    Style::default().fg(theme.panel.file_text)
})
```

##### 푸터 표시

- 원격 패널: 디스크 정보 대신 `user@host` 연결 정보 표시
- `theme.panel.remote_indicator` 색상 사용

```rust
if panel.is_remote() {
    // "user@host" 표시
} else if panel.disk_total > 0 {
    // 기존 디스크 정보 표시
}
```

---

#### `src/ui/theme.rs`

##### PanelColors 추가 필드

```rust
pub struct PanelColors {
    // ... 기존 16개 필드 ...
    pub remote_indicator: Color,    // [SSH] 인디케이터 색상
}
```

##### DialogColors 추가 필드

```rust
pub struct DialogColors {
    // ... 기존 필드 ...
    pub remote_bookmark_text: Color,             // 북마크 목록 내 원격 항목
    pub remote_connect_field_label: Color,       // 연결 다이얼로그 필드 레이블
    pub remote_connect_field_value: Color,       // 연결 다이얼로그 필드 값
    pub remote_connect_field_selected_bg: Color, // 선택된 필드 배경
}
```

##### 테마별 색상값

| 필드 | Dawn of Coding | Dark | Light |
|------|---------------|------|-------|
| `panel.remote_indicator` | 108 (녹색) | 117 (밝은 파랑) | 67 (파랑) |
| `dialog.remote_bookmark_text` | 108 | 117 | 67 |
| `dialog.remote_connect_field_label` | 145 | 252 | 243 |
| `dialog.remote_connect_field_value` | 188 | 255 | 238 |
| `dialog.remote_connect_field_selected_bg` | 60 | 117 | 67 |

##### to_json() 업데이트

패널 섹션:
```json
"__remote_indicator__": "원격 패널 [SSH] 인디케이터 색상...",
"remote_indicator": 108
```

다이얼로그 섹션:
```json
"__remote_bookmark_text__": "북마크 목록 내 원격 프로필 텍스트 색상...",
"remote_bookmark_text": 108,
"__remote_connect_field_label__": "원격 연결 다이얼로그의 필드 레이블...",
"remote_connect_field_label": 145,
...
```

---

#### `src/ui/theme_loader.rs`

JSON 로딩 구조체에 새 필드 추가:

```rust
// PanelColorsJson
#[serde(default = "default_67")]
pub remote_indicator: u8,

// DialogColorsJson
#[serde(default = "default_67")]
pub remote_bookmark_text: u8,
#[serde(default = "default_243")]
pub remote_connect_field_label: u8,
#[serde(default = "default_243")]
pub remote_connect_field_value: u8,
#[serde(default = "default_67")]
pub remote_connect_field_selected_bg: u8,
```

변환 코드:
```rust
let panel = PanelColors {
    // ...
    remote_indicator: idx(json.panel.remote_indicator),
};

let dialog = DialogColors {
    // ...
    remote_bookmark_text: idx(json.dialog.remote_bookmark_text),
    remote_connect_field_label: idx(json.dialog.remote_connect_field_label),
    remote_connect_field_value: idx(json.dialog.remote_connect_field_value),
    remote_connect_field_selected_bg: idx(json.dialog.remote_connect_field_selected_bg),
};
```

---

#### `src/ui/dialogs.rs`

##### 1. Go to Path 다이얼로그 — 북마크 + 원격 프로필 혼합

```
기존: bookmarked_path만 표시
변경: bookmarked_path + remote_profiles 하나의 목록으로 혼합
```

구현:
- `mixed_entries: Vec<String>` — 로컬 북마크 + 원격 프로필
- `remote_indices: HashSet<usize>` — 원격 항목 인덱스 추적
- 원격 프로필은 `[SSH] user@host:/path` 형태로 표시
- `draw_bookmark_list()`에 `remote_indices` 전달 → 원격 항목에 `remote_bookmark_text` 색상 적용
- 퍼지 검색(`fuzzy_match`) 로컬/원격 모두 동일 적용

선택 처리:
- 원격 프로필 선택 → `connect_remote_panel()` 직접 호출
- 로컬 북마크 선택 → 기존 로컬 이동 로직

##### 2. RemoteConnect 다이얼로그

```
┌─ Remote Connect ────────────────────┐
│      Host: example.com              │
│      Port: 22                       │
│      User: admin                    │
│      Auth: Password (Tab to toggle) │
│  Password: ********                 │
│                                     │
│ Tab/↑↓:navigate Enter:connect Esc:cancel │
└─────────────────────────────────────┘
```

필드 네비게이션:
- `Tab`/`↓`: 다음 필드 (Auth 필드에서는 Password ↔ Key File 토글)
- `BackTab`/`↑`: 이전 필드
- `Enter`: 연결 시도 → 성공 시 `RemoteProfileSave` 다이얼로그
- `Esc`: 취소

Auth Type = Key File일 때:
```
│      Auth: Key File (Tab to toggle)  │
│  Key File: ~/.ssh/id_rsa             │
│ Passphrase: (none)                   │
```

##### 3. RemoteProfileSave 다이얼로그

연결 성공 후 표시:
- `Enter`: 프로필 저장 (기존 프로필이면 업데이트, 새로우면 추가)
- `Esc`: 저장하지 않고 닫기
- 프로필 이름 편집 가능

##### 4. RemoteConnecting 다이얼로그

연결 중 메시지 표시 (현재는 blocking이라 실제로는 connect_remote_panel에서 직접 처리).

---

## 사용 방법

### 원격 서버 접속

1. `/` 키로 Go to Path 다이얼로그 열기
2. `user@host:/path` 형식으로 입력
3. 저장된 프로필이 있으면 자동 연결, 없으면 인증 정보 입력 다이얼로그 표시
4. 연결 성공 시 원격 디렉토리가 패널에 표시됨

### 저장된 프로필로 접속

1. `/` 키로 Go to Path 다이얼로그 열기
2. 북마크 목록에 `[SSH]` 접두사로 원격 프로필 표시
3. 방향키로 선택 후 Enter

### 파일 전송

1. 로컬 패널에서 파일 선택
2. `c` (또는 `F5`) 키로 복사 다이얼로그
3. 대상이 원격 패널이면 `user@host:/remote/path/` 형태로 표시
4. Enter로 전송 시작 → 진행률 다이얼로그 표시

### 원격 파일 작업

| 키 | 작업 |
|----|------|
| Enter | 디렉토리 진입 (파일 열기는 불가) |
| Backspace | 상위 디렉토리 |
| F7 | 디렉토리 생성 |
| Shift+F7 | 파일 생성 |
| F8 | 삭제 |
| F6 | 이름 변경 |
| `~` | 로컬 홈으로 복귀 (원격 해제) |

### 원격 해제

- Go to Path에서 로컬 경로(`/` 또는 `~` 시작) 입력
- `~` 키로 홈 디렉토리 이동

---

## 재사용된 기존 코드

| 구성요소 | 원래 위치 | 재사용 방식 |
|----------|-----------|-------------|
| `ProgressMessage` 열거형 | `file_ops.rs:18-38` | rsync/scp 전송 진행률에 동일 사용 |
| `FileOperationProgress` + `DialogType::Progress` | `app.rs` | 전송 진행률 UI 그대로 사용 |
| `FileItem` 구조체 | `app.rs:541-549` | SFTP 엔트리를 이 형태로 변환 |
| 정렬 로직 | `sort_items()` 추출 | 원격 파일에도 동일 적용 |
| `fuzzy_match()` | `app.rs:305` | 북마크+프로필 검색에 재사용 |

---

## 설정 파일 형식

### `~/.cokacdir/settings.json`

```json
{
  "remote_profiles": [
    {
      "name": "my-server",
      "host": "example.com",
      "port": 22,
      "user": "admin",
      "auth": {
        "type": "password",
        "password": "secret"
      },
      "default_path": "/home/admin"
    },
    {
      "name": "dev-server",
      "host": "dev.example.com",
      "port": 2222,
      "user": "deploy",
      "auth": {
        "type": "key_file",
        "path": "~/.ssh/deploy_key",
        "passphrase": null
      },
      "default_path": "/var/www"
    }
  ]
}
```

---

## 테마 커스터마이징

### 새로 추가된 색상 필드

`~/.cokacdir/themes/{theme_name}.json`에서 수정:

```json
{
  "panel": {
    "remote_indicator": 108
  },
  "dialog": {
    "remote_bookmark_text": 108,
    "remote_connect_field_label": 145,
    "remote_connect_field_value": 188,
    "remote_connect_field_selected_bg": 60
  }
}
```

---

## 시스템 요구사항

### 필수

- SSH 접속 가능한 원격 서버 (SFTP 서브시스템 활성화)

### 선택 (파일 전송용)

- `rsync`: 진행률 표시 포함 전송 (권장)
- `scp`: rsync 없을 때 자동 fallback
- `sshpass`: 비밀번호 인증 시 rsync/scp에 필요

```bash
# Ubuntu/Debian
sudo apt install rsync sshpass

# macOS
brew install rsync
# sshpass는 보안상 brew에서 미제공 — 소스 빌드 필요
```

---

## 검증 방법

1. **연결 테스트**: `/` → `user@host:/path` → SFTP 연결 → 원격 파일 목록 확인
2. **브라우징 테스트**: Enter로 디렉토리 진입/나가기, 정렬 변경, 파일 선택
3. **전송 테스트**: 로컬 → `c` → 원격 패널로 복사 → rsync + 진행률 확인
4. **역방향 테스트**: 원격 → `c` → 로컬 패널로 다운로드
5. **원격 작업 테스트**: 삭제(F8), 이름변경(F6), mkdir(F7)
6. **프로필 테스트**: 저장 → 재시작 → 북마크 목록에서 선택 → 재연결
7. **Fallback 테스트**: rsync 없는 환경에서 scp fallback 동작 확인
8. **연결 끊김 테스트**: 로컬 경로 입력 시 원격 해제 확인

---

## 변경 파일 요약

| 파일 | 상태 | 변경 내용 |
|------|------|-----------|
| `Cargo.toml` | 수정 | russh, russh-sftp, russh-keys, async-trait 의존성 추가 |
| `src/services/mod.rs` | 수정 | remote, remote_transfer 모듈 등록 |
| `src/services/remote.rs` | **신규** | RemoteProfile, SftpSession, 연결/SFTP 작업 관리 |
| `src/services/remote_transfer.rs` | **신규** | rsync/scp 전송, 진행률 파싱 |
| `src/config.rs` | 수정 | Settings에 remote_profiles 추가 |
| `src/ui/app.rs` | 수정 | PanelState 확장, load_files 분기, execute_* 분기, goto 확장, 전송 분기 |
| `src/ui/dialogs.rs` | 수정 | 북마크+프로필 혼합, RemoteConnect/ProfileSave 다이얼로그 |
| `src/ui/panel.rs` | 수정 | 원격 패널 헤더/푸터 표시 ([SSH] 인디케이터) |
| `src/ui/theme.rs` | 수정 | 원격 관련 색상 필드 5개 추가, 3개 테마 모두 적용 |
| `src/ui/theme_loader.rs` | 수정 | JSON 로딩 구조체에 새 필드 추가 |

---

## 알려진 제한사항

1. **원격 파일 직접 열기 불가**: 뷰어/에디터는 로컬 파일만 지원. 원격 파일은 먼저 복사(다운로드) 후 열어야 함
2. **비밀번호 저장**: settings.json에 평문 저장됨. 보안이 중요한 환경에서는 키 파일 인증 권장
3. **연결 블로킹**: SSH 연결 시 UI가 잠시 멈춤 (async background 연결은 향후 개선 가능)
4. **서버 키 검증 없음**: 모든 서버 키를 자동 수락. 향후 known_hosts 지원 가능
5. **russh API 호환성**: russh/russh-sftp 버전에 따라 일부 메서드명 조정 필요할 수 있음 (첫 빌드 시 확인)
