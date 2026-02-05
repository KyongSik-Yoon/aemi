import { motion } from 'framer-motion'
import Card from './ui/Card'

const shortcutGroups = [
  {
    title: 'Navigation',
    shortcuts: [
      { key: '↑ ↓', action: 'Navigate files' },
      { key: 'Enter', action: 'Open directory / file' },
      { key: 'Esc', action: 'Parent directory' },
      { key: 'Tab / ← →', action: 'Switch panels' },
      { key: 'Home / End', action: 'First / Last item' },
      { key: 'PgUp / PgDn', action: 'Move 10 lines' },
      { key: '/', action: 'Go to path / Search bookmarks' },
      { key: "'", action: 'Toggle bookmark' },
      { key: '1 / 2', action: 'Home / Refresh' },
      { key: 'q', action: 'Quit' },
    ],
  },
  {
    title: 'File Operations',
    shortcuts: [
      { key: 'k', action: 'Create directory' },
      { key: 'm', action: 'Create file' },
      { key: 'r', action: 'Rename' },
      { key: 'x / Del', action: 'Delete' },
      { key: 't', action: 'Create tar archive' },
      { key: 'f', action: 'Find / Search files' },
      { key: 'u', action: 'Set/Edit file handler' },
    ],
  },
  {
    title: 'Selection',
    shortcuts: [
      { key: 'Space', action: 'Select / Deselect file' },
      { key: 'Shift+↑↓', action: 'Select while moving' },
      { key: '*', action: 'Toggle select all' },
      { key: ';', action: 'Select by extension' },
      { key: 'Ctrl+A', action: 'Select all files' },
    ],
  },
  {
    title: 'Clipboard',
    shortcuts: [
      { key: 'Ctrl+C', action: 'Copy to clipboard' },
      { key: 'Ctrl+X', action: 'Cut to clipboard' },
      { key: 'Ctrl+V / Shift+V', action: 'Paste from clipboard' },
    ],
  },
  {
    title: 'Sorting',
    shortcuts: [
      { key: 'n', action: 'Sort by name' },
      { key: 's', action: 'Sort by size' },
      { key: 'd', action: 'Sort by date' },
      { key: 'y', action: 'Sort by type' },
    ],
  },
  {
    title: 'View & Tools',
    shortcuts: [
      { key: 'h', action: 'Help' },
      { key: 'i', action: 'File info' },
      { key: 'e', action: 'Edit file' },
      { key: 'p', action: 'Process manager' },
      { key: '`', action: 'Settings' },
    ],
  },
  {
    title: 'AI Screen',
    shortcuts: [
      { key: '.', action: 'Open AI command' },
      { key: 'Enter', action: 'Send message' },
      { key: 'Shift+Enter', action: 'New line' },
      { key: 'Ctrl+↑↓', action: 'Scroll history' },
      { key: 'PgUp / PgDn', action: 'Page scroll' },
      { key: 'Esc', action: 'Close AI screen' },
    ],
  },
  {
    title: 'macOS Only',
    shortcuts: [
      { key: 'o', action: 'Open in Finder' },
      { key: 'c', action: 'Open in VS Code' },
    ],
  },
  {
    title: 'File Viewer',
    shortcuts: [
      { key: 'Ctrl+F / /', action: 'Search in file' },
      { key: 'Ctrl+G', action: 'Go to line' },
      { key: 'b / [ / ]', action: 'Bookmark / Prev / Next' },
      { key: 'H', action: 'Toggle hex mode' },
      { key: 'W', action: 'Toggle word wrap' },
      { key: 'E', action: 'Open in editor' },
      { key: 'Esc / Q', action: 'Close viewer' },
    ],
  },
  {
    title: 'File Editor',
    shortcuts: [
      { key: 'Ctrl+S', action: 'Save' },
      { key: 'Ctrl+Z / Y', action: 'Undo / Redo' },
      { key: 'Ctrl+C / X / V', action: 'Copy / Cut / Paste' },
      { key: 'Ctrl+K', action: 'Delete line' },
      { key: 'Ctrl+J', action: 'Duplicate line' },
      { key: 'Ctrl+/', action: 'Toggle comment' },
      { key: 'Ctrl+F / H', action: 'Find / Replace' },
      { key: 'Ctrl+G', action: 'Go to line' },
      { key: 'Alt+↑↓', action: 'Move line up/down' },
      { key: 'Esc', action: 'Close editor' },
    ],
  },
]

export default function Shortcuts() {
  return (
    <section className="py-24 px-4" id="shortcuts">
      <div className="max-w-6xl mx-auto">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
          className="text-center mb-16"
        >
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">Keyboard Shortcuts</span>
          </h2>
          <p className="text-zinc-400 text-lg max-w-2xl mx-auto">
            Master these shortcuts and navigate at the speed of thought.
          </p>
        </motion.div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {shortcutGroups.map((group, groupIndex) => (
            <motion.div
              key={groupIndex}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.5, delay: groupIndex * 0.1 }}
            >
              <Card>
                <h3 className="text-lg font-semibold mb-4 text-accent-cyan">
                  {group.title}
                </h3>
                <div className="space-y-2">
                  {group.shortcuts.map((shortcut, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between py-2 border-b border-zinc-800 last:border-0"
                    >
                      <kbd className="px-2 py-1 bg-bg-elevated rounded text-sm font-mono text-white">
                        {shortcut.key}
                      </kbd>
                      <span className="text-zinc-400 text-sm">{shortcut.action}</span>
                    </div>
                  ))}
                </div>
              </Card>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  )
}
