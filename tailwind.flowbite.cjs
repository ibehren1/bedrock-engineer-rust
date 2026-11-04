// The flowbite 2 Tailwind plugin, adapted to Tailwind CSS 4. With Tailwind CSS 3 the app used
// it (through flowbite-react 0.10) for two things, and this file keeps both:
//
// 1. Theme config: Flowbite's colour palette in place of Tailwind's (blue-600 is #1C64F2, not
//    Tailwind's #2563EB), plus shadow-sm-light and h-modal. index.css loads this file with
//    @plugin for that. The plugin's safelist and darkMode are left out: Tailwind CSS 4 has no
//    safelist, and the app's own darkMode (tailwind.config.js) always won over the plugin's.
//
// 2. Base styles for form controls and tooltips: the select chevron, checkbox and radio
//    drawing, tooltip arrows. Tailwind CSS 3 put these and the utilities in one cascade, so a
//    rule like `select:not([size])` (padding-right for the chevron) outranked `.px-2` by
//    specificity. Tailwind CSS 4 puts base styles in a layer below utilities, where any
//    utility wins. So @plugin doesn't emit them (`base` is off); flowbiteBaseCss() renders
//    them on their own, and the Vite config writes them to a file that index.css imports into
//    the utilities layer, ahead of Tailwind's utilities: specificity decides again, as before.
//
// The plugin also draws the chevron and tick as SVG data URIs from theme('colors.…').
// Tailwind CSS 4 answers theme() with a CSS variable, which never resolves inside an image,
// so colour lookups are answered from the plugin's own palette, as Tailwind CSS 3 did.
const plugin = require('tailwindcss/plugin')
const flowbite = require('flowbite/plugin')

const { handler, config } = flowbite()
const { colors, ...extend } = config.theme.extend

/** theme('colors.blue.600') → '#1C64F2', from the palette the plugin itself installs. */
const paletteColor = (path) => {
  const match = /^colors\.([a-z]+)\.?(\d+)?$/.exec(path)
  if (!match) return undefined
  const entry = colors[match[1]]
  return typeof entry === 'string' ? entry : entry?.[match[2]]
}

module.exports = plugin.withOptions(
  ({ base = false } = {}) =>
    (api) => {
      if (!base) return
      handler({
        ...api,
        theme: (path, fallback) => paletteColor(path) ?? api.theme(path, fallback)
      })
    },
  () => ({ theme: { extend: { ...extend, colors } } })
)
