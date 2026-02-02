# COKACDIR

Dual-panel terminal file manager with AI-powered natural language commands.

**Terminal File Manager for Vibe Coders** - An easy terminal explorer for vibe coders who are scared of the terminal.

## Features

- **Blazing Fast**: Written in Rust for maximum performance. ~10ms startup, ~5MB memory usage, ~4MB static binary with zero runtime dependencies.
- **AI-Powered Commands**: Natural language file operations powered by Claude AI. Press `.` and describe what you want.
- **Dual-Panel Navigation**: Classic dual-panel interface for efficient file management
- **Keyboard Driven**: Full keyboard navigation designed for power users
- **Built-in Viewer & Editor**: View and edit files with syntax highlighting for 20+ languages
- **Image Viewer**: View images directly in terminal with zoom and pan support
- **Process Manager**: Monitor and manage system processes. Sort by CPU, memory, or PID.
- **File Search**: Find files by name pattern with recursive search
- **Customizable Themes**: Light/Dark themes with full color customization

## Installation

### Quick Install (Recommended)

```bash
/bin/bash -c "$(curl -fsSL https://cokacdir.cokac.com/install.sh)"
```

Then run:

```bash
cokacdir
```

### From Source

```bash
# Clone the repository
git clone https://github.com/kstost/cokacdir.git
cd cokacdir

# Build release version
cargo build --release

# Run
./target/release/cokacdir
```

### Cross-Platform Build

Build for multiple platforms using the included build system:

```bash
# Build for current platform
python3 build.py

# Build for macOS (arm64 + x86_64)
python3 build.py --macos

# Build for all platforms
python3 build.py --all

# Check build tools status
python3 build.py --status
```

See [build_manual.md](build_manual.md) for detailed build instructions.

## Enable AI Commands (Optional)

Install Claude Code to unlock natural language file operations:

```bash
npm install -g @anthropic-ai/claude-code
```

Learn more at [docs.anthropic.com](https://docs.anthropic.com/en/docs/claude-code)

## Custom File Handlers (Extension Handler)

You can define custom programs to open files based on their extension. When you press `Enter` on a file, the configured handler will be executed instead of the built-in viewer/editor.

### Configuration File

Edit `~/.cokacdir/settings.json` and add the `extension_handler` section:

```json
{
  "extension_handler": {
    "jpg|jpeg|png|gif|webp": ["feh {{FILEPATH}}", "eog {{FILEPATH}}"],
    "mp4|avi|mkv|webm": ["vlc {{FILEPATH}}", "mpv {{FILEPATH}}"],
    "pdf": ["evince {{FILEPATH}}", "xdg-open {{FILEPATH}}"],
    "rs|py|js|ts": ["@vim {{FILEPATH}}", "@nano {{FILEPATH}}"],
    "md": ["@vim {{FILEPATH}}"]
  }
}
```

### Syntax

| Element | Description |
|---------|-------------|
| `extension_handler` | Main configuration key |
| `"jpg\|jpeg\|png"` | Pipe-separated extensions (case-insensitive) |
| `["cmd1", "cmd2"]` | Array of commands (fallback order) |
| `{{FILEPATH}}` | Placeholder replaced with actual file path |
| `@` prefix | Terminal mode for TUI apps (vim, nano, etc.) |

### Execution Modes

| Mode | Prefix | Behavior | Use Case |
|------|--------|----------|----------|
| Background | (none) | Launches program and returns immediately | GUI apps (feh, vlc, evince) |
| Terminal | `@` | Suspends COKACDIR, runs program, restores on exit | TUI apps (vim, nano, less) |

### Fallback Mechanism

Commands are tried in order. If the first command fails, the next one is attempted:

```json
{
  "extension_handler": {
    "jpg": ["feh {{FILEPATH}}", "eog {{FILEPATH}}", "xdg-open {{FILEPATH}}"]
  }
}
```

1. Try `feh` first
2. If `feh` fails → try `eog`
3. If `eog` fails → try `xdg-open`
4. If all fail → show error dialog

### Examples

#### Image Viewer

```json
{
  "extension_handler": {
    "jpg|jpeg|png|gif|bmp|webp|ico|tiff": ["feh {{FILEPATH}}"]
  }
}
```

#### Video Player

```json
{
  "extension_handler": {
    "mp4|avi|mkv|webm|mov|flv": ["vlc {{FILEPATH}}", "mpv {{FILEPATH}}"]
  }
}
```

#### PDF Reader

```json
{
  "extension_handler": {
    "pdf": ["evince {{FILEPATH}}", "okular {{FILEPATH}}", "xdg-open {{FILEPATH}}"]
  }
}
```

#### Text Editor (Terminal Mode)

```json
{
  "extension_handler": {
    "txt|md|rst": ["@vim {{FILEPATH}}"],
    "rs|py|js|ts|go|c|cpp|h": ["@vim {{FILEPATH}}", "code {{FILEPATH}}"]
  }
}
```

#### Open with VS Code

```json
{
  "extension_handler": {
    "rs|py|js|ts|json|yaml|toml": ["code {{FILEPATH}}"]
  }
}
```

### Complete Example

```json
{
  "left_panel": {
    "sort_by": "name",
    "sort_order": "asc"
  },
  "right_panel": {
    "sort_by": "name",
    "sort_order": "asc"
  },
  "active_panel": "left",
  "theme": {
    "name": "dark"
  },
  "extension_handler": {
    "jpg|jpeg|png|gif|webp|bmp": ["feh {{FILEPATH}}", "eog {{FILEPATH}}"],
    "mp4|avi|mkv|webm|mov": ["vlc {{FILEPATH}}"],
    "pdf": ["evince {{FILEPATH}}"],
    "txt|md": ["@vim {{FILEPATH}}"],
    "rs|py|js|ts": ["@vim {{FILEPATH}}", "code {{FILEPATH}}"],
    "html": ["firefox {{FILEPATH}}", "xdg-open {{FILEPATH}}"]
  }
}
```

### Notes

- **Case-insensitive**: `JPG`, `jpg`, `Jpg` are all matched
- **Spaces in paths**: File paths with spaces are automatically escaped
- **Undefined extensions**: Files without a handler use the built-in viewer/editor
- **Hot-reload**: Edit settings.json in COKACDIR's editor and save (`Ctrl+S`) to apply changes immediately
- **Error handling**: If all handlers fail, an error dialog shows the last error message

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate files |
| `Enter` | Open directory |
| `Esc` | Parent directory |
| `Tab` | Switch panels |
| `←`/`→` | Switch panels (keep position) |
| `Home`/`End` | First / Last item |
| `PgUp`/`PgDn` | Move 10 lines |
| `/` | Go to path |
| `1` | Go to home directory |
| `2` | Refresh file list |

### File Operations

| Key | Action |
|-----|--------|
| `k` | Create directory |
| `x` | Delete |
| `r` | Rename |
| `t` | Create tar archive |
| `f` | Find/Search files |

### Clipboard Operations

| Key | Action |
|-----|--------|
| `Ctrl+C` | Copy to clipboard |
| `Ctrl+X` | Cut to clipboard |
| `Ctrl+V` | Paste from clipboard |

### View & Tools

| Key | Action |
|-----|--------|
| `h` | Help |
| `i` | File info |
| `e` | Edit file |
| `p` | Process manager |
| `` ` `` | Settings |

### macOS Only

| Key | Action |
|-----|--------|
| `o` | Open current folder in Finder |
| `c` | Open current folder in VS Code |

#### `o` - Open in Finder

Opens the current panel's directory in macOS Finder. Uses the native `open` command.

```
# Equivalent terminal command
open /path/to/current/folder
```

#### `c` - Open in VS Code

Opens the current panel's directory in Visual Studio Code. Automatically detects which VS Code variant is installed:

1. First tries `code` (VS Code stable)
2. Falls back to `code-insiders` (VS Code Insiders)
3. Shows error if neither is found

```
# Equivalent terminal command
code /path/to/current/folder
# or
code-insiders /path/to/current/folder
```

**Note**: VS Code CLI (`code` command) must be installed. In VS Code, press `Cmd+Shift+P` and run "Shell Command: Install 'code' command in PATH".

### Selection & AI

| Key | Action |
|-----|--------|
| `Space` | Select file |
| `*` | Select all |
| `;` | Select by extension |
| `n` / `s` / `d` / `y` | Sort by name / size / date / type |
| `.` | AI command |
| `q` | Quit |

### File Viewer

| Key | Action |
|-----|--------|
| `↑`/`↓`/`j`/`k` | Scroll |
| `PgUp`/`PgDn` | Page scroll |
| `Home`/`End`/`G` | Go to start/end |
| `Ctrl+F`/`/` | Search |
| `Ctrl+G` | Go to line |
| `b` | Toggle bookmark |
| `[`/`]` | Prev/Next bookmark |
| `H` | Toggle hex mode |
| `W` | Toggle word wrap |
| `E` | Open in editor |
| `Esc`/`Q` | Close viewer |

### File Editor

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save |
| `Ctrl+Z/Y` | Undo/Redo |
| `Ctrl+C/X/V` | Copy/Cut/Paste |
| `Ctrl+A` | Select all |
| `Ctrl+D` | Select word |
| `Ctrl+L` | Select line |
| `Ctrl+K` | Delete line |
| `Ctrl+J` | Duplicate line |
| `Ctrl+/` | Toggle comment |
| `Ctrl+F` | Find |
| `Ctrl+H` | Find & Replace |
| `Ctrl+G` | Go to line |
| `Alt+↑/↓` | Move line up/down |
| `Esc` | Close editor |

### Process Manager

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate processes |
| `PgUp`/`PgDn` | Page scroll |
| `k` | Kill process (SIGTERM) |
| `K` | Force kill (SIGKILL) |
| `r` | Refresh list |
| `p` | Sort by PID |
| `c` | Sort by CPU |
| `m` | Sort by memory |
| `n` | Sort by command name |
| `Esc`/`q` | Close |

### Image Viewer

| Key | Action |
|-----|--------|
| `+`/`-` | Zoom in/out |
| `r` | Reset zoom |
| `↑`/`↓`/`←`/`→` | Pan image |
| `PgUp`/`PgDn` | Previous/Next image |
| `Esc`/`q` | Close viewer |

## Supported Platforms

- macOS (Apple Silicon & Intel)
- Linux (x86_64 & ARM64)

## License

MIT License

## Author

cokac <monogatree@gmail.com>

Homepage: https://cokacdir.cokac.com

## Disclaimer

THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.

IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

This includes, without limitation:

- Data loss or corruption
- System damage or malfunction
- Security breaches or vulnerabilities
- Financial losses
- Any direct, indirect, incidental, special, exemplary, or consequential damages

The user assumes full responsibility for all consequences arising from the use of this software, regardless of whether such use was intended, authorized, or anticipated.

**USE AT YOUR OWN RISK.**
