import React from 'react'
import { cn } from './cn'

/**
 * 面と状態表示のプリミティブ。
 *
 * Card は 11 箇所に逐語コピーされていた
 * "flex flex-col p-3 text-sm bg-raised ... rounded border border-subtle"
 * を置き換える。半径は container トークンなので Newspaper では角が落ちる。
 * 内側の余白は p-2.5（10px）。カードが縦に 10 枚並ぶ画面が複数あり、p-3 だと
 * 枠の余白だけで一画面ぶん食う。
 */
export const Card: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({ className, ...props }) => (
  <div
    className={cn('rounded-container border border-subtle bg-surface p-2.5', className)}
    {...props}
  />
)

export type Tone = 'neutral' | 'success' | 'error' | 'warning' | 'accent'

/**
 * 状態を表す控えめなバッジ。
 * 以前の bg-green-500 / bg-red-700 のベタ塗りではなく、地色を薄い色に落として
 * 文字色でトーンを示す。成功が連続しても画面が騒がしくならず、失敗が目立つ。
 *
 * 地色は -soft / -tint トークン（index.css で color-mix 済み）を使う。
 * bg-success/10 のような不透明度修飾ではなく専用トークンなのは、トークンの値が
 * var(--success) であって Tailwind 側で alpha を差し込めないため。
 *
 * warning が amber ではなく orange になるのは charcoal 外観の都合。あの外観では
 * amber がアクセント色なので、警告色と衝突する。切り替えは index.css の
 * --warning が持っているので、ここは token 名を書くだけでよい。
 */
const TONES: Record<Tone, string> = {
  neutral: 'bg-raised text-ink-muted',
  success: 'bg-success-soft text-success',
  error: 'bg-danger-soft text-danger',
  warning: 'bg-warning-soft text-warning',
  accent: 'bg-accent-tint text-accent'
}

const DOTS: Record<Tone, string> = {
  neutral: 'bg-ink-faint',
  success: 'bg-success',
  error: 'bg-danger',
  warning: 'bg-warning',
  accent: 'bg-accent'
}

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: Tone
}

export const Badge: React.FC<BadgeProps> = ({ tone = 'neutral', className, ...props }) => (
  <span
    className={cn(
      'inline-flex items-center rounded-control px-1.5 py-0.5 text-micro',
      TONES[tone],
      className
    )}
    {...props}
  />
)

/** 5px の状態ドット。大きなチェックマークの置き換え。 */
export const StatusDot: React.FC<{ tone?: Tone; className?: string; title?: string }> = ({
  tone = 'neutral',
  className,
  title
}) => (
  <span
    title={title}
    className={cn('inline-block size-1.5 shrink-0 rounded-full', DOTS[tone], className)}
  />
)
