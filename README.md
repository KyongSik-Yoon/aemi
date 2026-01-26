# cokacdir

[![npm version](https://badge.fury.io/js/cokacdir.svg)](https://www.npmjs.com/package/cokacdir)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Dual-panel file manager for terminal with AI-powered natural language commands.

![cokacdir screenshot](https://cokacdir.cokac.com/screenshot.png)

## Features

- **Dual-Panel Interface** - Classic / Midnight Commander style
- **AI-Powered Commands** - Natural language file operations via Claude AI
- **File Operations** - Copy, Move, Delete, Rename, Create directories
- **Built-in Viewer/Editor** - View and edit files without leaving the app
- **File Search** - Find files by name, size, date with advanced filters
- **Process Manager** - View and manage system processes
- **System Info** - Display system information and disk usage
- **Keyboard-Driven** - Full keyboard navigation for power users

## Installation

```bash
npm install -g cokacdir
```

## Usage

```bash
cokacdir
```

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `Tab` | Switch between panels |
| `↑` `↓` | Navigate files |
| `Enter` | Open directory / Execute file |
| `Home` | Go to first item |
| `End` | Go to last item |
| `PageUp` / `PageDown` | Scroll page |
| `Backspace` | Go to parent directory |

### File Operations

| Key | Action |
|-----|--------|
| `F1` | Help |
| `F3` | View file |
| `F4` | Edit file |
| `F5` | Copy |
| `F6` | Move |
| `F7` | Create directory |
| `F8` | Delete |
| `F9` | Rename |
| `F10` | Quit |

### Selection & Search

| Key | Action |
|-----|--------|
| `Space` | Select/Deselect file |
| `*` | Invert selection |
| `/` | Quick search |
| `Ctrl+F` | Advanced search |
| `Ctrl+I` | File info |

### AI Commands

| Key | Action |
|-----|--------|
| `Ctrl+A` | Open AI command modal |

Type natural language commands like:
- "delete all .tmp files"
- "move images to photos folder"
- "find files larger than 10MB"

### Other

| Key | Action |
|-----|--------|
| `Ctrl+P` | Process manager |
| `Ctrl+R` | Refresh panels |
| `=` | Sync panels (same directory) |

## Requirements

- Node.js >= 18.0.0
- Terminal with Unicode support
- (Optional) Claude Code for AI features

## Configuration

### AI Features

To enable AI-powered commands, install Claude CLI:

```bash
npm install -g @anthropic-ai/claude-cli
claude login
```

## Screenshots

### Dual Panel View
```
┌─ /home/user ──────────────────┐┌─ /home/user/projects ─────────┐
│ ..                            ││ ..                            │
│ Documents/              <DIR> ││ cokacdir/               <DIR> │
│ Downloads/              <DIR> ││ website/                <DIR> │
│ Pictures/               <DIR> ││ README.md              1.2 KB │
│ config.json            512 B  ││ package.json           2.1 KB │
└───────────────────────────────┘└───────────────────────────────┘
 1Help 2     3View 4Edit 5Copy 6Move 7Mkdir 8Del  9Ren  10Quit
```

## Development

```bash
# Clone repository
git clone https://github.com/kstost/cokacdir.git
cd cokacdir

# Install dependencies
npm install

# Run in development mode
npm run dev

# Build
npm run build

# Run tests
npm test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Disclaimer

**THIS SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.**

**IN NO EVENT SHALL THE AUTHORS, COPYRIGHT HOLDERS, OR CONTRIBUTORS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.**

This includes, without limitation:
- Data loss or corruption
- System damage or malfunction
- Security breaches or vulnerabilities
- Financial losses
- Any direct, indirect, incidental, special, exemplary, or consequential damages

**The user assumes full responsibility for all consequences arising from the use of this software, regardless of whether such use was intended, authorized, or anticipated.**

**USE AT YOUR OWN RISK.**


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

**cokac** - [https://cokacdir.cokac.com](https://cokacdir.cokac.com)

## Links

- [Homepage](https://cokacdir.cokac.com)
- [npm Package](https://www.npmjs.com/package/cokacdir)
- [GitHub Repository](https://github.com/kstost/cokacdir)
- [Issue Tracker](https://github.com/kstost/cokacdir/issues)
