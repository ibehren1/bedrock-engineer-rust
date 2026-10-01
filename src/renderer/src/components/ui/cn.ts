import { clsx, type ClassValue } from 'clsx'
import { extendTailwindMerge } from 'tailwind-merge'

/**
 * tailwind-merge には、このリポジトリ独自のトークン系ユーティリティを教えておく
 * 必要がある。そうしないと `rounded-control` と `rounded-container` が競合すると
 * 判定されず、両方が出力されて後勝ちになる（意図しない角丸になる）。
 */
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      // 角丸トークン（tailwind.config.js の borderRadius に対応）
      rounded: [{ rounded: ['control', 'container'] }],
      // 文字サイズのセマンティックエイリアス（fontSize に対応）
      'font-size': [{ text: ['title', 'heading', 'subheading', 'body', 'label', 'micro'] }],
      // 面・文字・境界の色トークン
      'bg-color': [{ bg: ['canvas', 'surface', 'surface-2', 'raised', 'sunken'] }],
      'text-color': [{ text: ['ink', 'ink-muted', 'ink-faint'] }],
      'border-color': [{ border: ['subtle', 'faint', 'strong'] }],
      shadow: [{ shadow: ['raised'] }]
    }
  }
})

/**
 * クラス名を結合し、競合する Tailwind ユーティリティは後から渡したものを勝たせる。
 * これがないと `className` で上書きしたつもりのユーティリティが効かない。
 */
export const cn = (...inputs: ClassValue[]): string => twMerge(clsx(inputs))
