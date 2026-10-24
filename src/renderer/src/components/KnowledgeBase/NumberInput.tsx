import React, { useEffect, useState } from 'react'

type Props = Omit<React.InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'type'> & {
  value: number
  min: number
  max: number
  onChange: (value: number) => void
}

/** `text` as a whole number within `[min, max]`, or `undefined` if it isn't one. */
export function parseInRange(text: string, min: number, max: number): number | undefined {
  if (text.trim() === '') return undefined
  const n = Number(text)
  if (!Number.isFinite(n)) return undefined
  const v = Math.round(n)
  return v >= min && v <= max ? v : undefined
}

export function clampNumber(text: string, min: number, max: number, fallback: number): number {
  const n = Number(text)
  if (text.trim() === '' || !Number.isFinite(n)) return fallback
  return Math.min(max, Math.max(min, Math.round(n)))
}

/**
 * A number field that can be emptied while typing: in-range values apply as you type, anything
 * else is clamped (or, if empty, put back) when the field loses focus or Enter is pressed. Enter
 * never submits a surrounding form.
 */
export const NumberInput: React.FC<Props> = ({
  value,
  min,
  max,
  onChange,
  onBlur,
  onKeyDown,
  ...rest
}) => {
  const [text, setText] = useState(String(value))
  useEffect(() => setText(String(value)), [value])

  const commit = () => {
    const v = clampNumber(text, min, max, value)
    setText(String(v))
    if (v !== value) onChange(v)
  }

  return (
    <input
      {...rest}
      type="number"
      inputMode="numeric"
      min={min}
      max={max}
      value={text}
      onChange={(e) => {
        setText(e.target.value)
        const v = parseInRange(e.target.value, min, max)
        if (v !== undefined && v !== value) onChange(v)
      }}
      onBlur={(e) => {
        commit()
        onBlur?.(e)
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          commit()
        }
        onKeyDown?.(e)
      }}
    />
  )
}
