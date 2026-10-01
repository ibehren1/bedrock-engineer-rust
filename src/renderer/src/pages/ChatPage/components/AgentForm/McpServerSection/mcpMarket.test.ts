import {
  buildMarketSearchTerms,
  marketUrls,
  suggestMarketCategories,
  MCP_MARKET_BASE_URL
} from './mcpMarket'

describe('marketUrls', () => {
  it('builds category and server links', () => {
    expect(marketUrls.category('database-management')).toBe(
      `${MCP_MARKET_BASE_URL}/categories/database-management`
    )
    expect(marketUrls.server('firecrawl')).toBe(`${MCP_MARKET_BASE_URL}/server/firecrawl`)
  })

  it('encodes and trims search queries', () => {
    expect(marketUrls.search('  postgres database  ')).toBe(
      `${MCP_MARKET_BASE_URL}/search?q=postgres%20database`
    )
  })
})

describe('suggestMarketCategories', () => {
  it('picks database and data categories for a data agent', () => {
    const slugs = suggestMarketCategories({
      name: 'Data Analyst',
      description: 'Runs SQL against Postgres and builds dashboards',
      system: 'You query the database and summarise metrics for the team.'
    }).map((category) => category.slug)

    expect(slugs).toContain('database-management')
    expect(slugs).toContain('analytics-monitoring')
  })

  it('weights enabled tools, not just prose', () => {
    const slugs = suggestMarketCategories({
      name: 'Helper',
      description: 'General assistant',
      toolNames: ['tavilySearch', 'codeInterpreter']
    }).map((category) => category.slug)

    expect(slugs).toContain('web-scraping-data-collection')
    expect(slugs).toContain('data-science-ml')
  })

  it('uses the agent category as a hint', () => {
    const slugs = suggestMarketCategories({ name: 'Nameless', category: 'design' }).map(
      (category) => category.slug
    )

    expect(slugs).toContain('design-tools')
  })

  it('honours the limit and returns at most that many categories', () => {
    const categories = suggestMarketCategories(
      {
        name: 'Everything agent',
        description: 'code api database analytics scraping browser aws deploy security',
        toolNames: ['executeCommand', 'tavilySearch']
      },
      3
    )

    expect(categories).toHaveLength(3)
  })

  it('matches whole words only, so "postgres" is not a social-media "post"', () => {
    const slugs = suggestMarketCategories({
      name: 'Release Manager',
      description: 'Runs the deploy pipeline and reports on postgres migrations'
    }).map((category) => category.slug)

    expect(slugs).not.toContain('social-media-management')
    expect(slugs).toContain('deployment-devops')
    expect(slugs).toContain('database-management')
  })

  it('falls back to starter categories when nothing matches', () => {
    const slugs = suggestMarketCategories({ name: '', description: '' }).map(
      (category) => category.slug
    )

    expect(slugs).toEqual(['developer-tools', 'productivity-workflow'])
  })
})

describe('buildMarketSearchTerms', () => {
  it('combines the agent name with its top category', () => {
    expect(
      buildMarketSearchTerms({
        name: 'Release Manager',
        description: 'Runs the deploy pipeline and watches kubernetes'
      })
    ).toBe('Release Manager Deployment & DevOps')
  })

  it('falls back to the name alone when there is nothing else to go on', () => {
    expect(buildMarketSearchTerms({ name: 'Scratch agent' })).toBe('Scratch agent Developer tools')
    expect(buildMarketSearchTerms({})).toBe('Developer tools')
  })
})
