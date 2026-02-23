import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { motion } from 'framer-motion'
import { ArrowLeft, Github, Cloud, Server, Bot, Terminal, Copy, Check, Monitor, Apple, Rocket, MessageCircle, RefreshCw } from 'lucide-react'
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
  icon: typeof Server
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

function EC2PageInner() {
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
              <Cloud className="w-4 h-4 text-accent-cyan" />
              <span className="hidden sm:inline">EC2 Setup Guide</span>
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
              <Cloud className="w-4 h-4" />
              AWS EC2 Sandbox
            </div>
            <h1 className="text-3xl sm:text-4xl lg:text-5xl font-extrabold text-white mb-4">
              {t('Claude Code on EC2', 'EC2에서 Claude Code')}
              <br />
              <span className="gradient-text">{t('Sandbox Setup', '샌드박스 셋업')}</span>
            </h1>
            <p className="text-lg text-zinc-400 leading-relaxed max-w-2xl mx-auto">
              {t(
                'A guide to setting up a aimi & Claude Code environment on AWS EC2 and using it anywhere via Telegram bot.',
                'AWS EC2 위에 aimi & Claude Code 환경을 만들고, 텔레그램 봇으로 어디서나 사용하는 가이드입니다.'
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
              {t('3 Prerequisites', '준비물 3가지')}
            </h3>
            <div className="grid sm:grid-cols-3 gap-3">
              <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-card border border-zinc-800">
                <Server className="w-5 h-5 text-accent-cyan flex-shrink-0" />
                <div>
                  <p className="text-white font-medium text-sm">{t('EC2 IP Address', 'EC2 IP 주소')}</p>
                  <p className="text-zinc-500 text-xs">{t('Public IPv4', '퍼블릭 IPv4')}</p>
                </div>
              </div>
              <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-card border border-zinc-800">
                <Terminal className="w-5 h-5 text-accent-cyan flex-shrink-0" />
                <div>
                  <p className="text-white font-medium text-sm">{t('PEM Key File', 'PEM 키 파일')}</p>
                  <p className="text-zinc-500 text-xs">{t('For SSH access', 'SSH 접속용')}</p>
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

          {/* Step 1: EC2 */}
          <SectionCard icon={Server} title={t('Create EC2 Instance', 'EC2 인스턴스 만들기')} step={1}>
            <p>
              <a
                href="https://ap-northeast-2.console.aws.amazon.com/ec2/home#LaunchInstances:"
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent-cyan hover:underline font-medium"
              >
                {t('AWS EC2 Console → Launch Instances', 'AWS EC2 콘솔 → Launch Instances')}
              </a>
              {' '}{t('— create a new instance.', '에서 새 인스턴스를 만듭니다.')}
            </p>

            <InlineStep n={1}>
              {t(
                <>Set the instance <strong className="text-white">name</strong>.</>,
                <>인스턴스 <strong className="text-white">이름</strong>을 정합니다.</>
              )}
            </InlineStep>
            <InlineStep n={2}>
              {t(
                <>Select <strong className="text-white">Ubuntu</strong> as the OS.</>,
                <>OS는 <strong className="text-white">Ubuntu</strong>를 선택합니다.</>
              )}
            </InlineStep>
            <InlineStep n={3}>
              {t(
                <>Click <strong className="text-white">Create new key pair</strong> and download it as <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code>.</>,
                <>키페어에서 <strong className="text-white">새 키 페어 생성</strong>을 눌러 <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code> 이름으로 다운로드합니다.</>
              )}
            </InlineStep>
            <InlineStep n={4}>
              {t(
                <>Set the storage to <strong className="text-white">32 GB</strong>, then click <strong className="text-white">Launch instance</strong>.</>,
                <>스토리지 구성을 <strong className="text-white">32 GB</strong>로 설정한 뒤, <strong className="text-white">인스턴스 시작</strong> 버튼을 누릅니다.</>
              )}
            </InlineStep>

            <div className="my-4 p-4 rounded-lg border border-accent-cyan/20 bg-accent-cyan/5">
              <p className="text-sm text-zinc-300">
                {t(
                  <>Create a <strong className="text-white">credential</strong> folder on your computer and place the <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code> file inside.</>,
                  <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code> 파일은 컴퓨터에 <strong className="text-white">credential</strong> 폴더를 만들어 그 안에 넣어 둡니다.</>
                )}
              </p>
            </div>

            <p>
              {t(
                <>Go to the <a href="https://ap-northeast-2.console.aws.amazon.com/ec2/home#Instances:" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">EC2 instance list</a>, click the instance you just created, and copy the <strong className="text-white">Public IPv4 address</strong> from the details.</>,
                <><a href="https://ap-northeast-2.console.aws.amazon.com/ec2/home#Instances:" target="_blank" rel="noopener noreferrer" className="text-accent-cyan hover:underline font-medium">EC2 인스턴스 목록</a>에서 방금 만든 인스턴스를 클릭하고, 세부정보의 <strong className="text-white">퍼블릭 IPv4 주소</strong>를 복사해 둡니다.</>
              )}
            </p>
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

          {/* Step 3: Setup command */}
          <SectionCard icon={Terminal} title={t('Run EC2 Setup Command', 'EC2 셋팅 명령어 실행')} step={3}>
            <p>
              {t(
                <>Open a terminal in the <strong className="text-white">credential</strong> folder where <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code> is located.</>,
                <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">secret.pem</code> 파일이 들어 있는 <strong className="text-white">credential</strong> 폴더에서 터미널을 엽니다.</>
              )}
            </p>

            {/* macOS */}
            <div className="mt-6 p-4 rounded-xl border border-zinc-800 bg-bg-card">
              <div className="flex items-center gap-2 mb-3">
                <Apple className="w-5 h-5 text-zinc-400" />
                <h4 className="text-white font-semibold">macOS</h4>
              </div>
              <p className="text-zinc-400 text-sm mb-3">
                {t(
                  <>Right-click the credential folder → <strong className="text-zinc-200">Services</strong> → <strong className="text-zinc-200">New Terminal at Folder</strong></>,
                  <>credential 폴더를 우클릭 → <strong className="text-zinc-200">Services</strong> → <strong className="text-zinc-200">New Terminal at Folder</strong></>
                )}
              </p>
              <CopyBlock code={`export PEM=secret.pem\nexport IP=0.0.0.0\nexport TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz\nexport URL=https://raw.githubusercontent.com/kstost/service-setup-aimi/refs/heads/main/basic_setup_ec2.sh\nssh -t -i "$PEM" ubuntu@$IP "bash -ic \\"source <(curl -sL $URL) > /dev/null 2>&1 && npx -y service-setup-aimi $TOKEN && claude\\""`} label="macOS Terminal">
{`export PEM=`}<Hl>secret.pem</Hl>{`\nexport IP=`}<Hl>0.0.0.0</Hl>{`\nexport TOKEN=`}<Hl>123456789:ABCdefGHIjklMNOpqrsTUVwxyz</Hl>{`\nexport URL=https://raw.githubusercontent.com/kstost/service-setup-aimi/refs/heads/main/basic_setup_ec2.sh\nssh -t -i "$PEM" ubuntu@$IP "bash -ic \\"source <(curl -sL $URL) > /dev/null 2>&1 && npx -y service-setup-aimi $TOKEN && claude\\""`}
              </CopyBlock>
            </div>

            {/* Windows */}
            <div className="mt-4 p-4 rounded-xl border border-zinc-800 bg-bg-card">
              <div className="flex items-center gap-2 mb-3">
                <Monitor className="w-5 h-5 text-zinc-400" />
                <h4 className="text-white font-semibold">Windows</h4>
              </div>
              <p className="text-zinc-400 text-sm mb-3">
                {t(
                  <>Right-click the credential folder → <strong className="text-zinc-200">Open in Terminal</strong></>,
                  <>credential 폴더를 우클릭 → <strong className="text-zinc-200">터미널에서 열기</strong></>
                )}
              </p>
              <CopyBlock code={`$PEM = "secret.pem"; \`\n$IP = "0.0.0.0"; \`\n$TOKEN = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"; \`\n$URL = "https://raw.githubusercontent.com/kstost/service-setup-aimi/refs/heads/main/basic_setup_ec2.sh"; \`\nssh -t -i $PEM ubuntu@$IP "bash -ic 'source <(curl -sL $URL) > /dev/null 2>&1 && npx -y service-setup-aimi $TOKEN && claude'"`} label="PowerShell">
{`$PEM = "`}<Hl>secret.pem</Hl>{`"; \`\n$IP = "`}<Hl>0.0.0.0</Hl>{`"; \`\n$TOKEN = "`}<Hl>123456789:ABCdefGHIjklMNOpqrsTUVwxyz</Hl>{`"; \`\n$URL = "https://raw.githubusercontent.com/kstost/service-setup-aimi/refs/heads/main/basic_setup_ec2.sh"; \`\nssh -t -i $PEM ubuntu@$IP "bash -ic 'source <(curl -sL $URL) > /dev/null 2>&1 && npx -y service-setup-aimi $TOKEN && claude'"`}
              </CopyBlock>
            </div>

            <div className="mt-4 p-4 rounded-lg border border-accent-cyan/20 bg-accent-cyan/5">
              <p className="text-sm text-zinc-300">
                {t(
                  'Replace the PEM file name, EC2 IP address, and Telegram bot token with your own and run the command. After a moment, the Claude Code setup process will appear — complete the authentication and you\'re ready.',
                  '명령어 안의 PEM 파일 이름, EC2 IP 주소, 텔레그램 봇 토큰을 본인 것으로 바꿔 넣고 실행하세요. 잠시 기다리면 Claude Code 설정 과정이 나오고, 인증을 마치면 준비 완료입니다.'
                )}
              </p>
            </div>
          </SectionCard>

          {/* Divider */}
          <div className="flex items-center gap-4 my-12">
            <div className="flex-1 h-px bg-zinc-800" />
            <span className="text-zinc-500 text-sm font-medium">{t('Setup Complete', '설치 완료')}</span>
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
                  <>Type <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/start /home/ubuntu</code> to launch Claude as if you're in the <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-zinc-300 text-sm">/home/ubuntu</code> directory on EC2.</>,
                  <><code className="px-1.5 py-0.5 bg-bg-elevated rounded text-accent-cyan text-sm">/start /home/ubuntu</code> 를 입력하면 EC2 컴퓨터 상 <code className="px-1.5 py-0.5 bg-bg-elevated rounded text-zinc-300 text-sm">/home/ubuntu</code> 폴더에서 Claude를 실행한 것과 같은 상태가 됩니다.</>
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
              <h2 className="text-xl sm:text-2xl font-bold text-white">{t('EC2 Update Command', 'EC2 업데이트 명령어')}</h2>
            </div>
            <div className="ml-0 sm:ml-[52px] text-zinc-300 text-sm sm:text-base leading-relaxed space-y-4">
              <p>
                {t(
                  <>When a new version is released, run the update command to upgrade to the latest version.</>,
                  <>새 버전이 출시되면 업데이트 명령어를 실행하여 최신 버전으로 업그레이드할 수 있습니다.</>
                )}
              </p>

              {/* macOS */}
              <div className="p-4 rounded-xl border border-zinc-800 bg-bg-card">
                <div className="flex items-center gap-2 mb-3">
                  <Apple className="w-5 h-5 text-zinc-400" />
                  <h4 className="text-white font-semibold">macOS</h4>
                </div>
                <CopyBlock code={`export PEM=secret.pem\nexport IP=0.0.0.0\nexport TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz\nssh -t -i "$PEM" ubuntu@$IP "bash -ic \\"curl -fsSL https://aimi.cokac.com/install.sh | /bin/bash && npx -y service-setup-aimi $TOKEN\\""`} label="macOS Terminal">
{`export PEM=`}<Hl>secret.pem</Hl>{`\nexport IP=`}<Hl>0.0.0.0</Hl>{`\nexport TOKEN=`}<Hl>123456789:ABCdefGHIjklMNOpqrsTUVwxyz</Hl>{`\nssh -t -i "$PEM" ubuntu@$IP "bash -ic \\"curl -fsSL https://aimi.cokac.com/install.sh | /bin/bash && npx -y service-setup-aimi $TOKEN\\""`}
                </CopyBlock>
              </div>

              {/* Windows */}
              <div className="p-4 rounded-xl border border-zinc-800 bg-bg-card">
                <div className="flex items-center gap-2 mb-3">
                  <Monitor className="w-5 h-5 text-zinc-400" />
                  <h4 className="text-white font-semibold">Windows</h4>
                </div>
                <CopyBlock code={`$PEM = "secret.pem"; \`\n$IP = "0.0.0.0"; \`\n$TOKEN = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"; \`\nssh -t -i $PEM ubuntu@$IP "bash -ic 'curl -fsSL https://aimi.cokac.com/install.sh | /bin/bash && npx -y service-setup-aimi $TOKEN'"`} label="PowerShell">
{`$PEM = "`}<Hl>secret.pem</Hl>{`"; \`\n$IP = "`}<Hl>0.0.0.0</Hl>{`"; \`\n$TOKEN = "`}<Hl>123456789:ABCdefGHIjklMNOpqrsTUVwxyz</Hl>{`"; \`\nssh -t -i $PEM ubuntu@$IP "bash -ic 'curl -fsSL https://aimi.cokac.com/install.sh | /bin/bash && npx -y service-setup-aimi $TOKEN'"`}
                </CopyBlock>
              </div>

              <div className="p-4 rounded-lg border border-accent-cyan/20 bg-accent-cyan/5">
                <p className="text-sm text-zinc-300">
                  {t(
                    'Replace the PEM file name, EC2 IP address, and Telegram bot token with your own and run the command.',
                    '명령어 안의 PEM 파일 이름, EC2 IP 주소, 텔레그램 봇 토큰을 본인 것으로 바꿔 넣고 실행하세요.'
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

export default function EC2Page() {
  return (
    <LanguageProvider>
      <EC2PageInner />
    </LanguageProvider>
  )
}
