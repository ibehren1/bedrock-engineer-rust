import type { StepType } from '@reactour/tour'
import type { TFunction } from 'i18next'

/**
 * The app tour (Home → "Open Tour"): one stop per sidebar item, pointing at the item's icon.
 * Settings comes first because nothing works until AWS credentials are set; the rest follow the
 * sidebar's order, and Help closes with the command palette shortcut.
 */
export const TOUR_STOPS: readonly { href: string; content: string }[] = [
  { href: '/setting', content: 'set your aws credential' },
  { href: '/chat', content: 'tour.chat' },
  { href: '/my-agents', content: 'tour.myAgents' },
  { href: '/memory', content: 'tour.memory' },
  { href: '/agent-directory', content: 'tour.agentDirectory' },
  { href: '/background-agent', content: 'tour.backgroundAgent' },
  { href: '/generative-ui', content: 'tour.websiteGenerator' },
  { href: '/step-functions-generator', content: 'tour.stepFunctionsGenerator' },
  { href: '/diagram-generator', content: 'tour.diagramGenerator' },
  { href: '/accounting', content: 'tour.accounting' },
  { href: '/knowledge-bases', content: 'tour.knowledgeBases' },
  { href: '/help', content: 'tour.help' }
]

/** The sidebar link a stop points at (Layout's ListItem sets `data-tour` to its href). */
export const tourSelector = (href: string): string => `[data-tour="${href}"]`

/** Tour steps for the current sidebar: items hidden in Settings → Sidebar are skipped. */
export function buildTourSteps(
  t: TFunction,
  hiddenItems: readonly string[],
  paletteShortcut: string
): StepType[] {
  return TOUR_STOPS.filter(({ href }) => !hiddenItems.includes(href)).map(({ href, content }) => ({
    selector: tourSelector(href),
    content: t(content, { shortcut: paletteShortcut }),
    position: 'right'
  }))
}
