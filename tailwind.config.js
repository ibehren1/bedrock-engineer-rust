/** @type {import('tailwindcss').Config} */
const flowbite = require('flowbite-react/tailwind')

module.exports = {
  content: [
    './src/renderer/src/**/*.{js,jsx,ts,tsx}',
    './src/renderer/index.html',
    flowbite.content()
  ],
  theme: {
    extend: {
      // Semantic colours, backed by the CSS variables in src/renderer/index.css.
      // A theme sets those variables; nothing in this file changes per theme.
      //
      // Text colours are named `ink` because `text-*` is shared between colour
      // and font-size, and `text-body` is already a size.
      colors: {
        canvas: 'var(--canvas)',
        surface: {
          DEFAULT: 'var(--surface)',
          2: 'var(--surface-2)'
        },
        raised: 'var(--raised)',
        sunken: 'var(--sunken)',
        // The moving part of a switch. Not a surface: it must stay lighter than
        // its track in every appearance. See the note in index.css.
        knob: 'var(--knob)',
        ink: {
          DEFAULT: 'var(--ink)',
          muted: 'var(--ink-muted)',
          faint: 'var(--ink-faint)'
        },
        // `fg` is what to draw ON the accent. It is dark in charcoal, where the
        // accent is amber, so white-on-accent is never a safe assumption.
        accent: {
          DEFAULT: 'var(--accent)',
          strong: 'var(--accent-strong)',
          soft: 'var(--accent-soft)',
          tint: 'var(--accent-tint)',
          'tint-strong': 'var(--accent-tint-strong)',
          fg: 'var(--accent-fg)'
        },
        // Status is tokenised so charcoal can move warning off amber, which is
        // its accent. `-soft` is the tinted background for a quiet badge.
        success: {
          DEFAULT: 'var(--success)',
          soft: 'var(--success-soft)',
          'soft-strong': 'var(--success-soft-strong)',
          strong: 'var(--success-strong)'
        },
        danger: {
          DEFAULT: 'var(--danger)',
          soft: 'var(--danger-soft)',
          'soft-strong': 'var(--danger-soft-strong)',
          strong: 'var(--danger-strong)'
        },
        warning: {
          DEFAULT: 'var(--warning)',
          soft: 'var(--warning-soft)',
          'soft-strong': 'var(--warning-soft-strong)',
          strong: 'var(--warning-strong)'
        }
      },

      borderColor: {
        subtle: 'var(--border)',
        faint: 'var(--border-soft)',
        strong: 'var(--border-strong)'
      },
      divideColor: {
        subtle: 'var(--border)',
        faint: 'var(--border-soft)'
      },
      ringColor: {
        accent: 'var(--accent)'
      },
      outlineColor: {
        accent: 'var(--accent)'
      },

      // Two radii plus rounded-full for avatars and counts. Tighter than the
      // stock scale on purpose: a 3px control edge reads as machined, an 8px one
      // reads as a consumer app. A theme can square itself from these.
      borderRadius: {
        control: 'var(--radius-control)',
        container: 'var(--radius-container)'
      },

      boxShadow: {
        raised: 'var(--shadow-raised)'
      },

      // MUST stay variable references, not literal stacks. Preflight compiles
      // theme('fontFamily.sans') into its own `html` rule, so a literal list
      // would bake one face in and the font settings could never override it.
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)']
      },

      // A tight, tool-oriented scale. Interface chrome sits at 12.5px, the size
      // VS Code, Zed and Linear use for dense UI; the previous 14px read as a
      // consumer app. Every step carries its own line-height and tracking.
      //
      // Deliberately NOT shrunk: the chat transcript. That is reading material,
      // and components/Markdown/styles.module.css sets its own 14px/1.6.
      fontSize: {
        micro: ['0.625rem', { lineHeight: '1.4', letterSpacing: '0.07em', fontWeight: '600' }], // 10
        label: ['0.6875rem', { lineHeight: '1.4', fontWeight: '500' }], // 11
        xs: ['0.6875rem', { lineHeight: '1.4' }], // 11
        body: ['0.78125rem', { lineHeight: '1.5' }], // 12.5
        sm: ['0.78125rem', { lineHeight: '1.45' }], // 12.5 — the chrome default
        subheading: ['0.84375rem', { lineHeight: '1.4', fontWeight: '600' }], // 13.5
        base: ['0.875rem', { lineHeight: '1.5' }], // 14
        heading: [
          '0.9375rem',
          { lineHeight: '1.35', letterSpacing: '-0.006em', fontWeight: '600' }
        ], // 15
        lg: ['0.9375rem', { lineHeight: '1.4', letterSpacing: '-0.006em' }], // 15
        title: ['1.0625rem', { lineHeight: '1.3', letterSpacing: '-0.012em', fontWeight: '600' }], // 17
        xl: ['1.0625rem', { lineHeight: '1.3', letterSpacing: '-0.012em' }], // 17
        '2xl': ['1.25rem', { lineHeight: '1.25', letterSpacing: '-0.018em' }] // 20
      },

      // gradient-x and its keyframes are gone: the seven animated
      // gradient-clipped "Thinking"/"Reasoning" words that were its only users
      // are now static accent text, which also removed seven continuous
      // repaints.
      animation: {
        'todo-flash': 'todoFlash 0.5s ease-in-out 2'
      },
      keyframes: {
        todoFlash: {
          '0%, 100%': { opacity: '1', transform: 'scale(1)' },
          '50%': { opacity: '0.4', transform: 'scale(1.3)' }
        }
      }
    }
  },
  plugins: [flowbite.plugin()],
  // Surfaces, borders, text and accent no longer go through `dark:` at all —
  // they are tokens, so every appearance works without a variant. This is kept
  // only for the vendor CSS (Flowbite, react-cmdk) and the handful of status
  // colours that still carry a dark: partner.
  //
  // Both dark-canvas appearances are listed. The array form of `variant` is
  // required: the tempting one-liner
  // `['selector', '[data-theme="dark"], [data-theme="charcoal"]']` is a trap,
  // because Tailwind interpolates `&` against the whole string and the
  // descendant combinator then binds to the last selector only.
  darkMode: ['variant', ['&:is([data-theme="dark"] *)', '&:is([data-theme="charcoal"] *)']]
}
