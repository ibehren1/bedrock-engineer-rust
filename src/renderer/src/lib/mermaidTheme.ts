import mermaid from 'mermaid'

/**
 * Mermaid の配色を、アプリのトークンから組み立てる。
 *
 * mermaid の 'default' テーマは青・紫・ピンクを混ぜた多色パレットで、5 つの外観の
 * どれとも合わない。ここでは 'base' テーマに themeVariables を与え、surface から
 * ink へ段階的に混色した濃淡だけで描くグレースケールにする。単色ではなく複数段
 * なので、ノード・サブグラフ・代替行・円グラフの系列を濃さで区別できる。
 *
 * 色相を持ち込まないので、newspaper では紙のグレー、charcoal では暖かいグレーと、
 * その外観のグレーがそのまま図に出る。外観を追加しても図側の設定は不要。
 *
 * mermaid はシングルトンで、initialize はモジュール読み込み時に一度しか走らない。
 * 外観の切り替えに追従させるには再 initialize と再描画が必要なので、
 * subscribeToAppearance() を用意している。
 */

/** <html> に実際に効いているトークン値を読む。 */
const token = (name: string, fallback: string): string => {
  if (typeof document === 'undefined') return fallback
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

const channels = (hex: string): [number, number, number] => {
  const h = hex.replace('#', '')
  const full =
    h.length === 3
      ? h
          .split('')
          .map((c) => c + c)
          .join('')
      : h
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16)) as [number, number, number]
}

const mix = (from: string, to: string, t: number): string => {
  const a = channels(from)
  const b = channels(to)
  const out = a.map((v, i) => Math.round(v + (b[i] - v) * t))
  return '#' + out.map((v) => v.toString(16).padStart(2, '0')).join('')
}

/** WCAG relative luminance. */
const luminance = (hex: string): number => {
  const [r, g, b] = channels(hex).map((v) => {
    const c = v / 255
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

const contrast = (a: string, b: string): number => {
  const la = luminance(a)
  const lb = luminance(b)
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
}

/** 候補のうち、その塗りの上で最もコントラストが高い文字色を選ぶ。 */
const labelOn = (fill: string, candidates: string[]): string =>
  candidates.reduce(
    (best, c) => (contrast(c, fill) > contrast(best, fill) ? c : best),
    candidates[0]
  )

/**
 * ノードの塗りに使う 3 段の濃淡。
 *
 * 面のトークン（surface-2 / raised / sunken）をそのまま使うと、隣り合う段の
 * コントラストが 1.13〜1.32:1 しかなく見分けがつかなかった。面のランプは UI の
 * 陰影用に意図的に狭いので、図には向かない。ここでは surface から ink へ
 * 12% / 30% / 50% 混ぜて、隣接段で最低 1.48:1 を確保している。
 *
 * 混ぜる先が ink なので方向は外観に追従する（明るい外観では暗く、暗い外観では
 * 明るくなる）。文字色は段ごとにコントラストで選ぶ。最も濃い段は中間グレーに
 * なることがあり、ink でも canvas でも 4.5:1 に届かないため、純黒・純白も候補に
 * 入れて最低 4.7:1 を保証している。
 */
const FILL_STEPS = [0.12, 0.3, 0.5] as const

const buildThemeVariables = () => {
  const canvas = token('--canvas', '#f3f4f6')
  const surface = token('--surface', '#ffffff')
  const border = token('--border', '#e5e7eb')
  const ink = token('--ink', '#111827')
  const inkMuted = token('--ink-muted', '#4b5563')
  const fontMono = token('--font-mono', 'ui-monospace, monospace')

  const [fill1, fill2, fill3] = FILL_STEPS.map((t) => mix(surface, ink, t))
  const candidates = [ink, canvas, '#000000', '#ffffff']
  const text1 = labelOn(fill1, candidates)
  const text2 = labelOn(fill2, candidates)
  const text3 = labelOn(fill3, candidates)

  // 線はどの段の塗りの上でも見えるよう、ink 寄りに振る。
  const line = mix(surface, ink, 0.7)

  return {
    background: canvas,
    fontFamily: fontMono,
    fontSize: '12px',

    // 3 段の濃淡。primary が一番淡く、tertiary が一番濃い。
    primaryColor: fill1,
    primaryBorderColor: line,
    primaryTextColor: text1,
    secondaryColor: fill2,
    secondaryBorderColor: line,
    secondaryTextColor: text2,
    tertiaryColor: fill3,
    tertiaryBorderColor: line,
    tertiaryTextColor: text3,

    // 線と文字
    lineColor: line,
    textColor: ink,
    titleColor: ink,
    arrowheadColor: line,

    // フローチャート
    mainBkg: fill1,
    nodeBorder: line,
    nodeTextColor: text1,
    clusterBkg: canvas,
    clusterBorder: border,
    edgeLabelBackground: canvas,
    defaultLinkColor: line,

    // シーケンス図
    actorBkg: fill1,
    actorBorder: line,
    actorTextColor: text1,
    actorLineColor: inkMuted,
    signalColor: ink,
    signalTextColor: ink,
    labelBoxBkgColor: fill2,
    labelBoxBorderColor: line,
    labelTextColor: text2,
    loopTextColor: ink,
    activationBkgColor: fill2,
    activationBorderColor: line,
    sequenceNumberColor: text3,
    noteBkgColor: fill2,
    noteBorderColor: line,
    noteTextColor: text2,

    // ガント / 状態遷移 / クラス図の帯と区切り
    sectionBkgColor: fill1,
    altSectionBkgColor: fill2,
    sectionBkgColor2: fill3,
    gridColor: border,
    todayLineColor: ink,
    taskBkgColor: fill2,
    taskBorderColor: line,
    taskTextColor: text2,
    taskTextOutsideColor: ink,
    taskTextDarkColor: text2,
    taskTextLightColor: text3,
    activeTaskBkgColor: fill3,
    activeTaskBorderColor: ink,
    doneTaskBkgColor: fill1,
    doneTaskBorderColor: border,
    critBorderColor: ink,
    critBkgColor: fill3,

    // 円グラフは 12 系列。隣接系列が紛れないよう淡→濃を往復させる。
    pie1: fill1,
    pie2: fill3,
    pie3: fill2,
    pie4: mix(surface, ink, 0.62),
    pie5: mix(surface, ink, 0.2),
    pie6: mix(surface, ink, 0.4),
    pie7: mix(surface, ink, 0.06),
    pie8: mix(surface, ink, 0.56),
    pie9: mix(surface, ink, 0.24),
    pie10: mix(surface, ink, 0.44),
    pie11: mix(surface, ink, 0.16),
    pie12: mix(surface, ink, 0.36),
    pieStrokeColor: line,
    pieOuterStrokeColor: line,
    pieTitleTextColor: ink,
    pieSectionTextColor: text2,
    pieLegendTextColor: ink
  }
}

/** mermaid を現在の外観で初期化する。 */
export const initMermaid = (): void => {
  mermaid.initialize({
    // syntax error が dom node に勝手に追加されないようにする
    // https://github.com/mermaid-js/mermaid/pull/4359
    suppressErrorRendering: true,
    // HTML labels stay on, but mermaid strips scripts and event handlers from them (DOMPurify):
    // the SVG is injected into the app's own document, so 'loose' would let a diagram run code.
    securityLevel: 'antiscript',
    htmlLabels: true,
    theme: 'base',
    themeVariables: buildThemeVariables()
  })
}

/**
 * <html data-theme> の変化を購読する。mermaid を再初期化したうえで
 * コールバックを呼ぶので、購読側は再描画するだけでよい。
 */
export const subscribeToAppearance = (onChange: () => void): (() => void) => {
  if (typeof document === 'undefined') return () => undefined
  const observer = new MutationObserver(() => {
    initMermaid()
    onChange()
  })
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme']
  })
  return () => observer.disconnect()
}

initMermaid()
