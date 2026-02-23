import SectionHeading from '../ui/SectionHeading'
import KeyBadge from '../ui/KeyBadge'
import TipBox from '../ui/TipBox'
import StepByStep from '../ui/StepByStep'
import { useLanguage } from '../LanguageContext'

const codeStyle = "text-accent-cyan font-mono bg-bg-elevated px-1 py-0.5 rounded"
const jsonBlockStyle = "bg-bg-elevated border border-zinc-800 rounded-lg p-4 font-mono text-sm leading-relaxed overflow-x-auto mb-4"

export default function SettingsConfig() {
  const { lang, t } = useLanguage()

  return (
    <section className="mb-16">
      <SectionHeading id="settings-config">{t('Settings', '설정 바꾸기')}</SectionHeading>

      {lang === 'ko' ? (
        <>
          <p className="text-zinc-400 mb-6 leading-relaxed">
            aimi의 외관이나 동작 방식을 취향에 맞게 바꿀 수 있습니다.
            테마(색상), 비교 화면 스타일, 단축키 등을 설정 파일에서 변경할 수 있습니다.
          </p>

          <SectionHeading id="settings-open" level={3}>설정 화면 열기</SectionHeading>
          <StepByStep steps={[
            {
              title: '백틱(`) 키를 누릅니다',
              description: (
                <span>
                  <KeyBadge>`</KeyBadge> (백틱 — 키보드 숫자 1 왼쪽, 물결표 ~ 와 같은 키)를 누르면 설정 화면이 열립니다.
                </span>
              ),
            },
            {
              title: '설정 항목을 선택합니다',
              description: (
                <span>
                  <KeyBadge>{'\u2191'}</KeyBadge><KeyBadge>{'\u2193'}</KeyBadge>로 변경하고 싶은 설정 항목을 선택합니다.
                </span>
              ),
            },
            {
              title: '값을 변경합니다',
              description: (
                <span>
                  <KeyBadge>{'\u2190'}</KeyBadge><KeyBadge>{'\u2192'}</KeyBadge> 좌우 화살표로 값을 바꿉니다.
                  예를 들어 테마를 고를 때 좌우 화살표로 다른 테마를 미리보기할 수 있습니다.
                </span>
              ),
            },
            {
              title: 'Enter로 저장하거나 Esc로 취소합니다',
              description: (
                <span>
                  <KeyBadge>Enter</KeyBadge>를 누르면 변경사항이 저장됩니다.
                  마음에 안 들면 <KeyBadge>Esc</KeyBadge>를 눌러 변경 없이 나갈 수 있습니다.
                </span>
              ),
            },
          ]} />

          <SectionHeading id="theme-change" level={3}>테마 (색상) 변경</SectionHeading>
          <p className="text-zinc-400 mb-4">
            aimi의 색상 조합을 바꿀 수 있습니다. 기본 테마가 마음에 안 들면 다른 테마로 바꿔보세요.
          </p>
          <p className="text-zinc-400 mb-4">
            테마 파일은 aimi를 처음 실행할 때 자동으로 생성됩니다.
            위치는 <code className={codeStyle}>~/.aimi/themes/</code> 폴더입니다.
          </p>

          <TipBox>
            JSON 파일을 직접 편집하면 더 세밀한 색상 커스터마이징이 가능합니다.
            하지만 처음에는 설정 화면에서 제공하는 테마 중 골라 쓰는 것으로 충분합니다.
          </TipBox>

          {/* ── 키바인딩 커스터마이징 ── */}
          <SectionHeading id="keybinding-custom" level={3}>단축키 커스터마이징</SectionHeading>
          <p className="text-zinc-400 mb-4">
            aimi의 모든 단축키를 원하는 키로 바꿀 수 있습니다.
            설정 파일(<code className={codeStyle}>~/.aimi/settings.json</code>)의
            {' '}<code className={codeStyle}>keybindings</code> 섹션을 편집하면 됩니다.
          </p>

          <SectionHeading id="keybinding-howto" level={3}>단축키 변경 방법</SectionHeading>
          <StepByStep steps={[
            {
              title: 'settings.json 파일을 엽니다',
              description: (
                <span>
                  aimi에서 <code className={codeStyle}>~/.aimi/</code> 폴더로 이동한 뒤
                  {' '}<code className={codeStyle}>settings.json</code> 파일을 <KeyBadge>E</KeyBadge>로 열어 편집합니다.
                </span>
              ),
            },
            {
              title: 'keybindings 섹션을 찾거나 추가합니다',
              description: (
                <span>
                  파일 안에 <code className={codeStyle}>"keybindings"</code> 항목이 있습니다.
                  변경하고 싶은 컨텍스트와 액션만 추가하면 됩니다.
                  나머지는 기본값이 그대로 유지됩니다.
                </span>
              ),
            },
            {
              title: '저장하면 즉시 반영됩니다',
              description: (
                <span>
                  <KeyBadge>Ctrl+S</KeyBadge>로 저장하면 변경된 단축키가 즉시 적용됩니다.
                  에디터를 닫을 필요 없이 저장만 하면 화면 표시까지 바로 업데이트됩니다.
                </span>
              ),
            },
          ]} />

          <SectionHeading id="keybinding-format" level={3}>키 표기법</SectionHeading>
          <p className="text-zinc-400 mb-4">
            키 조합은 <code className={codeStyle}>"수식자+키"</code> 형식으로 작성합니다.
            대소문자를 구분하지 않으며, 여러 수식자를 조합할 수 있습니다.
          </p>
          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">조합 형식</p>
          <p className="text-zinc-400 mb-3 text-sm">
            수식자와 키를 <code className={codeStyle}>+</code>로 연결합니다. 수식자 없이 단일 키만 쓸 수도 있습니다.
          </p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">표기</th>
                  <th className="pb-2 text-zinc-300 font-semibold">실제 키 입력</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"q"</td><td className="py-1.5">Q 키 단독</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"esc"</td><td className="py-1.5">Escape 키 단독</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"f5"</td><td className="py-1.5">F5 키 단독</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+s"</td><td className="py-1.5">Ctrl 누른 채로 S</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"shift+up"</td><td className="py-1.5">Shift 누른 채로 위쪽 화살표</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"alt+enter"</td><td className="py-1.5">Alt 누른 채로 Enter</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+shift+a"</td><td className="py-1.5">Ctrl + Shift 누른 채로 A</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+alt+delete"</td><td className="py-1.5">Ctrl + Alt 누른 채로 Delete</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">"ctrl+shift+f12"</td><td className="py-1.5">Ctrl + Shift 누른 채로 F12</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">수식자 키 (Modifier)</p>
          <p className="text-zinc-400 mb-2 text-sm">+ 앞에 붙여서 키 조합을 만듭니다. 여러 개를 함께 쓸 수 있습니다.</p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">표기</th>
                  <th className="pb-2 text-zinc-300 font-semibold">키</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">ctrl 또는 control</td><td className="py-1.5">Control (Ctrl) 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">shift</td><td className="py-1.5">Shift 키</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">alt</td><td className="py-1.5">Alt 키 (macOS에서는 Option)</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">특수 키 (Special Keys)</p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">표기</th>
                  <th className="pb-2 text-zinc-300 font-semibold">키</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">up, down, left, right</td><td className="py-1.5">방향키 (화살표)</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">enter 또는 return</td><td className="py-1.5">Enter 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">esc 또는 escape</td><td className="py-1.5">Escape 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">tab</td><td className="py-1.5">Tab 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">space</td><td className="py-1.5">스페이스바</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">backspace</td><td className="py-1.5">Backspace 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">delete 또는 del</td><td className="py-1.5">Delete 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">home</td><td className="py-1.5">Home 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">end</td><td className="py-1.5">End 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">pageup</td><td className="py-1.5">Page Up 키</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">pagedown</td><td className="py-1.5">Page Down 키</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">f1, f2, ... f12</td><td className="py-1.5">기능키 (Function Keys)</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">문자 및 기호 키</p>
          <div className={jsonBlockStyle}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">표기</th>
                  <th className="pb-2 text-zinc-300 font-semibold">키</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">a ~ z</td><td className="py-1.5">알파벳 (소문자로 적으면 대소문자 모두 인식)</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">0 ~ 9</td><td className="py-1.5">숫자 키</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">* ; ' ` . / - = [ ] 등</td><td className="py-1.5">기호 키 (한 글자 그대로 적음)</td></tr>
              </tbody>
            </table>
          </div>

          <TipBox>
            알파벳 키는 소문자로만 적으면 됩니다. 예를 들어 <code className={codeStyle}>"n"</code>으로 지정하면
            {' '}<KeyBadge>N</KeyBadge>과 <KeyBadge>Shift+N</KeyBadge> 모두 동작합니다.
          </TipBox>

          <SectionHeading id="keybinding-contexts" level={3}>컨텍스트 목록</SectionHeading>
          <p className="text-zinc-400 mb-4">
            각 화면(컨텍스트)마다 별도의 단축키 그룹이 있습니다.
            변경하고 싶은 화면의 컨텍스트 이름을 사용하세요:
          </p>
          <div className={jsonBlockStyle}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold">컨텍스트</th>
                  <th className="pb-2 text-zinc-300 font-semibold">화면</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_panel</td><td className="py-1.5">메인 파일 목록</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_editor</td><td className="py-1.5">파일 편집기</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_info</td><td className="py-1.5">파일 정보</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">system_info</td><td className="py-1.5">시스템 정보</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">search_result</td><td className="py-1.5">검색 결과</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">advanced_search</td><td className="py-1.5">고급 검색 다이얼로그</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">diff_screen</td><td className="py-1.5">폴더 비교 목록</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">diff_file_view</td><td className="py-1.5">파일 내용 비교</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">image_viewer</td><td className="py-1.5">이미지 뷰어</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">process_manager</td><td className="py-1.5">프로세스 관리자</td></tr>
              </tbody>
            </table>
          </div>

          <SectionHeading id="keybinding-examples" level={3}>설정 예시</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Quit 키를 <KeyBadge>Q</KeyBadge> 대신 <KeyBadge>Ctrl+Q</KeyBadge>로 바꾸고,
            정렬 키를 변경하는 예시입니다:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_panel": {
      "quit": ["ctrl+q"],
      "sort_by_name": ["ctrl+1"],
      "sort_by_size": ["ctrl+2"],
      "sort_by_date": ["ctrl+3"]
    }
  }
}`}
            </pre>
          </div>

          <p className="text-zinc-400 mb-4">
            편집기에서 저장 키를 <KeyBadge>Ctrl+Shift+S</KeyBadge>로 바꾸는 예시:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_editor": {
      "save": ["ctrl+shift+s"]
    }
  }
}`}
            </pre>
          </div>

          <p className="text-zinc-400 mb-4">
            하나의 액션에 여러 키를 바인딩할 수도 있습니다:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_panel": {
      "quit": ["ctrl+q", "ctrl+w"]
    }
  }
}`}
            </pre>
          </div>

          <TipBox>
            변경하지 않은 액션은 기본 단축키가 그대로 유지됩니다.
            모든 키를 다시 지정할 필요 없이 바꾸고 싶은 것만 적으면 됩니다.
          </TipBox>

          <TipBox variant="warning">
            같은 컨텍스트 내에서 하나의 키를 두 개 이상의 액션에 중복 지정하면
            어떤 액션이 실행될지 예측할 수 없으니 주의하세요.
          </TipBox>

          <TipBox variant="note">
            설정은 <code className={codeStyle}>~/.aimi/</code> 폴더에
            저장되어 다음에 aimi를 실행할 때도 유지됩니다.
          </TipBox>
        </>
      ) : (
        <>
          <p className="text-zinc-400 mb-6 leading-relaxed">
            Customize aimi's appearance and behavior to your liking.
            Change themes (colors), diff view styles, keybindings, and more from the settings file.
          </p>

          <SectionHeading id="settings-open" level={3}>Opening Settings</SectionHeading>
          <StepByStep steps={[
            {
              title: 'Press the backtick (`) key',
              description: (
                <span>
                  Press <KeyBadge>`</KeyBadge> (backtick — the key to the left of 1, same key as tilde ~) to open settings.
                </span>
              ),
            },
            {
              title: 'Select a setting',
              description: (
                <span>
                  Use <KeyBadge>{'\u2191'}</KeyBadge><KeyBadge>{'\u2193'}</KeyBadge> to choose the setting you want to change.
                </span>
              ),
            },
            {
              title: 'Change the value',
              description: (
                <span>
                  Use <KeyBadge>{'\u2190'}</KeyBadge><KeyBadge>{'\u2192'}</KeyBadge> left/right arrows to change values.
                  For example, when choosing a theme, you can preview different themes with the arrow keys.
                </span>
              ),
            },
            {
              title: 'Press Enter to save or Esc to cancel',
              description: (
                <span>
                  Press <KeyBadge>Enter</KeyBadge> to save changes.
                  Press <KeyBadge>Esc</KeyBadge> to exit without saving.
                </span>
              ),
            },
          ]} />

          <SectionHeading id="theme-change" level={3}>Changing Themes (Colors)</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Change aimi's color scheme. If you don't like the default theme, try a different one.
          </p>
          <p className="text-zinc-400 mb-4">
            Theme files are automatically generated when you first run aimi.
            They're located in the <code className={codeStyle}>~/.aimi/themes/</code> folder.
          </p>

          <TipBox>
            For more fine-grained color customization, you can directly edit the JSON files.
            But for beginners, the built-in themes from the settings screen are more than enough.
          </TipBox>

          {/* ── Keybinding Customization ── */}
          <SectionHeading id="keybinding-custom" level={3}>Customizing Keybindings</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Every keyboard shortcut in aimi can be remapped to any key you prefer.
            Edit the <code className={codeStyle}>keybindings</code> section
            in <code className={codeStyle}>~/.aimi/settings.json</code>.
          </p>

          <SectionHeading id="keybinding-howto" level={3}>How to Change Keybindings</SectionHeading>
          <StepByStep steps={[
            {
              title: 'Open settings.json',
              description: (
                <span>
                  Navigate to <code className={codeStyle}>~/.aimi/</code> in aimi,
                  then press <KeyBadge>E</KeyBadge> on <code className={codeStyle}>settings.json</code> to edit it.
                </span>
              ),
            },
            {
              title: 'Find or add the keybindings section',
              description: (
                <span>
                  Look for the <code className={codeStyle}>"keybindings"</code> section.
                  Only add the contexts and actions you want to change —
                  everything else keeps its default binding.
                </span>
              ),
            },
            {
              title: 'Save to apply instantly',
              description: (
                <span>
                  Press <KeyBadge>Ctrl+S</KeyBadge> to save. Changed keybindings take effect immediately —
                  even the on-screen key hints update right away without closing the editor.
                </span>
              ),
            },
          ]} />

          <SectionHeading id="keybinding-format" level={3}>Key Notation</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Key combinations are written as <code className={codeStyle}>"modifier+key"</code>.
            Names are case-insensitive, and multiple modifiers can be combined.
          </p>
          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">Combination format</p>
          <p className="text-zinc-400 mb-3 text-sm">
            Connect modifiers and keys with <code className={codeStyle}>+</code>. You can also use a single key without any modifier.
          </p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">Notation</th>
                  <th className="pb-2 text-zinc-300 font-semibold">Actual key input</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"q"</td><td className="py-1.5">Q key alone</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"esc"</td><td className="py-1.5">Escape key alone</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"f5"</td><td className="py-1.5">F5 key alone</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+s"</td><td className="py-1.5">Hold Ctrl, press S</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"shift+up"</td><td className="py-1.5">Hold Shift, press Up Arrow</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"alt+enter"</td><td className="py-1.5">Hold Alt, press Enter</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+shift+a"</td><td className="py-1.5">Hold Ctrl + Shift, press A</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">"ctrl+alt+delete"</td><td className="py-1.5">Hold Ctrl + Alt, press Delete</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">"ctrl+shift+f12"</td><td className="py-1.5">Hold Ctrl + Shift, press F12</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">Modifier Keys</p>
          <p className="text-zinc-400 mb-2 text-sm">Placed before + to create key combinations. Multiple modifiers can be chained.</p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">Notation</th>
                  <th className="pb-2 text-zinc-300 font-semibold">Key</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">ctrl or control</td><td className="py-1.5">Control (Ctrl) key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">shift</td><td className="py-1.5">Shift key</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">alt</td><td className="py-1.5">Alt key (Option on macOS)</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">Special Keys</p>
          <div className={jsonBlockStyle + ' mb-6'}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">Notation</th>
                  <th className="pb-2 text-zinc-300 font-semibold">Key</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">up, down, left, right</td><td className="py-1.5">Arrow keys</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">enter or return</td><td className="py-1.5">Enter key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">esc or escape</td><td className="py-1.5">Escape key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">tab</td><td className="py-1.5">Tab key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">space</td><td className="py-1.5">Space bar</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">backspace</td><td className="py-1.5">Backspace key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">delete or del</td><td className="py-1.5">Delete key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">home</td><td className="py-1.5">Home key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">end</td><td className="py-1.5">End key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">pageup</td><td className="py-1.5">Page Up key</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">pagedown</td><td className="py-1.5">Page Down key</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">f1, f2, ... f12</td><td className="py-1.5">Function keys</td></tr>
              </tbody>
            </table>
          </div>

          <p className="text-zinc-400 mb-2 font-semibold text-zinc-300">Character & Symbol Keys</p>
          <div className={jsonBlockStyle}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold w-2/5">Notation</th>
                  <th className="pb-2 text-zinc-300 font-semibold">Key</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">a ~ z</td><td className="py-1.5">Letters (write lowercase; both cases are recognized)</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">0 ~ 9</td><td className="py-1.5">Number keys</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">* ; ' ` . / - = [ ] etc.</td><td className="py-1.5">Symbol keys (write the character as-is)</td></tr>
              </tbody>
            </table>
          </div>

          <TipBox>
            For letter keys, just write lowercase. For example, <code className={codeStyle}>"n"</code> will
            match both <KeyBadge>N</KeyBadge> and <KeyBadge>Shift+N</KeyBadge> automatically.
          </TipBox>

          <SectionHeading id="keybinding-contexts" level={3}>Available Contexts</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Each screen has its own set of keybindings.
            Use the context name for the screen you want to customize:
          </p>
          <div className={jsonBlockStyle}>
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="pb-2 text-zinc-300 font-semibold">Context</th>
                  <th className="pb-2 text-zinc-300 font-semibold">Screen</th>
                </tr>
              </thead>
              <tbody className="text-zinc-400">
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_panel</td><td className="py-1.5">Main file list</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_editor</td><td className="py-1.5">File editor</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">file_info</td><td className="py-1.5">File info dialog</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">system_info</td><td className="py-1.5">System info dialog</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">search_result</td><td className="py-1.5">Search results</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">advanced_search</td><td className="py-1.5">Advanced search dialog</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">diff_screen</td><td className="py-1.5">Folder diff list</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">diff_file_view</td><td className="py-1.5">File content diff</td></tr>
                <tr className="border-b border-zinc-800"><td className="py-1.5 text-accent-cyan">image_viewer</td><td className="py-1.5">Image viewer</td></tr>
                <tr><td className="py-1.5 text-accent-cyan">process_manager</td><td className="py-1.5">Process manager</td></tr>
              </tbody>
            </table>
          </div>

          <SectionHeading id="keybinding-examples" level={3}>Examples</SectionHeading>
          <p className="text-zinc-400 mb-4">
            Change Quit from <KeyBadge>Q</KeyBadge> to <KeyBadge>Ctrl+Q</KeyBadge> and
            remap sorting keys:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_panel": {
      "quit": ["ctrl+q"],
      "sort_by_name": ["ctrl+1"],
      "sort_by_size": ["ctrl+2"],
      "sort_by_date": ["ctrl+3"]
    }
  }
}`}
            </pre>
          </div>

          <p className="text-zinc-400 mb-4">
            Change the editor save key to <KeyBadge>Ctrl+Shift+S</KeyBadge>:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_editor": {
      "save": ["ctrl+shift+s"]
    }
  }
}`}
            </pre>
          </div>

          <p className="text-zinc-400 mb-4">
            Bind multiple keys to the same action:
          </p>
          <div className={jsonBlockStyle}>
            <pre className="text-zinc-300">
{`{
  "keybindings": {
    "file_panel": {
      "quit": ["ctrl+q", "ctrl+w"]
    }
  }
}`}
            </pre>
          </div>

          <TipBox>
            Actions you don't override keep their default keybindings.
            You only need to specify the ones you want to change.
          </TipBox>

          <TipBox variant="warning">
            Avoid binding the same key to multiple actions within the same context —
            the behavior would be unpredictable.
          </TipBox>

          <TipBox variant="note">
            Settings are saved in the <code className={codeStyle}>~/.aimi/</code> folder
            and persist across aimi sessions.
          </TipBox>
        </>
      )}
    </section>
  )
}
