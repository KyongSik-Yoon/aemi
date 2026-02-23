import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { motion } from 'framer-motion'
import { ArrowLeft, Github, Apple, Bot, Terminal, Copy, Check, Rocket, MessageCircle, RefreshCw } from 'lucide-react'
import { LanguageProvider, useLanguage } from '../tutorial/LanguageContext'

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

function Hl({ children }: { children: React.ReactNode }) {
  return <span className="text-yellow-400 bg-yellow-400/10 rounded px-0.5">{children}</span>
}

function CopyBlock({ code, label, children }: { code: string; label?: string; children?: React.ReactNode }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {}
  }
  return (
    <div className="my-4">
      {label && <p className="text-xs text-zinc-500 mb-1 font-mono">{label}</p>}
      <div className="relative group bg-bg-card border border-zinc-800 rounded-lg overflow-hidden hover:border-accent-cyan/30 transition-colors">
        <div className="flex items-start justify-between px-4 py-3 gap-2">
          <pre className="overflow-x-auto min-w-0 flex-1 font-mono text-accent-cyan text-xs sm:text-sm whitespace-pre leading-relaxed">
            {children || code}
          </pre>
          <button
            onClick={handleCopy}
            className="p-2 rounded-md bg-bg-elevated hover:bg-zinc-700 transition-colors shrink-0 mt-0.5"
            aria-label="Copy to clipboard"
          >
            {copied ? (
              <Check className="w-4 h-4 text-accent-green" />
            ) : (
              <Copy className="w-4 h-4 text-zinc-400 group-hover:text-white" />
            )}
          </button>
        </div>
      </div>
    </div>
  )
}

function SectionCard({ icon: Icon, title, step, children }: {
  icon: typeof Terminal
  title: string
  step: number
  children: React.ReactNode
}) {
  return (
    <motion.section
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ duration: 0.5 }}
      className="mb-10"
    >
      <div className="flex items-center gap-3 mb-5">
        <div className="flex-shrink-0 w-10 h-10 rounded-full bg-accent-cyan/20 border border-accent-cyan/50 flex items-center justify-center text-accent-cyan font-bold text-lg">
          {step}
        </div>
        <div className="flex items-center gap-2">
          <Icon className="w-5 h-5 text-accent-cyan" />
          <h2 className="text-xl sm:text-2xl font-bold text-white">{title}</h2>
        </div>
      </div>
      <div className="ml-0 sm:ml-[52px] text-zinc-300 text-sm sm:text-base leading-relaxed space-y-4">
        {children}
      </div>
    </motion.section>
  )
}

function InlineStep({ n, children }: { n: number; children: React.ReactNode }) {
  return (
    <div className="flex gap-3 py-2">
      <span className="flex-shrink-0 w-6 h-6 rounded-full bg-accent-purple/20 border border-accent-purple/40 flex items-center justify-center text-accent-purple font-bold text-xs">
        {n}
      </span>
      <div className="text-zinc-300 text-sm sm:text-base leading-relaxed">{children}</div>
    </div>
  )
}

function MacOSPageInner() {
  const navigate = useNavigate()
  const { t } = useLanguage()

  useEffect(() => {
    window.scrollTo(0, 0)
  }, [])

  return (
    <div className="min-h-screen bg-bg-dark">
      {/* Top navigation bar */}
      <header className="fixed top-0 left-0 right-0 z-30 bg-bg-dark/80 backdrop-blur-md border-b border-zinc-800">
        <div className="max-w-4xl mx-auto px-4 h-16 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link
              to="/"
              className="flex items-center gap-2 text-zinc-400 hover:text-white transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
              <span className="text-sm">Home</span>
            </Link>
            <div className="hidden sm:block h-5 w-px bg-zinc-700" />
            <Link to="/" className="hidden sm:block">
              <span className="gradient-text font-bold text-lg">aimi</span>
            </Link>
          </div>

          <div className="flex items-center gap-3">
            <LanguageToggle />
            <span className="text-white font-semibold flex items-center gap-2">
              <Apple className="w-4 h-4 text-accent-cyan" />
              <span className="hidden sm:inline">macOS Setup Guide</span>
            </span>
          </div>

          <a
            href="https://github.com/kstost/aimi"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 text-zinc-400 hover:text-white transition-colors"
          >
            <Github className="w-4 h-4" />
            <span className="text-sm hidden sm:inline">GitHub</span>
          </a>
        </div>
      </header>

      {/* Main content */}
      <main className="max-w-4xl mx-auto px-4 pt-24 pb-16">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
        >
          {/* Page title */}
          <div className="mb-12 text-center">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-accent-cyan/10 border border-accent-cyan/30 text-accent-cyan text-sm font-medium mb-4">
              <Apple className="w-4 h-4" />
              macOS
            </div>
            <h1 className="text-3xl sm:text-4xl lg:text-5xl font-extrabold text-white mb-4">
              {t('aimi on macOS', 'macOS에서 aimi')}
              <br />
              <span className="gradient-text">{t('Setup Guide', '셋업 가이드')}</span>
            </h1>
            <p className="text-lg text-zinc-400 leading-relaxed max-w-2xl mx-auto">
              {t(
                'Install aimi on a macOS with Claude Code and use it anywhere via Telegram bot.',
                'Claude Code가 설치된 macOS에 aimi을 설치하고, 텔레그램 봇으로 어디서나 사용하는 가이드입니다.'
              )}
            </p>
          </div>

          {/* Prerequisites */}
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            className="mb-12 p-5 rounded-xl border border-accent-purple/30 bg-accent-purple/5"
          >
            <h3 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
              <Rocket className="w-5 h-5 text-accent-purple" />
              {t('2 Prerequisites', '준비물 2가지')}
            </h3>
            <div className="grid sm:grid-cols-2 gap-3">
              <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-card border border-zinc-800">
                <Apple className="w-5 h-5 text-accent-cyan flex-shrink-0" />
                <div>
                  <p className="text-white font-medium text-sm">{t('macOS with Claude Code', 'Claude Code 설치된 macOS')}</p>
                  <p className="text-zinc-500 text-xs">{t('Terminal accessible', '터미널 사용 가능')}</p>
                </div>
              </div>
              <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-card border border-zinc-800">
                <Bot className="w-5 h-5 text-accent-cyan flex-shrink-0" />
                <div>
                  <p className="text-white font-medium text-sm">{t('Telegram Bot Token', '텔레그램 봇 토큰')}</p>
                  <p className="text-zinc-500 text-xs">{t('Issued by BotFather', 'BotFather 발급')}</p>
                </div>
              </div>
            </div>
          </motion.div>

          {/* Step 1: Install aimi */}
          <SectionCard icon={Terminal} title={t('Install aimi', 'aimi 설치')} step={1}>
            <p>
              {t('Open a terminal and enter the following command.', '터미널을 열고 아래 명령어를 입력합니다.')}
            </p>
            <CopyBlock code={`/bin/bash -c "$(curl -fsSL https://aimi.cokac.com/install.sh)"`} label="Terminal">
{`/bin/bash -c "$(curl -fsSL https://aimi.cokac.com/install.sh)"`}
            </CopyBlock>
          </SectionCard>

          {/* Step 2: Telegram */}
          <SectionCard icon={Bot} title={t('Create Telegram Bot', '텔레그램 봇 만들기')} step={2}>
            <p>
              {t(
                <>Go to <a href="https://t.me/botfather" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">@BotFather</a> and press <strong className="text-white">START BOT</strong> to begin.</>,
                <><a href="https://t.me/botfather" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">@BotFather</a>에서 <strong className="text-white">START BOT</strong>을 눌러 대화를 시작합니다.</>
              )}
            </p>

            <InlineStep n={1}>
              {t(
                <>Type <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/newbot</code>.</>,
                <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/newbot</code>을 입력합니다.</>
              )}
            </InlineStep>
            <InlineStep n={2}>
              {t(
                <>Set the bot's <strong className="text-white">name</strong> and <strong className="text-white">username</strong>.<br /><span className="text-zinc-500">e.g. name: 'mybot', username: 'my_cokac_bot'</span></>,
                <>Bot의 <strong className="text-white">name</strong>과 <strong className="text-white">username</strong>을 정합니다.<br /><span className="text-zinc-500">예: name은 '코깎봇', username은 'cokac_bot'</span></>
              )}
            </InlineStep>
            <InlineStep n={3}>
              {t('A token will be issued in the following format:', '토큰이 발급됩니다. 아래와 같은 형식입니다:')}
            </InlineStep>

            <div className="my-2 px-4 py-3 bg-bg-card border border-zinc-800 rounded-lg">
              <code className="font-mono text-sm text-yellow-400">123456789:ABCdefGHIjklMNOpqrsTUVwxyz</code>
            </div>
            <p className="text-zinc-500 text-sm">{t('Copy this token.', '이 토큰을 복사해 둡니다.')}</p>
          </SectionCard>

          {/* Step 3: Register background service */}
          <SectionCard icon={Apple} title={t('Register Background Service', '백그라운드 서비스 등록')} step={3}>
            <p>
              {t(
                <>Register a macOS system service with the Telegram bot token you received. It will <strong className="text-white">start automatically</strong> even after rebooting your computer.</>,
                <>발급받은 텔레그램 봇 토큰으로 macOS 시스템 서비스를 등록합니다. 컴퓨터를 껐다 켜도 <strong className="text-white">자동으로 구동</strong>됩니다.</>
              )}
            </p>

            <div className="my-4 p-4 rounded-lg border border-yellow-500/20 bg-yellow-500/5">
              <p className="text-sm text-zinc-300">
                {t(
                  <>The command below requires <strong className="text-white">Node.js</strong>. If not installed, download and install it from <a href="https://nodejs.org/en/download" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">nodejs.org/en/download</a> first.</>,
                  <>아래 명령어는 <strong className="text-white">Node.js</strong>가 필요합니다. 설치되어 있지 않다면 <a href="https://nodejs.org/ko/download" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">nodejs.org/ko/download</a>에서 먼저 다운로드하여 설치하세요.</>
                )}
              </p>
            </div>

            <CopyBlock code={`npx -y service-setup-aimi <텔레그램봇토큰>`} label="Terminal">
{`npx -y service-setup-aimi `}<Hl>{'<텔레그램봇토큰>'}</Hl>
            </CopyBlock>

            <div className="mt-4 p-4 rounded-lg border border-accent-cyan/20 bg-accent-cyan/5">
              <p className="text-sm text-zinc-300">
                {t(
                  <>Replace <Hl>{'<텔레그램봇토큰>'}</Hl> with the actual token issued by BotFather and run the command.</>,
                  <><Hl>{'<텔레그램봇토큰>'}</Hl> 부분을 BotFather에서 발급받은 실제 토큰으로 바꿔 넣고 실행하세요.</>
                )}
              </p>
            </div>
          </SectionCard>

          {/* Divider */}
          <div className="flex items-center gap-4 my-12">
            <div className="flex-1 h-px bg-zinc-800" />
            <span className="text-zinc-500 text-sm font-medium">{t('All Done', '모든 설정 끝')}</span>
            <div className="flex-1 h-px bg-zinc-800" />
          </div>

          {/* Usage */}
          <motion.section
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="mb-10"
          >
            <div className="flex items-center gap-3 mb-5">
              <div className="flex-shrink-0 w-10 h-10 rounded-full bg-accent-green/20 border border-accent-green/50 flex items-center justify-center">
                <MessageCircle className="w-5 h-5 text-accent-green" />
              </div>
              <h2 className="text-xl sm:text-2xl font-bold text-white">{t('Usage', '사용하기')}</h2>
            </div>
            <div className="ml-0 sm:ml-[52px] text-zinc-300 text-sm sm:text-base leading-relaxed space-y-4">
              <InlineStep n={1}>
                {t(
                  <>Go to <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">https://t.me/[your bot username]</code> and press <strong className="text-white">START BOT</strong> to begin chatting with the bot.</>,
                  <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">https://t.me/[앞서 정한 username]</code> 으로 접속해서 <strong className="text-white">START BOT</strong>을 눌러 봇과의 대화를 시작합니다.</>
                )}
              </InlineStep>
              <InlineStep n={2}>
                {t(
                  <>Type <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/start</code> to launch Claude.</>,
                  <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/start</code> 를 입력하면 Claude를 실행한 것과 같은 상태가 됩니다.</>
                )}
              </InlineStep>
              <InlineStep n={3}>
                {t(
                  <>From now on, you can make requests just like using Claude Code, such as <strong className="text-white">"Build me a website"</strong>.</>,
                  <>이제부터 <strong className="text-white">"웹사이트 만들어줘"</strong> 와 같이 Claude Code를 사용할 때와 같은 방식으로 요청할 수 있습니다.</>
                )}
              </InlineStep>

              <div className="mt-2 flex flex-col sm:flex-row gap-3">
                <button
                  onClick={() => {
                    navigate('/tutorial', { state: { scrollTo: 'telegram-workflow' } })
                  }}
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-accent-cyan/30 bg-accent-cyan/5 text-accent-cyan text-sm font-medium hover:bg-accent-cyan/10 transition-colors cursor-pointer"
                >
                  <MessageCircle className="w-4 h-4" />
                  {t('Workflow in Practice — Learn More →', '실전 사용 워크플로우 — 자세히 보기 →')}
                </button>
                <button
                  onClick={() => {
                    navigate('/tutorial', { state: { scrollTo: 'telegram-commands' } })
                  }}
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-accent-cyan/30 bg-accent-cyan/5 text-accent-cyan text-sm font-medium hover:bg-accent-cyan/10 transition-colors cursor-pointer"
                >
                  <Terminal className="w-4 h-4" />
                  {t('Available Commands — Learn More →', '사용 가능한 명령어 — 자세히 보기 →')}
                </button>
              </div>
            </div>
          </motion.section>

          {/* Update Command */}
          <div className="flex items-center gap-4 my-12">
            <div className="flex-1 h-px bg-zinc-800" />
            <div className="flex items-center gap-2 text-zinc-500 text-sm font-medium">
              <RefreshCw className="w-4 h-4" />
              {t('Update', '업데이트')}
            </div>
            <div className="flex-1 h-px bg-zinc-800" />
          </div>

          <motion.section
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="mb-10"
          >
            <div className="flex items-center gap-3 mb-5">
              <div className="flex-shrink-0 w-10 h-10 rounded-full bg-accent-cyan/20 border border-accent-cyan/50 flex items-center justify-center">
                <RefreshCw className="w-5 h-5 text-accent-cyan" />
              </div>
              <h2 className="text-xl sm:text-2xl font-bold text-white">{t('Update Command', '업데이트 명령어')}</h2>
            </div>
            <div className="ml-0 sm:ml-[52px] text-zinc-300 text-sm sm:text-base leading-relaxed space-y-4">
              <p>
                {t(
                  <>When a new version is released, run the update command to upgrade to the latest version.</>,
                  <>새 버전이 출시되면 업데이트 명령어를 실행하여 최신 버전으로 업그레이드할 수 있습니다.</>
                )}
              </p>

              <CopyBlock code={`/bin/bash -c "$(curl -fsSL https://aimi.cokac.com/install.sh)" && npx -y service-setup-aimi <텔레그램봇토큰>`} label="Terminal">
{`/bin/bash -c "$(curl -fsSL https://aimi.cokac.com/install.sh)" && npx -y service-setup-aimi `}<Hl>{'<텔레그램봇토큰>'}</Hl>
              </CopyBlock>

              <div className="p-4 rounded-lg border border-accent-cyan/20 bg-accent-cyan/5">
                <p className="text-sm text-zinc-300">
                  {t(
                    <>Replace <Hl>{'<텔레그램봇토큰>'}</Hl> with the actual token issued by BotFather and run the command.</>,
                    <><Hl>{'<텔레그램봇토큰>'}</Hl> 부분을 BotFather에서 발급받은 실제 토큰으로 바꿔 넣고 실행하세요.</>
                  )}
                </p>
              </div>
            </div>
          </motion.section>

        </motion.div>

        {/* Bottom navigation */}
        <div className="mt-16 pt-8 border-t border-zinc-800 flex flex-col sm:flex-row items-center justify-between gap-4">
          <Link
            to="/"
            className="flex items-center gap-2 text-zinc-400 hover:text-accent-cyan transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            {t('Back to Home', '홈으로 돌아가기')}
          </Link>
          <Link
            to="/tutorial"
            className="flex items-center gap-2 text-zinc-400 hover:text-accent-cyan transition-colors"
          >
            Beginner Tutorial →
          </Link>
        </div>
      </main>
    </div>
  )
}

export default function MacOSPage() {
  return (
    <LanguageProvider>
      <MacOSPageInner />
    </LanguageProvider>
  )
}
