import React from 'react'
import { cn } from './cn'

/**
 * フォーム部品。
 *
 * 移行前は input が 3 種類（rounded と rounded-lg、p-2 と p-2.5、
 * dark:bg-gray-800 と dark:bg-gray-700、フォーカスリングの有無）並存していた。
 * ラベルも 2 種類（text-xs 系が 53 箇所、text-sm font-medium 系が 11 箇所）あった。
 *
 * 色は面・文字・境界トークン経由なので、5 つの外観すべてで正しくなる。
 * 余白は px-2 py-1 で、flowbiteTheme.ts の入力系 md サイズと同値。素の入力と
 * Flowbite の TextInput が隣に並んでも行の高さが揃う。
 */
const CONTROL_BASE =
  'w-full rounded-control border border-strong bg-surface px-2 py-1 text-sm text-ink ' +
  'placeholder:text-ink-faint ' +
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:border-accent ' +
  'disabled:cursor-not-allowed disabled:opacity-50'

export const Input: React.FC<React.InputHTMLAttributes<HTMLInputElement>> = ({
  className,
  ...props
}) => <input className={cn(CONTROL_BASE, className)} {...props} />

export const Textarea: React.FC<React.TextareaHTMLAttributes<HTMLTextAreaElement>> = ({
  className,
  ...props
}) => <textarea className={cn(CONTROL_BASE, 'resize-y', className)} {...props} />

export const Select: React.FC<React.SelectHTMLAttributes<HTMLSelectElement>> = ({
  className,
  ...props
}) => <select className={cn(CONTROL_BASE, 'pr-8', className)} {...props} />

export interface LabelProps extends React.LabelHTMLAttributes<HTMLLabelElement> {
  /** 補足説明。ラベル直下に一段弱いトーンで表示する。 */
  hint?: React.ReactNode
}

export const Label: React.FC<LabelProps> = ({ className, hint, children, ...props }) => (
  <label className={cn('block text-label text-ink-muted mb-1', className)} {...props}>
    {children}
    {hint ? <span className="mt-0.5 block text-xs font-normal text-ink-faint">{hint}</span> : null}
  </label>
)
