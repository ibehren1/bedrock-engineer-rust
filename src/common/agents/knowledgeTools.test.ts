import { readFileSync } from 'fs'
import { join } from 'path'
import {
  KNOWLEDGE_BASE_TOOL_DEFINITIONS,
  knowledgeBaseTools,
  localKnowledgeBaseServer
} from './knowledgeTools'
import { buildKnowledgeBasePrompt, withKnowledgeBasePrompt } from './knowledgePrompt'

describe('knowledgeBaseTools', () => {
  it('matches the built-in MCP server definitions', () => {
    const rust = JSON.parse(
      readFileSync(join(__dirname, '../../../src-tauri/crates/knowledge/data/tools.json'), 'utf8')
    )
    expect(KNOWLEDGE_BASE_TOOL_DEFINITIONS).toEqual(rust)
  })

  it('is empty without local knowledge bases', () => {
    expect(knowledgeBaseTools({})).toEqual([])
    expect(knowledgeBaseTools({ localKnowledgeBases: [] })).toEqual([])
  })

  it('returns Bedrock tool specs', () => {
    const tools = knowledgeBaseTools({ localKnowledgeBases: [{ id: 'k', name: 'Docs' }] })
    expect(tools.map((t) => t.toolSpec?.name)).toEqual([
      'searchKnowledgeBase',
      'listKnowledgeBases',
      'listKnowledgeBaseSources'
    ])
    expect(tools[0].toolSpec?.inputSchema?.json).toMatchObject({ required: ['query'] })
  })
})

describe('localKnowledgeBaseServer', () => {
  it('matches the Rust built-in server entry', () => {
    // `local_knowledge_base_server` in src-tauri/crates/common/src/agent.rs.
    expect(localKnowledgeBaseServer('a1')).toEqual({
      name: 'local-knowledgebase',
      description: "Searches this agent's local knowledge bases",
      connectionType: 'builtin',
      agentId: 'a1'
    })
  })
})

describe('buildKnowledgeBasePrompt', () => {
  it('matches the Rust prompt text', () => {
    // Same expectation as `knowledge_base_block` in src-tauri/crates/agents/src/prompt.rs.
    expect(
      buildKnowledgeBasePrompt({
        localKnowledgeBases: [
          { id: 'a', name: 'Docs', description: 'Product docs' },
          { id: 'b', name: 'Notes' }
        ]
      })
    ).toBe(
      '<local_knowledge_bases>\nYou have local knowledge bases with documents and websites the user added:\n- Docs: Product docs\n- Notes\nSearch them with the searchKnowledgeBase tool before answering questions they may cover, and cite the sources (file path or URL) of the passages you use.\n</local_knowledge_bases>'
    )
  })

  it('appends only when there are knowledge bases', () => {
    expect(withKnowledgeBasePrompt('base', {})).toBe('base')
    const out = withKnowledgeBasePrompt('base', { localKnowledgeBases: [{ id: 'a', name: 'X' }] })
    expect(out.startsWith('base\n\n<local_knowledge_bases>')).toBe(true)
    expect(withKnowledgeBasePrompt('', { localKnowledgeBases: [{ id: 'a', name: 'X' }] })).toMatch(
      /^<local_knowledge_bases>/
    )
  })
})
