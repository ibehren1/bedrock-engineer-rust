/**
 * 共有 UI プリミティブ。
 *
 * ここに入れる基準: 同じ className の文字列が複数箇所に逐語コピーされていて、
 * かつ意味のある単位であること。ページ固有の一点物は各ページに置いたままにする。
 *
 * 色・角丸・影は必ずトークン経由で指定する（bg-surface / text-ink /
 * border-subtle / rounded-control / shadow-raised）。素のパレット（gray 系）と
 * dark バリアントの組み合わせを書くと、5 つの外観（light / newspaper / dim /
 * charcoal / dark）に
 * 追従できない。とくに charcoal はアクセントがアンバーなので、アクセント面の
 * 文字色に text-white を直接書くと読めなくなる（text-accent-fg を使う）。
 */
export { cn } from './cn'
export { Button, type ButtonProps, type ButtonVariant, type ButtonSize } from './Button'
export { Input, Textarea, Select, Label, type LabelProps } from './Field'
export { Card, Badge, StatusDot, type BadgeProps, type Tone } from './Surface'
