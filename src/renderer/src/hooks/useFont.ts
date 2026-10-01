import { useState, useEffect, useCallback } from 'react'

/** UI テキストの書体 */
export type AppFontSans = 'inter' | 'geist' | 'system'

/** コード・ID・パスなど等幅テキストの書体 */
export type AppFontMono = 'jetbrains' | 'geist-mono' | 'system'

export const DEFAULT_FONT_SANS: AppFontSans = 'inter'
export const DEFAULT_FONT_MONO: AppFontMono = 'jetbrains'

const SANS_VALUES: readonly AppFontSans[] = ['inter', 'geist', 'system']
const MONO_VALUES: readonly AppFontMono[] = ['jetbrains', 'geist-mono', 'system']

/**
 * 保存値を検証する。未知の値なら既定値にフォールバックする。
 * （設定ファイルを手で編集された場合や、書体を削除したあとの古い値のため）
 */
const coerce = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
  allowed.includes(value as T) ? (value as T) : fallback

/** <html> の data-font-sans / data-font-mono 属性に反映する */
const applyFonts = (sans: AppFontSans, mono: AppFontMono) => {
  if (typeof document !== 'undefined') {
    document.documentElement.dataset.fontSans = sans
    document.documentElement.dataset.fontMono = mono
  }
}

const readStoredSans = (): AppFontSans =>
  coerce(window.store?.get('appFontSans' as any), SANS_VALUES, DEFAULT_FONT_SANS)

const readStoredMono = (): AppFontMono =>
  coerce(window.store?.get('appFontMono' as any), MONO_VALUES, DEFAULT_FONT_MONO)

/**
 * 書体設定を管理するフック。テーマ（useTheme）とは独立しており、
 * 外観を切り替えても書体は変わらない。
 * - fontSans / fontMono: ユーザー設定
 * - setFontSans / setFontMono: 設定ファイル（config.json）と <html data-font-*> に反映
 */
export const useFont = () => {
  const [fontSans, setFontSansState] = useState<AppFontSans>(() => readStoredSans())
  const [fontMono, setFontMonoState] = useState<AppFontMono>(() => readStoredMono())

  useEffect(() => {
    applyFonts(fontSans, fontMono)
  }, [fontSans, fontMono])

  const setFontSans = useCallback((font: AppFontSans) => {
    setFontSansState(font)
    window.store?.set('appFontSans' as any, font)
  }, [])

  const setFontMono = useCallback((font: AppFontMono) => {
    setFontMonoState(font)
    window.store?.set('appFontMono' as any, font)
  }, [])

  return { fontSans, setFontSans, fontMono, setFontMono }
}
