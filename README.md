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
| `o` | Open folder in Finder |
| `c` | Open folder in VS Code |

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
