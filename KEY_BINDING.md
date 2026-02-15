# Key Binding System

## Overview

`src/keybindings.rs` provides a customizable, modular key binding system. Users can override any shortcut via `~/.cokacdir/settings.json`, and developers can extend the system to new screen contexts (Viewer, Editor, etc.) by reusing the generic `ActionMap<A>` infrastructure.

## Architecture

```
keybindings.rs
├── Generic infrastructure (shared by all contexts)
│   ├── KeyBind              Key code + modifiers
│   ├── ActionMap<A>         Generic reverse-lookup map (KeyBind → A)
│   │   ├── build()          Merge defaults + user overrides → ActionMap
│   │   ├── lookup()         Key event → Option<A> (with SHIFT fallback)
│   │   ├── keys()           Action → display strings (e.g. ["Ctrl+C"])
│   │   ├── first_key()      Action → first display key (e.g. "Ctrl+C")
│   │   └── keys_joined()    Action → joined string (e.g. "Ctrl+C / Shift+V")
│   ├── parse_key()          String → Vec<KeyBind>
│   └── format_key_display() String → display string (e.g. "ctrl+c" → "Ctrl+C")
│
├── FilePanel context
│   ├── PanelAction        Enum of all FilePanel actions
│   └── default_panel_keybindings()
│
└── Container
    ├── KeybindingsConfig  JSON serialization (per-context HashMap)
    └── Keybindings        Runtime container (per-context ActionMap)
```

### Core Components

#### `ActionMap<A>`

Generic reverse-lookup map parameterized by an action enum `A`.

- **`build(defaults, overrides)`** - Merges user overrides on top of defaults, then parses all key strings into `KeyBind` entries. Actions present in overrides completely replace the default bindings for that action; unspecified actions keep defaults.
- **`lookup(code, modifiers)`** - Looks up an action for a key event. Tries exact match first, then falls back by stripping SHIFT for Char keys (handles terminal variance for shifted symbols and uppercase letters).

#### `parse_key()`

Parses a key string into one or more `KeyBind` values. Alphabetic characters always produce both lowercase and uppercase variants for case-insensitive matching.

#### `KeybindingsConfig`

JSON-serializable struct with one field per context. Each field uses `#[serde(default)]` so missing fields fall back to defaults.

#### `Keybindings`

Runtime container built from `KeybindingsConfig`. Holds one `ActionMap` per context and exposes typed lookup methods (`panel_action()`, etc.).

## Key String Format

### Modifiers

Modifiers are joined with `+` before the key name. Order does not matter.

| Modifier | Aliases |
|----------|---------|
| `ctrl` | `control` |
| `shift` | |
| `alt` | |

### Key Names

| Category | Keys |
|----------|------|
| Arrows | `up`, `down`, `left`, `right` |
| Navigation | `home`, `end`, `pageup`, `pagedown` |
| Action | `enter` (`return`), `esc` (`escape`), `tab`, `space`, `backspace`, `delete` (`del`) |
| Function | `f1` ~ `f12` |
| Characters | Single character: `a`-`z`, `0`-`9`, `*`, `;`, `/`, `.`, `` ` ``, `'`, etc. |

### Examples

| String | Meaning |
|--------|---------|
| `q` | Q key (case-insensitive, matches both `q` and `Q`) |
| `ctrl+c` | Ctrl+C (case-insensitive) |
| `shift+up` | Shift + Up arrow |
| `ctrl+shift+a` | Ctrl+Shift+A |
| `alt+ctrl+x` | Alt+Ctrl+X |
| `ctrl+shift+1` | Ctrl+Shift+1 |
| `f5` | F5 key |
| `*` | Asterisk |

### Case Handling

- Alphabetic keys are always case-insensitive. `"q"` matches both `q` and `Q` regardless of modifiers.
- This applies to all modifier combinations: `"ctrl+c"` matches `Ctrl+c` and `Ctrl+C` (e.g., Caps Lock).
- The SHIFT fallback in `lookup()` additionally handles terminals that report SHIFT for uppercase letters or shifted symbols.

## User Configuration

Add a `keybindings` section to `~/.cokacdir/settings.json`:

```json
{
  "keybindings": {
    "file_panel": {
      "quit": ["//프로그램 종료", "ctrl+q"],
      "delete": ["//파일 삭제", "ctrl+d", "delete"],
      "copy": ["ctrl+c", "ctrl+shift+c"]
    }
  }
}
```

### Comments

JSON does not support comments. To add descriptions to key bindings, use strings starting with `//` inside the key array. These are ignored by the parser and serve as inline documentation.

```json
"quit": ["//Exit the application", "q", "ctrl+q"]
```

### Merge Behavior

- User config is merged on top of defaults per action.
- Only the actions you specify are overridden; everything else keeps the default bindings.
- When you override an action, you replace **all** its default keys. For example, setting `"quit": ["ctrl+q"]` removes the default `q` binding for quit.
- Multiple keys per action are supported via the array.

### Default FilePanel Bindings

| Action | Default Keys |
|--------|-------------|
| `quit` | `q` |
| `move_up` | `up` |
| `move_down` | `down` |
| `page_up` | `pageup` |
| `page_down` | `pagedown` |
| `go_home` | `home` |
| `go_end` | `end` |
| `open` | `enter` |
| `parent_dir` | `esc` |
| `switch_panel` | `tab` |
| `switch_panel_left` | `left` |
| `switch_panel_right` | `right` |
| `toggle_select` | `space` |
| `select_all` | `*`, `ctrl+a` |
| `select_by_extension` | `;` |
| `select_up` | `shift+up` |
| `select_down` | `shift+down` |
| `copy` | `ctrl+c` |
| `cut` | `ctrl+x` |
| `paste` | `ctrl+v`, `shift+v` |
| `sort_by_name` | `n` |
| `sort_by_type` | `y` |
| `sort_by_size` | `s` |
| `sort_by_date` | `d` |
| `help` | `h` |
| `file_info` | `i` |
| `edit` | `e` |
| `mkdir` | `k` |
| `mkfile` | `m` |
| `delete` | `x`, `delete`, `backspace` |
| `process_manager` | `p` |
| `rename` | `r` |
| `tar` | `t` |
| `search` | `f` |
| `go_to_path` | `/` |
| `add_panel` | `0` |
| `go_home_dir` | `1` |
| `refresh` | `2` |
| `git_log_diff` | `7` |
| `start_diff` | `8` |
| `close_panel` | `9` |
| `ai_screen` | `.` |
| `settings` | `` ` `` |
| `git_screen` | `g` |
| `toggle_bookmark` | `'` |
| `set_handler` | `u` |
| `open_in_finder` | `o` (macOS only) |
| `open_in_vscode` | `c` (macOS only) |

## Adding a New Context (Developer Guide)

To add keybindings for a new screen (e.g., Viewer):

### 1. Define the action enum

```rust
// in keybindings.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerAction {
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    SearchForward,
    SearchBackward,
    NextMatch,
    PrevMatch,
    GoToTop,
    GoToBottom,
    Quit,
}
```

### 2. Define default bindings

```rust
pub fn default_viewer_keybindings() -> HashMap<ViewerAction, Vec<String>> {
    let mut m = HashMap::new();
    m.insert(ViewerAction::ScrollUp, vec!["up".into()]);
    m.insert(ViewerAction::ScrollDown, vec!["down".into()]);
    m.insert(ViewerAction::Quit, vec!["q".into(), "esc".into()]);
    // ...
    m
}
```

### 3. Add to KeybindingsConfig

```rust
pub struct KeybindingsConfig {
    #[serde(default = "default_panel_keybindings")]
    pub file_panel: HashMap<PanelAction, Vec<String>>,

    #[serde(default = "default_viewer_keybindings")]
    pub viewer: HashMap<ViewerAction, Vec<String>>,
}
```

### 4. Add to Keybindings

```rust
pub struct Keybindings {
    panel: ActionMap<PanelAction>,
    viewer: ActionMap<ViewerAction>,
}

impl Keybindings {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        Self {
            panel: ActionMap::build(
                &default_panel_keybindings(),
                &config.file_panel,
            ),
            viewer: ActionMap::build(
                &default_viewer_keybindings(),
                &config.viewer,
            ),
        }
    }

    pub fn panel_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<PanelAction> {
        self.panel.lookup(code, modifiers)
    }

    pub fn viewer_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<ViewerAction> {
        self.viewer.lookup(code, modifiers)
    }
}
```

### 5. Use in the input handler

```rust
if let Some(action) = app.keybindings.viewer_action(code, modifiers) {
    match action {
        ViewerAction::ScrollUp => { /* ... */ }
        ViewerAction::Quit => { /* ... */ }
        // ...
    }
}
```

The user can then customize it in `settings.json`:

```json
{
  "keybindings": {
    "viewer": {
      "quit": ["q", "esc", "ctrl+w"],
      "scroll_up": ["up", "k"]
    }
  }
}
```

## File Locations

| File | Role |
|------|------|
| `src/keybindings.rs` | Module: enums, ActionMap, parse_key, format_key_display, defaults, config, runtime container |
| `src/config.rs` | `Settings.keybindings: KeybindingsConfig` |
| `src/ui/app.rs` | `App.keybindings: Keybindings` (built from settings) |
| `src/main.rs` | `handle_panel_input()` uses `app.keybindings.panel_action()` |
| `src/ui/draw.rs` | Function bar uses `app.keybindings.panel_first_key()` for key display |
| `src/ui/help.rs` | Help screen uses `app.keybindings.panel_keys_joined()` for key display |
| `~/.cokacdir/settings.json` | User-editable keybinding overrides |
