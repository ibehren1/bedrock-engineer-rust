/**
 * Deep links into MCP Market (https://mcpmarket.com) plus a local, offline
 * mapping from an agent's configuration to the market categories worth browsing.
 *
 * MCP Market publishes no API and its robots.txt asks AI crawlers to stay out,
 * so the app never fetches the site: it only builds links the user clicks.
 */

export const MCP_MARKET_BASE_URL = 'https://mcpmarket.com'

/** Category slugs observed on the MCP Market home page, with display labels. */
export const MCP_MARKET_CATEGORIES = [
  { slug: 'developer-tools', label: 'Developer tools' },
  { slug: 'api-development', label: 'API development' },
  { slug: 'data-science-ml', label: 'Data science & ML' },
  { slug: 'database-management', label: 'Database management' },
  { slug: 'analytics-monitoring', label: 'Analytics & monitoring' },
  { slug: 'web-scraping-data-collection', label: 'Web scraping' },
  { slug: 'browser-automation', label: 'Browser automation' },
  { slug: 'cloud-infrastructure', label: 'Cloud infrastructure' },
  { slug: 'deployment-devops', label: 'Deployment & DevOps' },
  { slug: 'security-testing', label: 'Security & testing' },
  { slug: 'productivity-workflow', label: 'Productivity & workflow' },
  { slug: 'collaboration-tools', label: 'Collaboration tools' },
  { slug: 'content-management', label: 'Content management' },
  { slug: 'learning-documentation', label: 'Learning & documentation' },
  { slug: 'design-tools', label: 'Design tools' },
  { slug: 'marketing-automation', label: 'Marketing automation' },
  { slug: 'social-media-management', label: 'Social media' },
  { slug: 'e-commerce-solutions', label: 'E-commerce' },
  { slug: 'mobile-development', label: 'Mobile development' },
  { slug: 'game-development', label: 'Game development' }
] as const

export type McpMarketCategory = (typeof MCP_MARKET_CATEGORIES)[number]
export type McpMarketCategorySlug = McpMarketCategory['slug']

/** The agent facts used to pick categories and search terms. */
export type AgentMarketContext = {
  name?: string
  description?: string
  system?: string
  category?: string
  toolNames?: string[]
  /** Scenario titles/content: the most concrete statement of what the agent does */
  scenarios?: { title?: string; content?: string }[]
  /** Allowed command patterns — these often name the CLIs the agent works with */
  allowedCommands?: string[]
  /** Additional instruction appended to the generated system prompt */
  additionalInstruction?: string
}

export const marketUrls = {
  home: () => MCP_MARKET_BASE_URL,
  servers: () => `${MCP_MARKET_BASE_URL}/server`,
  categories: () => `${MCP_MARKET_BASE_URL}/categories`,
  category: (slug: string) => `${MCP_MARKET_BASE_URL}/categories/${slug}`,
  server: (slug: string) => `${MCP_MARKET_BASE_URL}/server/${slug}`,
  /**
   * The `/search` page exists, but its query-parameter name is not documented
   * and was not verified against the live site. `?q=` is the usual convention;
   * if the site ignores it the page still opens and the query can be retyped.
   */
  search: (query: string) => `${MCP_MARKET_BASE_URL}/search?q=${encodeURIComponent(query.trim())}`
}

/** Keywords that suggest a category, matched against the agent's own text. */
const CATEGORY_KEYWORDS: Record<McpMarketCategorySlug, string[]> = {
  'developer-tools': [
    'code',
    'coding',
    'developer',
    'programming',
    'refactor',
    'git',
    'repository'
  ],
  'api-development': ['api', 'rest', 'graphql', 'openapi', 'endpoint', 'webhook'],
  'data-science-ml': [
    'data science',
    'machine learning',
    'ml',
    'model training',
    'notebook',
    'pandas'
  ],
  'database-management': ['database', 'sql', 'postgres', 'mysql', 'sqlite', 'dynamodb', 'query'],
  'analytics-monitoring': [
    'analytics',
    'metrics',
    'monitoring',
    'observability',
    'dashboard',
    'logs'
  ],
  'web-scraping-data-collection': ['scrape', 'scraping', 'crawl', 'web search', 'collect data'],
  'browser-automation': ['browser', 'playwright', 'puppeteer', 'selenium', 'automate the browser'],
  'cloud-infrastructure': ['aws', 'cloud', 'terraform', 'cloudformation', 'infrastructure', 's3'],
  'deployment-devops': [
    'deploy',
    'deployment',
    'ci/cd',
    'pipeline',
    'docker',
    'kubernetes',
    'devops'
  ],
  'security-testing': ['security', 'vulnerability', 'pentest', 'audit', 'compliance', 'test suite'],
  'productivity-workflow': ['task', 'todo', 'calendar', 'email', 'note', 'schedule', 'workflow'],
  'collaboration-tools': ['slack', 'teams', 'jira', 'confluence', 'ticket', 'issue tracker'],
  'content-management': ['cms', 'blog', 'article', 'publish', 'documentation site', 'wordpress'],
  'learning-documentation': ['teach', 'mentor', 'tutorial', 'learning', 'documentation', 'explain'],
  'design-tools': ['design', 'figma', 'ux', 'ui', 'wireframe', 'prototype'],
  'marketing-automation': ['marketing', 'campaign', 'seo', 'newsletter', 'crm'],
  'social-media-management': ['social media', 'twitter', 'x.com', 'linkedin', 'instagram', 'post'],
  'e-commerce-solutions': [
    'ecommerce',
    'e-commerce',
    'shop',
    'store',
    'checkout',
    'stripe',
    'order'
  ],
  'mobile-development': ['ios', 'android', 'mobile app', 'react native', 'flutter', 'swift'],
  'game-development': ['game', 'unity', 'unreal', 'godot', 'sprite']
}

/** Tools whose presence hints at a category, independent of the prose. */
const TOOL_KEYWORDS: Record<string, McpMarketCategorySlug[]> = {
  tavilySearch: ['web-scraping-data-collection'],
  fetchWebsite: ['web-scraping-data-collection', 'api-development'],
  executeCommand: ['developer-tools', 'deployment-devops'],
  codeInterpreter: ['data-science-ml'],
  retrieve: ['learning-documentation'],
  invokeBedrockAgent: ['cloud-infrastructure'],
  invokeFlow: ['cloud-infrastructure'],
  generateImage: ['design-tools'],
  generateVideo: ['design-tools'],
  screenCapture: ['browser-automation'],
  cameraCapture: ['browser-automation'],
  writeToFile: ['developer-tools'],
  readFiles: ['developer-tools']
}

/**
 * Whole-word keyword match. Substring matching gave false positives — "postgres"
 * contains "post", which otherwise scored the social-media category.
 */
const matchesKeyword = (haystack: string, keyword: string): boolean => {
  const escaped = keyword.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return new RegExp(`(^|[^a-z0-9])${escaped}([^a-z0-9]|$)`, 'i').test(haystack)
}

const AGENT_CATEGORY_HINTS: Record<string, McpMarketCategorySlug[]> = {
  software: ['developer-tools', 'api-development'],
  coding: ['developer-tools'],
  design: ['design-tools'],
  data: ['data-science-ml', 'database-management'],
  business: ['productivity-workflow', 'collaboration-tools'],
  diagram: ['cloud-infrastructure'],
  website: ['developer-tools', 'design-tools'],
  custom: []
}

/**
 * Rank MCP Market categories against an agent's configuration.
 * Runs entirely locally — no model call, no network.
 *
 * @param limit maximum number of categories to return
 */
export const suggestMarketCategories = (
  context: AgentMarketContext,
  limit = 4
): McpMarketCategory[] => {
  const haystack = [context.name, context.description, context.system]
    .filter(Boolean)
    .join('\n')
    .toLowerCase()

  const scores = new Map<McpMarketCategorySlug, number>()
  const bump = (slug: McpMarketCategorySlug, by: number) =>
    scores.set(slug, (scores.get(slug) || 0) + by)

  // Prose matches: each distinct keyword hit counts once.
  for (const [slug, keywords] of Object.entries(CATEGORY_KEYWORDS)) {
    const hits = keywords.filter((keyword) => matchesKeyword(haystack, keyword)).length
    if (hits > 0) bump(slug as McpMarketCategorySlug, hits * 2)
  }

  // Enabled tools are a stronger signal than prose, but there are fewer of them.
  for (const toolName of context.toolNames || []) {
    for (const slug of TOOL_KEYWORDS[toolName] || []) {
      bump(slug, 3)
    }
  }

  // The agent's own category, when it maps to something.
  const categoryKey = (context.category || '').toLowerCase()
  for (const [key, slugs] of Object.entries(AGENT_CATEGORY_HINTS)) {
    if (categoryKey.includes(key)) {
      slugs.forEach((slug) => bump(slug, 2))
    }
  }

  const ranked = [...scores.entries()]
    .filter(([, score]) => score > 0)
    .sort((a, b) => {
      if (b[1] !== a[1]) return b[1] - a[1]
      // Stable tie-break: the order categories are declared in.
      return categoryIndex(a[0]) - categoryIndex(b[0])
    })
    .map(([slug]) => MCP_MARKET_CATEGORIES.find((entry) => entry.slug === slug)!)

  // Nothing matched (a brand new agent with no description yet): fall back to
  // the categories most agents start from rather than showing nothing.
  if (ranked.length === 0) {
    return MCP_MARKET_CATEGORIES.filter((entry) =>
      ['developer-tools', 'productivity-workflow'].includes(entry.slug)
    ).slice(0, limit)
  }

  return ranked.slice(0, limit)
}

const categoryIndex = (slug: McpMarketCategorySlug) =>
  MCP_MARKET_CATEGORIES.findIndex((entry) => entry.slug === slug)

/**
 * Search terms for the agent, used to build a market search link.
 * Prefers the agent's name plus its top category label, so the query reads like
 * something a person would type.
 */
export const buildMarketSearchTerms = (context: AgentMarketContext): string => {
  const [topCategory] = suggestMarketCategories(context, 1)
  const name = (context.name || '').trim()
  if (name && topCategory) return `${name} ${topCategory.label}`
  if (topCategory) return topCategory.label
  return name || 'mcp server'
}
