import type { CustomFlowbiteTheme } from 'flowbite-react'

/**
 * Flowbite の既定テーマ上書き。
 *
 * これまで <Flowbite theme> は一度も設置されておらず、42 箇所の import すべてが
 * 素の既定値で描画されていた。既定値はマーケティングサイト向けの余白設計なので、
 * 密度の高い開発ツールには合わない。とくに Accordion は上下 20px（p-5）の余白と
 * 24px の矢印で 12px の中身を包むため、ツール呼び出しが 20 回あるターンでは
 * 本文が出るまでに 800px 近くが枠だけで埋まっていた。
 *
 * ここで決めることは 2 つある。
 *
 * 1. 色・境界・角丸はすべてデザイントークン。
 *    既定値の素の Tailwind パレット（gray 系・cyan 系）と dark バリアントは
 *    ひとつも残さない。dark バリアントはテーマに追従できないので
 *    （追従できないことがこのトークン制度の存在理由）、
 *    面は bg-canvas / bg-surface / bg-surface-2 / bg-raised / bg-sunken、
 *    罫線は border-subtle / -faint / -strong、文字は text-ink / -muted / -faint、
 *    強調は bg-accent + text-accent-fg、状態色は success / danger / warning、
 *    角丸は rounded-control / rounded-container に寄せる。
 *    charcoal ではアクセントがアンバーなので、アクセント面の文字色に
 *    text-white を書いてはいけない（text-accent-fg が正解）。同じ理由で
 *    danger / success / warning のベタ塗り上は text-canvas を使う。canvas は
 *    明るいテーマでは白系・暗いテーマでは黒系になるため、明度が反転する
 *    状態色の上でも読める唯一のトークンになる。
 *
 * 2. 密度はツール基準。
 *    行の高さはテキスト + 上下 2〜4px を目安にし、
 *    コントロールは px-2 py-1 / text-sm（12.5px）を既定サイズとする。
 *    Modal は p-3、Tooltip は px-1.5 py-0.5 / text-micro、Accordion の見出しは
 *    px-2 py-1.5 / text-xs、矢印は 14px。既定値の 1/2 前後になる。
 *
 * アプリが実際に使う Flowbite コンポーネントはすべてここで面倒を見る。
 * Modal / Tooltip / Button / Label / ToggleSwitch / Select / Dropdown /
 * TextInput / Accordion / Kbd / Textarea が実使用分で、Table・Spinner・
 * HelperText は Button や TextInput が内部で描画する・将来使う想定の分。
 */

/** アクセントのベタ塗り。文字色は必ず accent-fg（charcoal では暗色になる）。 */
const FILL_ACCENT =
  'border border-transparent bg-accent text-accent-fg focus:ring-2 focus:ring-accent enabled:hover:bg-accent-strong'

const FILL_DANGER =
  'border border-transparent bg-danger text-canvas focus:ring-2 focus:ring-accent enabled:hover:bg-danger-strong'
const FILL_SUCCESS =
  'border border-transparent bg-success text-canvas focus:ring-2 focus:ring-accent enabled:hover:bg-success-strong'
const FILL_WARNING =
  'border border-transparent bg-warning text-canvas focus:ring-2 focus:ring-accent enabled:hover:bg-warning-strong'

/** 入力系の枠色。size は各コンポーネントの sizes 側で持つ。 */
const CONTROL_NEUTRAL =
  'border-strong bg-surface text-ink placeholder-ink-faint focus:border-accent focus:ring-accent'
const CONTROL_DANGER =
  'border-danger bg-danger-soft text-ink placeholder-ink-faint focus:border-danger focus:ring-danger'
const CONTROL_SUCCESS =
  'border-success bg-success-soft text-ink placeholder-ink-faint focus:border-success focus:ring-success'
const CONTROL_WARNING =
  'border-warning bg-warning-soft text-ink placeholder-ink-faint focus:border-warning focus:ring-warning'
const CONTROL_ACCENT =
  'border-accent bg-accent-tint text-ink placeholder-ink-faint focus:border-accent focus:ring-accent'

/** 入力系の 3 サイズ。既定の p-2 / p-2.5 / p-4 を詰めたもの。 */
const CONTROL_SIZES = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2 py-1 text-sm',
  lg: 'px-2.5 py-1.5 text-sm'
}

/** 浮いている面（Tooltip・Dropdown・Popover）は同じ見た目に揃える。 */
const FLOATING_SURFACE = 'border border-subtle bg-surface text-ink'

/**
 * Flowbite の style スロットは dark / light / auto の 3 択だが、どれも同じ値を返す。
 * 「暗い配色の吹き出し」は既定では黒固定で、外観が 5 つあるとどれかで面の階層と
 * 噛み合わなくなる。テーマトークンを使う以上、外観の明暗はトークン側が決めるので
 * ここで分岐する意味がない。
 */
const everyStyle = (value: string) => ({ dark: value, light: value, auto: value })

export const flowbiteTheme: CustomFlowbiteTheme = {
  accordion: {
    root: {
      base: 'divide-y divide-subtle border-subtle',
      flush: {
        off: 'rounded-container border',
        on: 'border-b'
      }
    },
    content: {
      // 既定は p-5。中身は 12.5px なので上を落とし、左右も見出しと揃える。
      base: 'px-2 pb-2 pt-0 first:rounded-t-container last:rounded-b-container bg-canvas'
    },
    title: {
      arrow: {
        // 既定は h-6 w-6（24px）。11px の行に対して大きすぎた。
        base: 'h-3.5 w-3.5 shrink-0',
        open: { off: '', on: 'rotate-180' }
      },
      base: 'flex w-full items-center justify-between px-2 py-1.5 text-left text-xs font-medium text-ink-muted first:rounded-t-container last:rounded-b-container',
      flush: {
        // 既定の focus:ring-4 は 4px のリングで行が動いて見えるため 2px に。
        // 色は ring-accent。ring-subtle というユーティリティは存在しない
        // （ringColor の拡張は accent だけ）ので書いてはいけない。
        off: 'hover:bg-raised focus:outline-none focus:ring-2 focus:ring-accent',
        on: 'bg-transparent'
      },
      heading: '',
      open: {
        off: '',
        on: 'bg-raised text-ink'
      }
    }
  },

  modal: {
    content: {
      base: 'relative h-full w-full p-3 md:h-auto',
      inner:
        'relative flex max-h-[90dvh] flex-col rounded-container border border-subtle bg-surface shadow-raised'
    },
    header: {
      base: 'flex items-start justify-between rounded-t-container border-b border-subtle p-3',
      popup: 'border-b-0 p-3',
      title: 'text-heading text-ink',
      close: {
        base: 'ml-auto inline-flex items-center rounded-control bg-transparent p-1 text-sm text-ink-faint hover:bg-raised hover:text-ink',
        icon: 'h-3.5 w-3.5'
      }
    },
    body: {
      base: 'flex-1 overflow-auto p-3',
      popup: 'pt-0'
    },
    footer: {
      base: 'flex items-center justify-end gap-2 rounded-b-container border-t border-subtle p-3',
      popup: 'border-t'
    }
  },

  tooltip: {
    base: 'absolute z-10 inline-block rounded-control px-1.5 py-0.5 text-micro shadow-raised',
    arrow: {
      base: 'absolute z-10 h-2 w-2 rotate-45',
      style: everyStyle('bg-surface'),
      placement: '-4px'
    },
    style: everyStyle(FLOATING_SURFACE)
  },

  dropdown: {
    arrowIcon: 'ml-1 h-3 w-3',
    content: 'py-0.5 focus:outline-none',
    floating: {
      base: 'z-10 w-fit divide-y divide-faint rounded-container shadow-raised focus:outline-none',
      content: 'py-0.5 text-sm text-ink',
      divider: 'my-0.5 h-px bg-raised',
      header: 'block px-2 py-1 text-xs text-ink-muted',
      item: {
        container: '',
        base: 'flex w-full cursor-pointer items-center justify-start px-2 py-1 text-sm text-ink hover:bg-raised focus:bg-raised focus:outline-none',
        icon: 'mr-1.5 h-3.5 w-3.5'
      },
      arrow: {
        base: 'absolute z-10 h-2 w-2 rotate-45',
        style: everyStyle('bg-surface'),
        placement: '-4px'
      },
      style: everyStyle(FLOATING_SURFACE),
      target: 'w-fit'
    },
    inlineWrapper: 'flex items-center'
  },

  textInput: {
    addon:
      'inline-flex items-center rounded-l-control border border-r-0 border-strong bg-raised px-2 text-sm text-ink-muted',
    field: {
      icon: {
        base: 'pointer-events-none absolute inset-y-0 left-0 flex items-center pl-2',
        svg: 'h-3.5 w-3.5 text-ink-faint'
      },
      rightIcon: {
        base: 'pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2',
        svg: 'h-3.5 w-3.5 text-ink-faint'
      },
      input: {
        base: 'block w-full border disabled:cursor-not-allowed disabled:opacity-50',
        sizes: CONTROL_SIZES,
        colors: {
          gray: CONTROL_NEUTRAL,
          info: CONTROL_ACCENT,
          failure: CONTROL_DANGER,
          success: CONTROL_SUCCESS,
          warning: CONTROL_WARNING
        },
        withIcon: { on: 'pl-7', off: '' },
        withRightIcon: { on: 'pr-7', off: '' },
        withAddon: { on: 'rounded-r-control', off: 'rounded-control' },
        withShadow: { on: 'shadow-raised', off: '' }
      }
    }
  },

  select: {
    addon:
      'inline-flex items-center rounded-l-control border border-r-0 border-strong bg-raised px-2 text-sm text-ink-muted',
    field: {
      icon: {
        base: 'pointer-events-none absolute inset-y-0 left-0 flex items-center pl-2',
        svg: 'h-3.5 w-3.5 text-ink-faint'
      },
      select: {
        base: 'block w-full border disabled:cursor-not-allowed disabled:opacity-50',
        sizes: CONTROL_SIZES,
        colors: {
          gray: CONTROL_NEUTRAL,
          info: CONTROL_ACCENT,
          failure: CONTROL_DANGER,
          success: CONTROL_SUCCESS,
          warning: CONTROL_WARNING
        },
        withIcon: { on: 'pl-7', off: '' },
        withAddon: { on: 'rounded-r-control', off: 'rounded-control' },
        withShadow: { on: 'shadow-raised', off: '' }
      }
    }
  },

  textarea: {
    // Textarea には sizes スロットがないので md 相当を base に直書きする。
    base: 'block w-full rounded-control border px-2 py-1 text-sm disabled:cursor-not-allowed disabled:opacity-50',
    colors: {
      gray: CONTROL_NEUTRAL,
      info: CONTROL_ACCENT,
      failure: CONTROL_DANGER,
      success: CONTROL_SUCCESS,
      warning: CONTROL_WARNING
    },
    withShadow: { on: 'shadow-raised', off: '' }
  },

  label: {
    root: {
      base: 'text-label',
      disabled: 'opacity-50',
      colors: {
        default: 'text-ink',
        info: 'text-accent',
        failure: 'text-danger',
        success: 'text-success',
        warning: 'text-warning'
      }
    }
  },

  helperText: {
    root: {
      base: 'mt-1 text-xs',
      colors: {
        gray: 'text-ink-faint',
        info: 'text-accent',
        failure: 'text-danger',
        success: 'text-success',
        warning: 'text-warning'
      }
    }
  },

  button: {
    // 既定の base は p-0.5 を持ち、size の padding に 2px 足していた。落とす。
    base: 'group relative flex items-stretch justify-center text-center font-medium transition-colors focus:z-10 focus:outline-none',
    size: {
      xs: 'px-1.5 py-0.5 text-micro',
      sm: 'px-2 py-0.5 text-xs',
      md: 'px-2 py-1 text-sm',
      lg: 'px-2.5 py-1.5 text-sm',
      xl: 'px-3 py-1.5 text-base'
    },
    color: {
      // info が Flowbite の既定色。アプリで使うのは info / blue / gray /
      // light / failure だが、残りも塗り分けずトークンに寄せておく。
      // そうしないと未使用の色を誰かが使った瞬間に素の cyan が出る。
      info: FILL_ACCENT,
      blue: FILL_ACCENT,
      cyan: FILL_ACCENT,
      indigo: FILL_ACCENT,
      purple: FILL_ACCENT,
      pink: FILL_ACCENT,
      teal: FILL_ACCENT,
      lime: FILL_ACCENT,
      dark: 'border border-transparent bg-sunken text-ink focus:ring-2 focus:ring-accent enabled:hover:bg-raised',
      light:
        'border border-strong bg-surface text-ink focus:ring-2 focus:ring-accent enabled:hover:bg-raised',
      gray: 'border border-subtle bg-transparent text-ink-muted focus:ring-2 focus:ring-accent enabled:hover:bg-raised enabled:hover:text-ink',
      failure: FILL_DANGER,
      red: FILL_DANGER,
      success: FILL_SUCCESS,
      green: FILL_SUCCESS,
      warning: FILL_WARNING,
      yellow: FILL_WARNING
    },
    disabled: 'cursor-not-allowed opacity-50',
    inner: {
      base: 'flex items-stretch transition-colors',
      // spinner の逃げ幅も padding に合わせて詰める。
      isProcessingPadding: { xs: 'pl-5', sm: 'pl-6', md: 'pl-7', lg: 'pl-8', xl: 'pl-9' }
    },
    spinnerLeftPosition: {
      xs: 'left-1.5',
      sm: 'left-2',
      md: 'left-2',
      lg: 'left-2.5',
      xl: 'left-3'
    },
    label:
      'ml-1.5 inline-flex h-4 w-4 items-center justify-center rounded-full bg-accent-tint text-micro text-accent',
    outline: {
      color: { gray: 'border border-strong', default: 'border-0', light: '' },
      off: '',
      on: 'flex w-full justify-center bg-canvas text-ink transition-colors group-enabled:group-hover:bg-opacity-0 group-enabled:group-hover:text-inherit',
      pill: { off: 'rounded-control', on: 'rounded-full' }
    },
    pill: {
      off: 'rounded-control',
      on: 'rounded-full'
    }
  },

  spinner: {
    // base の text-* が軌道、color の fill-* が動く弧。
    base: 'inline animate-spin text-raised',
    color: {
      info: 'fill-accent',
      gray: 'fill-ink-muted',
      failure: 'fill-danger',
      success: 'fill-success',
      warning: 'fill-warning',
      pink: 'fill-accent',
      purple: 'fill-accent'
    },
    // 既定は light=off に暗色専用の軌道色（dark バリアント）を持つ。
    // base の text-raised がすでにテーマ追従なので不要。
    light: { off: { base: '' }, on: { base: '' } },
    size: { xs: 'h-3 w-3', sm: 'h-3.5 w-3.5', md: 'h-4 w-4', lg: 'h-6 w-6', xl: 'h-8 w-8' }
  },

  toggleSwitch: {
    root: {
      base: 'group flex rounded-control focus:outline-none',
      active: { on: 'cursor-pointer', off: 'cursor-not-allowed opacity-50' },
      label: 'ms-2 mt-px text-start text-xs font-medium text-ink'
    },
    toggle: {
      // 枠線なし。以前は border-strong を付けていたが、明るい外観では塗りより
      // 暗く、暗い外観では明るくなるため、暗い側でだけ「縁取りされた錠剤」に
      // 見えていた。トラックは塗りだけで表現する。
      base: 'relative rounded-full after:absolute after:rounded-full after:shadow-raised after:transition-all group-focus:ring-2 group-focus:ring-accent',
      checked: {
        on: 'after:translate-x-full rtl:after:-translate-x-full',
        off: 'bg-sunken',
        // つまみは常に --knob（ほぼ白）。--surface は charcoal ではトラックより
        // 暗いので、つまみが消えてしまう。
        color: {
          blue: 'bg-accent',
          info: 'bg-accent',
          cyan: 'bg-accent',
          indigo: 'bg-accent',
          purple: 'bg-accent',
          pink: 'bg-accent',
          teal: 'bg-accent',
          lime: 'bg-accent',
          gray: 'bg-ink-muted',
          dark: 'bg-ink-muted',
          light: 'bg-sunken',
          success: 'bg-success',
          green: 'bg-success',
          failure: 'bg-danger',
          red: 'bg-danger',
          warning: 'bg-warning',
          yellow: 'bg-warning'
        }
      },
      // 幾何は translate-x-full が左右対称になるように決めている:
      // inset 2px、つまみ = 高さ - 4、トラック幅 = 高さ * 2 - 4。
      // 以前は inset 1px でつまみが幅の半分より狭く、オン側で 3px、オフ側で 1px
      // の隙間になっていた。
      sizes: {
        sm: 'h-3 w-5 min-w-5 after:left-0.5 after:top-0.5 after:h-2 after:w-2 after:bg-knob rtl:after:right-0.5',
        md: 'h-4 w-7 min-w-7 after:left-0.5 after:top-0.5 after:h-3 after:w-3 after:bg-knob rtl:after:right-0.5',
        lg: 'h-5 w-9 min-w-9 after:left-0.5 after:top-0.5 after:h-4 after:w-4 after:bg-knob rtl:after:right-0.5'
      }
    }
  },

  kbd: {
    root: {
      base: 'rounded-control border border-subtle bg-raised px-1 py-0 font-mono text-micro text-ink',
      icon: 'inline-block'
    }
  },

  table: {
    root: {
      base: 'w-full text-left text-sm text-ink-muted',
      shadow:
        'absolute left-0 top-0 -z-10 h-full w-full rounded-container bg-surface shadow-raised',
      wrapper: 'relative'
    },
    head: {
      base: 'group/head text-micro text-ink-muted',
      cell: {
        base: 'bg-surface-2 px-2 py-1 group-first/head:first:rounded-tl-container group-first/head:last:rounded-tr-container'
      }
    },
    body: {
      base: 'group/body',
      cell: {
        base: 'px-2 py-1 text-sm group-first/body:group-first/row:first:rounded-tl-container group-first/body:group-first/row:last:rounded-tr-container group-last/body:group-last/row:first:rounded-bl-container group-last/body:group-last/row:last:rounded-br-container'
      }
    },
    row: {
      base: 'group/row',
      hovered: 'hover:bg-raised',
      striped: 'odd:bg-surface even:bg-surface-2'
    }
  }
}
