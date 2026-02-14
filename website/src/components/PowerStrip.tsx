import { useEffect, useRef, useState } from 'react'
import { motion, useInView } from 'framer-motion'
import { Zap, HardDrive, Package } from 'lucide-react'

function AnimatedNumber({ value, suffix, inView }: { value: number; suffix: string; inView: boolean }) {
  const [display, setDisplay] = useState(0)

  useEffect(() => {
    if (!inView) return
    const start = performance.now()
    const duration = 1200

    function tick(now: number) {
      const elapsed = now - start
      const progress = Math.min(elapsed / duration, 1)
      // ease-out cubic
      const eased = 1 - Math.pow(1 - progress, 3)
      setDisplay(Math.round(eased * value))
      if (progress < 1) requestAnimationFrame(tick)
    }

    requestAnimationFrame(tick)
  }, [inView, value])

  return (
    <span className="text-accent-cyan text-glow">
      ~{display}{suffix}
    </span>
  )
}

const stats = [
  { icon: Zap, value: 10, suffix: 'ms', label: 'startup' },
  { icon: HardDrive, value: 5, suffix: 'MB', label: 'memory' },
  { icon: Package, value: 4, suffix: 'MB', label: 'binary' },
]

export default function PowerStrip() {
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref, { once: true, margin: '-50px' })

  return (
    <section className="py-12 px-4">
      <div
        ref={ref}
        className="max-w-6xl mx-auto bg-bg-card/30 border border-accent-cyan/10 rounded-2xl py-10 px-6"
      >
        <div className="flex flex-col sm:flex-row items-center justify-center gap-8 sm:gap-16">
          {stats.map((stat, i) => (
            <motion.div
              key={i}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.5, delay: i * 0.15 }}
              className="flex items-center gap-3 text-center sm:text-left"
            >
              <stat.icon className="w-6 h-6 text-accent-cyan/60 shrink-0" />
              <div>
                <div className="text-3xl sm:text-4xl font-extrabold font-mono">
                  <AnimatedNumber value={stat.value} suffix={stat.suffix} inView={inView} />
                </div>
                <div className="text-zinc-400 text-sm">{stat.label}</div>
              </div>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  )
}
