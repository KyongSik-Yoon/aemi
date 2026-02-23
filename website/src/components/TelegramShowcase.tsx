import { motion } from 'framer-motion'
import { Send, Terminal, FileUp, Bot, Wrench, Smartphone, Cloud, Apple, Activity, Hand, Layers, ShieldCheck, Zap } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useLanguage } from './tutorial/LanguageContext'

function ChatBubble({ from, children, delay }: { from: 'user' | 'bot'; children: React.ReactNode; delay: number }) {
  const isUser = from === 'user'
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
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

function ChatDemo() {
  const { t } = useLanguage()
  return (
    <div className="relative">
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
        <div className="p-4 space-y-3 min-h-[280px]">
          <ChatBubble from="user" delay={0.1}>
            <code className="text-accent-cyan font-mono text-xs">/start ~/my-project</code>
          </ChatBubble>
          <ChatBubble from="bot" delay={0.3}>
            {t(
              <>Session started at <code className="text-accent-cyan font-mono text-xs">/home/user/my-project</code></>,
              <><code className="text-accent-cyan font-mono text-xs">/home/user/my-project</code>에서 세션이 시작되었습니다</>
            )}
          </ChatBubble>
          <ChatBubble from="user" delay={0.5}>
            {t(
              'Find all TODO comments and create a summary',
              'TODO 주석을 모두 찾아서 요약해줘'
            )}
          </ChatBubble>
          <ChatBubble from="bot" delay={0.7}>
            <span className="text-zinc-400">{t('Searching files...', '파일 검색 중...')}</span>
            <div className="mt-2 text-zinc-300">
              {t(
                <>Found <strong className="text-white">12 TODOs</strong> across 5 files. Here's the summary:</>,
                <>5개 파일에서 <strong className="text-white">12개의 TODO</strong>를 찾았습니다. 요약입니다:</>
              )}
            </div>
            <div className="mt-1 font-mono text-xs text-accent-cyan">
              src/main.rs:42 — TODO: add error handling<br />
              src/api.rs:18 — TODO: implement caching
            </div>
          </ChatBubble>
          <ChatBubble from="user" delay={0.9}>
            <code className="text-accent-cyan font-mono text-xs">!git status</code>
          </ChatBubble>
          <ChatBubble from="bot" delay={1.1}>
            <span className="font-mono text-xs text-zinc-400">On branch main<br />2 files changed, 48 insertions(+)</span>
          </ChatBubble>
        </div>
      </div>
    </div>
  )
}

export default function TelegramShowcase() {
  const { t } = useLanguage()

  const features = [
    {
      icon: Bot,
      title: t('Claude AI in Your Pocket', '주머니 속 Claude AI'),
      desc: t(
        'Chat with Claude AI through Telegram. Ask questions, generate code, analyze files — all from your phone.',
        '텔레그램으로 Claude AI와 대화하세요. 질문, 코드 생성, 파일 분석 — 모두 폰에서.'
      ),
    },
    {
      icon: Terminal,
      title: t('Remote Shell Access', '원격 쉘 접근'),
      desc: t(
        'Execute shell commands on your server with ! prefix. Check logs, manage processes, run scripts.',
        '! 접두사로 서버에서 쉘 명령을 실행하세요. 로그 확인, 프로세스 관리, 스크립트 실행.'
      ),
    },
    {
      icon: FileUp,
      title: t('File Transfer', '파일 전송'),
      desc: t(
        'Download server files or upload from your phone. AI can also send generated files directly to you.',
        '서버 파일을 다운로드하거나 폰에서 업로드하세요. AI가 생성한 파일도 바로 전송됩니다.'
      ),
    },
    {
      icon: Wrench,
      title: t('Dynamic Tool Control', '동적 도구 제어'),
      desc: t(
        'Add or remove AI tools on the fly. Fine-tune what Claude can do for security or workflow needs.',
        'AI 도구를 즉석에서 추가/제거하세요. 보안이나 워크플로우에 맞게 Claude 권한을 세밀하게 조정.'
      ),
    },
    {
      icon: Activity,
      title: t('Real-time Progress', '실시간 진행 상황'),
      desc: t(
        'Watch your tasks progress in real time. See exactly what Claude is doing as it works through your request.',
        '작업 진행 과정을 실시간으로 확인하세요. Claude가 요청을 처리하는 모습을 그대로 볼 수 있습니다.'
      ),
    },
    {
      icon: Hand,
      title: t('Interrupt Anytime', '언제든 중단'),
      desc: t(
        'Stop any running task instantly. You stay in full control — cancel, redirect, or restart at any moment.',
        '실행 중인 작업을 즉시 중단할 수 있습니다. 취소, 방향 전환, 재시작 — 항상 당신이 통제합니다.'
      ),
    },
    {
      icon: Layers,
      title: t('Multiple Sessions', '다중 세션'),
      desc: t(
        'Spin up multiple work sessions with dead-simple commands. Run parallel tasks across different projects.',
        '간단한 명령으로 여러 작업 세션을 띄우세요. 다른 프로젝트에서 병렬 작업을 실행할 수 있습니다.'
      ),
    },
    {
      icon: ShieldCheck,
      title: t('No Open Ports', '오픈 포트 불필요'),
      desc: t(
        'Zero exposed ports on your server. All communication goes through Telegram\'s API — secure by design.',
        '서버에 노출되는 포트가 없습니다. 모든 통신은 텔레그램 API를 통해 이루어져 설계부터 안전합니다.'
      ),
    },
  ]

  return (
    <section className="py-12 sm:py-24 px-4 relative overflow-hidden">
      {/* Background */}
      <div className="absolute inset-0 bg-gradient-to-b from-accent-cyan/5 via-accent-cyan/8 to-accent-cyan/5 pointer-events-none" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[300px] h-[300px] sm:w-[600px] sm:h-[600px] bg-accent-cyan/8 rounded-full blur-3xl pointer-events-none" />

      <div className="relative max-w-6xl mx-auto">
        {/* Header */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
          className="text-center mb-8 sm:mb-16"
        >
          <div className="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-accent-cyan/10 border border-accent-cyan/20 text-sm text-accent-cyan mb-6">
            <Smartphone className="w-4 h-4" />
            {t('Remote Control via Telegram', '텔레그램으로 원격 제어')}
          </div>
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            {t(
              <>Control <span className="text-accent-cyan">Claude Code</span> from Your Phone</>,
              <>폰에서 <span className="text-accent-cyan">Claude Code</span>를 제어하세요</>
            )}
          </h2>
          <p className="text-zinc-400 text-sm sm:text-lg max-w-2xl mx-auto">
            {t(
              'Run a Telegram bot on your server and get full Claude AI access anywhere. Execute commands, transfer files, and manage projects — all from a chat.',
              '서버에서 텔레그램 봇을 실행하고 어디서든 Claude AI에 접근하세요. 명령 실행, 파일 전송, 프로젝트 관리 — 모두 채팅으로.'
            )}
          </p>
        </motion.div>

        {/* 2-column layout: Chat demo + Feature grid */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8 sm:gap-12 items-start mb-10 sm:mb-16">
          {/* Left: Chat demo */}
          <motion.div
            initial={{ opacity: 0, x: -30 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.7 }}
          >
            <ChatDemo />
          </motion.div>

          {/* Right: Feature grid 2-col 4-row */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            {features.map((f, i) => (
              <motion.div
                key={i}
                initial={{ opacity: 0, y: 20 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ duration: 0.4, delay: 0.1 + i * 0.06 }}
                className="p-4 rounded-xl bg-bg-card/50 border border-zinc-800 hover:border-accent-cyan/30 transition-colors"
              >
                <div className="w-9 h-9 rounded-lg bg-accent-cyan/10 border border-accent-cyan/20 flex items-center justify-center mb-3">
                  <f.icon className="w-4.5 h-4.5 text-accent-cyan" />
                </div>
                <h3 className="text-white font-semibold text-sm mb-1">{f.title}</h3>
                <p className="text-zinc-500 text-xs leading-relaxed">{f.desc}</p>
              </motion.div>
            ))}
          </div>
        </div>

        {/* Quick start */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
          className="bg-bg-card border border-zinc-800 rounded-xl p-5 sm:p-8 text-center"
        >
          <h3 className="text-xl font-bold mb-3">{t('Get Started in 2 Steps', '2단계로 시작하기')}</h3>
          <div className="flex flex-col sm:flex-row gap-4 justify-center items-stretch max-w-2xl mx-auto mb-6">
            <div className="flex-1 bg-bg-elevated border border-zinc-700 rounded-lg p-4 text-left">
              <div className="text-accent-cyan font-mono text-sm mb-2">{t('Step 1', '1단계')}</div>
              <p className="text-zinc-400 text-sm">
                {t(
                  <>Create a bot via <strong className="text-white">@BotFather</strong> on Telegram and copy the API token</>,
                  <>텔레그램에서 <strong className="text-white">@BotFather</strong>로 봇을 만들고 API 토큰을 복사하세요</>
                )}
              </p>
            </div>
            <div className="flex-1 bg-bg-elevated border border-zinc-700 rounded-lg p-4 text-left">
              <div className="text-accent-cyan font-mono text-sm mb-2">{t('Step 2', '2단계')}</div>
              <code className="text-accent-cyan font-mono text-sm">aimi --ccserver YOUR_TOKEN</code>
            </div>
          </div>
          <div className="flex flex-wrap justify-center gap-3">
            <Link
              to="/workflows"
              className="inline-flex items-center gap-2 px-6 py-3 rounded-lg bg-accent-cyan text-bg-dark font-semibold hover:bg-accent-cyan/90 shadow-lg shadow-accent-cyan/25 transition-colors"
            >
              <Zap className="w-4 h-4" />
              {t('See Workflows', '워크플로우 보기')}
            </Link>
            <Link
              to="/tutorial"
              onClick={() => setTimeout(() => {
                const el = document.getElementById('telegram-bot')
                if (el) el.scrollIntoView({ behavior: 'smooth' })
              }, 100)}
              className="inline-flex items-center gap-2 px-6 py-3 rounded-lg bg-accent-cyan/10 border border-accent-cyan/30 text-accent-cyan font-semibold hover:bg-accent-cyan/20 transition-colors"
            >
              <Send className="w-4 h-4" />
              {t('Full Setup Guide', '전체 설정 가이드')}
            </Link>
            <Link
              to="/ec2"
              className="inline-flex items-center gap-2 px-6 py-3 rounded-lg bg-accent-purple/10 border border-accent-purple/30 text-accent-purple font-semibold hover:bg-accent-purple/20 transition-colors"
            >
              <Cloud className="w-4 h-4" />
              {t('EC2 Setup Guide', 'EC2 설정 가이드')}
            </Link>
            <Link
              to="/macos"
              className="inline-flex items-center gap-2 px-6 py-3 rounded-lg bg-zinc-800 border border-zinc-700 text-zinc-300 font-semibold hover:bg-zinc-700 transition-colors"
            >
              <Apple className="w-4 h-4" />
              {t('macOS Setup Guide', 'macOS 설정 가이드')}
            </Link>
          </div>
        </motion.div>
      </div>
    </section>
  )
}
