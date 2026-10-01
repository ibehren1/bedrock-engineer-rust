/**
 * `JSON.stringify(data, null, 2)` as HTML with the keys and values wrapped in colored spans,
 * for `dangerouslySetInnerHTML`.
 *
 * The data is tool / task output (model- or web-controlled), so `&`, `<` and `>` are escaped
 * before any markup is added: a string like `<img src=x onerror=…>` must render as text, not
 * become an element in the app's document.
 */
export function highlightJson(data: unknown): string {
  const jsonStr = JSON.stringify(data, null, 2) ?? String(data)
  const escaped = jsonStr.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

  return (
    escaped
      // キーの色を変更（"key": の部分）
      .replace(/"([^"]+)":/g, '<span class="text-accent">"$1"</span>:')
      // 文字列値の色を変更（": "value" の部分）
      .replace(/: "([^"]*)"/g, ': <span class="text-success">"$1"</span>')
      // 数値の色を変更
      // `\\d` matched a literal backslash followed by "d", so numbers were
      // never actually highlighted.
      .replace(/: (\d+)(,?)/g, ': <span class="text-accent">$1</span>$2')
      // ブール値の色を変更
      .replace(/: (true|false)(,?)/g, ': <span class="text-warning">$1</span>$2')
      // nullの色を変更
      .replace(/: (null)(,?)/g, ': <span class="text-purple-600 dark:text-purple-400">$1</span>$2')
  )
}
