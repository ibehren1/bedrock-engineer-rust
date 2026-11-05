import type { ChatFolder, SessionMetadata } from '@/types/chat/history'
import { draggedChatIds, groupSessionsByFolder, CHAT_DRAG_TYPE } from './groupSessions'

const session = (id: string, folderId?: string): SessionMetadata => ({
  id,
  title: id,
  createdAt: 0,
  updatedAt: 0,
  messageCount: 1,
  agentId: 'a',
  modelId: 'm',
  folderId
})

const folders: ChatFolder[] = [
  { id: 'f1', name: 'Work', createdAt: 1 },
  { id: 'f2', name: 'Empty', createdAt: 2 }
]

describe('groupSessionsByFolder', () => {
  it('splits chats by folder and keeps the list order', () => {
    const { byFolder, loose } = groupSessionsByFolder(
      [session('a', 'f1'), session('b'), session('c', 'f1'), session('d', 'gone')],
      folders
    )
    expect(byFolder.get('f1')?.map((s) => s.id)).toEqual(['a', 'c'])
    expect(byFolder.get('f2')).toEqual([])
    // A chat whose folder was deleted is loose.
    expect(loose.map((s) => s.id)).toEqual(['b', 'd'])
  })
})

describe('draggedChatIds', () => {
  const transfer = (data: string) =>
    ({ getData: (type: string) => (type === CHAT_DRAG_TYPE ? data : '') }) as DataTransfer

  it('reads the ids a history row put on the drag', () => {
    expect(draggedChatIds(transfer('["a","b"]'))).toEqual(['a', 'b'])
  })

  it('gives nothing for other drags', () => {
    expect(draggedChatIds(transfer(''))).toEqual([])
    expect(draggedChatIds(transfer('not json'))).toEqual([])
    expect(draggedChatIds(transfer('{"a":1}'))).toEqual([])
    expect(draggedChatIds(null)).toEqual([])
  })
})
