/**
 * 「今ダークな背景で表示されているか」を判定する唯一の場所。
 *
 * この関数が存在する理由: アプリは <html data-theme="..."> で外観を切り替えるが、
 * 以前は 19 箇所が `document.documentElement.classList.contains('dark')` を見ていた。
 * `dark` という class はどこにも付与されないため、すべて常に false であり、
 * トークン分析のグラフ・Mermaid 図・コードインタプリタはダーク表示にならなかった。
 *
 * React の外（モジュール初期化時や Mermaid の設定など）からも呼べるよう、
 * フックではなく素の関数にしている。React 内では useTheme() を使うとよい。
 */

/**
 * ダークなキャンバス上に描画される外観の一覧。
 * tailwind.config.js の darkMode 配列と必ず一致させること。片方だけ増やすと、
 * `dark:` ユーティリティは効くのに Monaco や Chart.js は明るいまま、という
 * ずれ方をする。
 */
const DARK_APPEARANCES: ReadonlySet<string> = new Set(['charcoal', 'dark'])

/** <html data-theme> を読み、ダーク表示かどうかを返す。 */
export const isDarkAppearance = (): boolean => {
  if (typeof document === 'undefined') return false
  return DARK_APPEARANCES.has(document.documentElement.dataset.theme ?? '')
}

/** 解決済みテーマ名を渡して判定する（値を既に持っている場合に使う）。 */
export const isDarkAppearanceName = (theme: string | undefined | null): boolean =>
  DARK_APPEARANCES.has(theme ?? '')
