import { motion } from 'framer-motion'
import { BookOpen, Cloud, Apple, Send, Zap } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useLanguage } from './tutorial/LanguageContext'

function LanguageToggle() {
  const { lang, setLang } = useLanguage()
  return (
    <div className="flex items-center border border-zinc-700 rounded-md overflow-hidden text-xs">
      <button
        onClick={() => setLang('en')}
        className={`px-2 py-1 font-semibold transition-colors ${
          lang === 'en'
            ? 'bg-accent-cyan/20 text-accent-cyan'
            : 'text-zinc-500 hover:text-zinc-300'
        }`}
      >
        EN
      </button>
      <div className="w-px h-4 bg-zinc-700" />
      <button
        onClick={() => setLang('ko')}
        className={`px-2 py-1 font-semibold transition-colors ${
          lang === 'ko'
            ? 'bg-accent-cyan/20 text-accent-cyan'
            : 'text-zinc-500 hover:text-zinc-300'
        }`}
      >
        KO
      </button>
    </div>
  )
}

function ChatBubble({ from, children, delay }: { from: 'user' | 'bot'; children: React.ReactNode; delay: number }) {
  const isUser = from === 'user'
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay }}
      className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}
    >
      <div
        className={`max-w-[80%] px-4 py-2.5 rounded-2xl text-sm leading-relaxed ${
          isUser
            ? 'bg-accent-cyan/20 border border-accent-cyan/30 text-zinc-200 rounded-br-sm'
            : 'bg-bg-card border border-zinc-700 text-zinc-300 rounded-bl-sm'
        }`}
      >
        {children}
      </div>
    </motion.div>
  )
}

function HeroChatDemo() {
  const { t } = useLanguage()
  return (
    <div className="relative max-w-lg mx-auto">
      <div className="absolute inset-0 bg-gradient-to-r from-accent-cyan/20 via-primary/10 to-accent-cyan/20 rounded-xl blur-lg opacity-30" />
      <div className="relative bg-bg-dark border border-zinc-700 rounded-xl overflow-hidden shadow-2xl">
        {/* Title bar */}
        <div className="flex items-center gap-3 px-4 py-3 bg-bg-card border-b border-zinc-800">
          <div className="w-8 h-8 rounded-full bg-accent-cyan/20 flex items-center justify-center">
            <Send className="w-4 h-4 text-accent-cyan" />
          </div>
          <div>
            <div className="text-sm text-white font-medium">aimi Bot</div>
            <div className="text-xs text-zinc-500">online</div>
          </div>
        </div>

        {/* Chat body */}
        <div className="p-4 space-y-3">
          <ChatBubble from="user" delay={0.6}>
            <code className="text-accent-cyan font-mono text-xs">/start ~/my-project</code>
          </ChatBubble>
          <ChatBubble from="bot" delay={0.8}>
            {t(
              <>Session started at <code className="text-accent-cyan font-mono text-xs">/home/user/my-project</code></>,
              <><code className="text-accent-cyan font-mono text-xs">/home/user/my-project</code>에서 세션이 시작되었습니다</>
            )}
          </ChatBubble>
          <ChatBubble from="user" delay={1.0}>
            {t(
              'Find all TODO comments and create a summary',
              'TODO 주석을 모두 찾아서 요약해줘'
            )}
          </ChatBubble>
          <ChatBubble from="bot" delay={1.2}>
            <span className="text-zinc-400">{t('Searching files...', '파일 검색 중...')}</span>
            <div className="mt-2 text-zinc-300">
              {t(
                <>Found <strong className="text-white">12 TODOs</strong> across 5 files.</>,
                <>5개 파일에서 <strong className="text-white">12개의 TODO</strong>를 찾았습니다.</>
              )}
            </div>
            <div className="mt-1 font-mono text-xs text-accent-cyan">
              src/main.rs:42 — TODO: add error handling<br />
              src/api.rs:18 — TODO: implement caching
            </div>
          </ChatBubble>
        </div>
      </div>
    </div>
  )
}

export default function Hero() {
  const { t } = useLanguage()

  return (
    <section className="relative flex flex-col items-center justify-center px-4 py-12 sm:py-20 sm:min-h-screen overflow-hidden">
      {/* Language toggle - top right */}
      <div className="absolute top-4 right-4 z-20">
        <LanguageToggle />
      </div>

      {/* Animated grid background */}
      <div className="absolute inset-0 grid-background opacity-50" />

      {/* Gradient orbs */}
      <div className="absolute top-1/4 left-1/4 w-48 h-48 sm:w-96 sm:h-96 bg-primary/20 rounded-full blur-3xl animate-glow-pulse" />
      <div className="absolute bottom-1/4 right-1/4 w-48 h-48 sm:w-96 sm:h-96 bg-accent-cyan/20 rounded-full blur-3xl animate-glow-pulse" style={{ animationDelay: '2s' }} />

      <div className="relative z-10 w-full max-w-6xl mx-auto text-center">
        {/* Main title */}
        <motion.h1
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.1 }}
          className="text-4xl sm:text-5xl md:text-6xl lg:text-7xl font-extrabold mb-4 sm:mb-6"
        >
          <span className="gradient-text">aimi</span>
        </motion.h1>

        {/* Tagline */}
        <motion.p
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.2 }}
          className="text-xl sm:text-2xl md:text-3xl lg:text-4xl font-bold text-white mb-3 sm:mb-4"
        >
          {t(
            <>Claude Code, <span className="text-glow text-accent-cyan">Anywhere</span></>,
            <>Claude Code, <span className="text-glow text-accent-cyan">어디서든</span></>
          )}
        </motion.p>

        {/* Sub-tagline */}
        <motion.p
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.25 }}
          className="text-sm sm:text-base md:text-lg text-zinc-400 mb-8 sm:mb-12 px-2 max-w-2xl mx-auto"
        >
          {t(
            <>Run Claude Code autonomously in a virtual cloud computer.<br />Control everything from Telegram — maximum convenience, maximum safety.</>,
            <>가상 클라우드 컴퓨터 안에서 Claude Code를 자율적으로 실행하고,<br />텔레그램으로 모든 것을 제어하세요 — 편리함은 극대화, 안전함까지.</>
          )}
        </motion.p>

        {/* Chat preview */}
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 1, delay: 0.4 }}
          className="mb-8 sm:mb-14"
        >
          <HeroChatDemo />
        </motion.div>

        {/* CTA buttons */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.5 }}
          className="flex flex-col sm:flex-row gap-4 justify-center items-center mb-6 sm:mb-8"
        >
          <Link
            to="/ec2"
            className="inline-flex items-center gap-2 px-6 py-3 sm:px-8 sm:py-4 rounded-lg bg-accent-cyan text-bg-dark font-bold text-base sm:text-lg hover:bg-accent-cyan/90 shadow-lg shadow-accent-cyan/25 transition-all duration-200"
          >
            <Cloud className="w-5 h-5" />
            {t('EC2 Setup', 'EC2 설정')}
          </Link>
          <Link
            to="/macos"
            className="inline-flex items-center gap-2 px-6 py-3 sm:px-8 sm:py-4 rounded-lg bg-accent-purple text-white font-bold text-base sm:text-lg hover:bg-accent-purple/90 shadow-lg shadow-accent-purple/25 transition-all duration-200"
          >
            <Apple className="w-5 h-5" />
            {t('macOS Setup', 'macOS 설정')}
          </Link>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.6 }}
          className="flex flex-wrap justify-center gap-3 mb-8 sm:mb-16"
        >
          <Link
            to="/workflows"
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-accent-cyan/30 bg-accent-cyan/5 text-accent-cyan font-medium text-sm hover:bg-accent-cyan/10 hover:border-accent-cyan/50 transition-all duration-200"
          >
            <Zap className="w-4 h-4" />
            {t('See Workflows', '워크플로우 보기')}
          </Link>
          <Link
            to="/tutorial"
            className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg border border-zinc-700 text-zinc-400 font-medium text-sm hover:text-white hover:border-zinc-500 transition-all duration-200"
          >
            <BookOpen className="w-4 h-4" />
            {t('Beginner Tutorial', '초보자 튜토리얼')}
          </Link>
        </motion.div>

      </div>

      {/* Scroll indicator */}
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 1.5 }}
        className="absolute bottom-8 left-1/2 -translate-x-1/2"
        aria-hidden="true"
      >
        <motion.div
          animate={{ y: [0, 8, 0] }}
          transition={{ duration: 1.5, repeat: Infinity }}
          className="w-6 h-10 border-2 border-zinc-600 rounded-full flex justify-center pt-2"
        >
          <div className="w-1.5 h-1.5 bg-accent-cyan rounded-full" />
        </motion.div>
      </motion.div>
    </section>
  )
}
