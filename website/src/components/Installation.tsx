import { motion } from 'framer-motion'
import { Zap, Apple, Monitor } from 'lucide-react'
import CodeBlock from './ui/CodeBlock'

export default function Installation() {
  return (
    <section className="py-24 px-4 bg-bg-card/30" id="install">
      <div className="max-w-3xl mx-auto">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
          className="text-center mb-12"
        >
          <div className="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-accent-green/10 border border-accent-green/20 mb-6">
            <Zap className="w-4 h-4 text-accent-green" />
            <span className="text-sm text-accent-green">Quick Start</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            Get Started in <span className="text-accent-cyan">Seconds</span>
          </h2>
          <p className="text-zinc-400 text-lg">
            One command installation. No dependencies required.
          </p>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6, delay: 0.1 }}
          className="space-y-4"
        >
          <CodeBlock code={`/bin/bash -c "$(curl -fsSL https://cokacdir.cokac.com/install.sh)"`} />
          <CodeBlock code="cokacdir" />
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6, delay: 0.2 }}
          className="flex items-center justify-center gap-6 mt-8 text-zinc-500"
        >
          <div className="flex items-center gap-2">
            <Apple className="w-4 h-4" />
            <span className="text-sm">macOS</span>
          </div>
          <div className="flex items-center gap-2">
            <Monitor className="w-4 h-4" />
            <span className="text-sm">Linux</span>
          </div>
        </motion.div>

        {/* Optional AI setup */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6, delay: 0.3 }}
          className="mt-12 p-6 rounded-xl border border-zinc-800 bg-bg-card"
        >
          <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
            <span className="text-accent-purple">✨</span>
            Enable AI Commands (Optional)
          </h3>
          <p className="text-zinc-400 text-sm mb-4">
            Install Claude Code to unlock natural language file operations.
          </p>
          <div className="space-y-3">
            <CodeBlock code="npm install -g @anthropic-ai/claude-code" />
          </div>
          <p className="text-zinc-500 text-xs mt-4">
            Learn more at <a href="https://docs.anthropic.com/en/docs/claude-code" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline">docs.anthropic.com</a>
          </p>
        </motion.div>

        {/* Custom File Handlers */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6, delay: 0.4 }}
          className="mt-6 p-6 rounded-xl border border-zinc-800 bg-bg-card"
        >
          <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
            <span className="text-accent-cyan">⚙️</span>
            Custom File Handlers (Optional)
          </h3>
          <p className="text-zinc-400 text-sm mb-4">
            Define custom programs for each file extension in <code className="text-accent-green">~/.cokacdir/settings.json</code>
          </p>
          <div className="bg-bg-elevated rounded-lg p-4 font-mono text-sm overflow-x-auto">
            <pre className="text-zinc-300">{`{
  "extension_handler": {
    "jpg|jpeg|png|gif": ["feh {{FILEPATH}}"],
    "mp4|avi|mkv": ["vlc {{FILEPATH}}"],
    "pdf": ["evince {{FILEPATH}}"],
    "rs|py|js": ["@vim {{FILEPATH}}"]
  }
}`}</pre>
          </div>
          <div className="mt-4 space-y-2 text-xs text-zinc-500">
            <p><code className="text-accent-cyan">|</code> — Combine multiple extensions</p>
            <p><code className="text-accent-cyan">@</code> — Terminal mode for TUI apps (vim, nano)</p>
            <p><code className="text-accent-cyan">["cmd1", "cmd2"]</code> — Fallback commands</p>
          </div>
        </motion.div>
      </div>
    </section>
  )
}
