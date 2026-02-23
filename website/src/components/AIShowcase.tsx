import { useEffect, useState, useRef } from 'react'
import { motion, AnimatePresence, useInView } from 'framer-motion'

const aiExamples = [
  { prompt: 'Find all files larger than 100MB', result: 'Found 3 files: backup.tar.gz (240MB), dataset.csv (180MB), video.mp4 (1.2GB)' },
  { prompt: 'Organize my photos by year', result: 'Moved 847 photos into 2022/, 2023/, 2024/, 2025/ folders' },
  { prompt: 'Delete all .tmp files in this project', result: 'Deleted 23 .tmp files (freed 12MB)' },
  { prompt: 'Compress the src folder into a tar', result: 'Created src.tar.gz (2.4MB) from 156 files' },
]

function useTypingAnimation(text: string, active: boolean, speed = 40) {
  const [displayed, setDisplayed] = useState('')

  useEffect(() => {
    if (!active) {
      setDisplayed('')
      return
    }
    setDisplayed('')
    let i = 0
    const interval = setInterval(() => {
      i++
      setDisplayed(text.slice(0, i))
      if (i >= text.length) clearInterval(interval)
    }, speed)
    return () => clearInterval(interval)
  }, [text, active, speed])

  return displayed
}

function TerminalDemo() {
  const [step, setStep] = useState(0) // 0=idle, 1=typing prompt, 2=showing result
  const [exampleIdx, setExampleIdx] = useState(0)
  const example = aiExamples[exampleIdx]
  const typedPrompt = useTypingAnimation(example.prompt, step >= 1, 35)
  const typedResult = useTypingAnimation(example.result, step >= 2, 20)

  useEffect(() => {
    // Cycle: idle(500ms) -> typing prompt -> wait(600ms) -> result -> wait(2500ms) -> next
    const timers: ReturnType<typeof setTimeout>[] = []

    timers.push(setTimeout(() => setStep(1), 500))
    timers.push(setTimeout(() => setStep(2), 500 + example.prompt.length * 35 + 600))
    timers.push(
      setTimeout(() => {
        setStep(0)
        setExampleIdx((prev) => (prev + 1) % aiExamples.length)
      }, 500 + example.prompt.length * 35 + 600 + example.result.length * 20 + 2500)
    )

    return () => timers.forEach(clearTimeout)
  }, [exampleIdx, example.prompt.length, example.result.length])

  return (
    <div className="relative overflow-hidden">
      <div className="absolute inset-0 bg-gradient-to-r from-accent-purple/40 via-primary/30 to-accent-purple/40 rounded-xl blur-lg opacity-40" />
      <div className="relative bg-bg-dark border border-zinc-700 rounded-xl overflow-hidden shadow-2xl">
        {/* Title bar */}
        <div className="flex items-center gap-2 px-4 py-3 bg-bg-card border-b border-zinc-800">
          <div className="flex gap-2" aria-hidden="true">
            <div className="w-3 h-3 rounded-full bg-red-500/80" />
            <div className="w-3 h-3 rounded-full bg-yellow-500/80" />
            <div className="w-3 h-3 rounded-full bg-green-500/80" />
          </div>
          <span className="text-xs text-zinc-500 ml-2 font-mono">aimi — AI Command</span>
        </div>

        {/* Terminal body */}
        <div className="p-3 sm:p-5 font-mono text-sm min-h-[140px] sm:min-h-[160px]">
          <div className="text-zinc-500 text-xs mb-3">Press <span className="text-accent-purple">.</span> to open AI prompt</div>

          {/* AI input line */}
          <div className="flex items-start gap-2 mb-3">
            <span className="text-accent-purple shrink-0">AI &gt;</span>
            <span className="text-white">
              {typedPrompt}
              {step === 1 && <span className="typing-cursor" />}
            </span>
          </div>

          {/* Result */}
          <AnimatePresence mode="wait">
            {step >= 2 && (
              <motion.div
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.3 }}
                className="ml-4 sm:ml-6 text-accent-green text-xs leading-relaxed"
              >
                {typedResult}
                {step === 2 && typedResult.length < example.result.length && <span className="typing-cursor" />}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  )
}

const bubbleExamples = [
  'Find all files larger than 100MB',
  'Organize my photos by year',
  'Delete all .tmp files in this project',
  'Compress the src folder into a tar',
  'Show disk usage by folder',
]

export default function AIShowcase() {
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref, { once: true, margin: '-100px' })

  return (
    <section className="py-12 sm:py-24 px-4 relative overflow-hidden" ref={ref}>
      {/* Purple tint background */}
      <div className="absolute inset-0 bg-gradient-to-b from-accent-purple/5 via-accent-purple/10 to-accent-purple/5 pointer-events-none" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[300px] h-[300px] sm:w-[600px] sm:h-[600px] bg-accent-purple/10 rounded-full blur-3xl pointer-events-none" />

      <div className="relative max-w-6xl mx-auto">
        {/* Header */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
          className="text-center mb-8 sm:mb-16"
        >
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            Just Press <kbd className="px-2 sm:px-3 py-1 bg-accent-purple/20 border border-accent-purple/40 rounded-lg text-accent-purple font-mono">.</kbd> and Ask
          </h2>
          <p className="text-zinc-400 text-sm sm:text-lg max-w-2xl mx-auto">
            Natural language file operations powered by AI. Describe what you want — done.
          </p>
        </motion.div>

        {/* 2-column layout */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 sm:gap-10 items-center">
          {/* Left: Terminal demo */}
          <motion.div
            initial={{ opacity: 0, x: -30 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.7 }}
          >
            <TerminalDemo />
          </motion.div>

          {/* Right: Example bubbles */}
          <div className="space-y-3 sm:space-y-4">
            {bubbleExamples.map((text, i) => (
              <motion.div
                key={i}
                initial={{ opacity: 0, x: 30 }}
                animate={inView ? { opacity: 1, x: 0 } : {}}
                transition={{ duration: 0.5, delay: 0.2 + i * 0.12 }}
                className="flex items-center gap-3"
              >
                <div className="shrink-0 w-8 h-8 rounded-full bg-accent-purple/20 border border-accent-purple/30 flex items-center justify-center">
                  <span className="text-accent-purple text-sm">AI</span>
                </div>
                <div className="bg-bg-card border border-zinc-800 rounded-2xl rounded-tl-sm px-4 py-3 text-sm text-zinc-300">
                  "{text}"
                </div>
              </motion.div>
            ))}

            {/* Powered by badge */}
            <motion.div
              initial={{ opacity: 0 }}
              whileInView={{ opacity: 1 }}
              viewport={{ once: true }}
              transition={{ duration: 0.6, delay: 0.8 }}
              className="pt-4"
            >
              <span className="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-accent-purple/10 border border-accent-purple/20 text-sm text-accent-purple">
                Powered by Claude AI
              </span>
            </motion.div>
          </div>
        </div>
      </div>
    </section>
  )
}
