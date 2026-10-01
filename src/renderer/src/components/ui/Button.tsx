import React from 'react'
import { cn } from './cn'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'subtle'
export type ButtonSize = 'sm' | 'md'

/**
 * アプリ共通のボタン。
 *
 * これまで primary ボタンには 2 つの流儀が並存していた:
 *   px-3 py-1  / bg-blue-500 / rounded            (9 箇所)
 *   px-4 py-2  / bg-blue-600 / rounded-md / shadow-sm / focus ring (9 箇所)
 * さらにフォーカスリングの有無も揃っていなかった。ここに一本化する。
 *
 * 色はアクセントトークン経由なので、charcoal（アンバー）でも文字色が自動的に
 * 追従する。primary に text-white を直接書いてはいけない。danger も同じ理由で
 * text-white ではなく text-canvas を使う。canvas は明るい外観では白系・暗い
 * 外観では黒系になるため、明度が反転する状態色のベタ塗り上でも読める。
 *
 * どの variant も hover で必ず面が動くこと。以前 danger だけ hover 先が
 * bg-danger（=素の状態と同じ）になっていて、押せる要素に見えなかった。
 */
const VARIANTS: Record<ButtonVariant, string> = {
  primary: 'bg-accent text-accent-fg hover:bg-accent-strong',
  secondary: 'border border-strong text-ink hover:bg-raised bg-transparent',
  ghost: 'text-ink-muted hover:bg-raised hover:text-ink bg-transparent',
  subtle: 'bg-raised text-ink hover:bg-sunken',
  danger: 'bg-danger text-canvas hover:bg-danger-strong'
}

const SIZES: Record<ButtonSize, string> = {
  sm: 'px-2 py-0.5 text-xs gap-1',
  md: 'px-2 py-1 text-sm gap-1.5'
}

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: ButtonSize
}

export const Button: React.FC<ButtonProps> = ({
  variant = 'secondary',
  size = 'md',
  className,
  type = 'button',
  ...props
}) => (
  <button
    type={type}
    className={cn(
      'inline-flex items-center justify-center rounded-control font-medium transition-colors',
      // The nav rail and most buttons had no focus ring at all.
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent',
      'disabled:cursor-not-allowed disabled:opacity-50',
      VARIANTS[variant],
      SIZES[size],
      className
    )}
    {...props}
  />
)
