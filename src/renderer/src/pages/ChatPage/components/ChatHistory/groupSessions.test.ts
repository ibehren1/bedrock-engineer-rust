import type { ChatFolder, SessionMetadata } from '@/types/chat/history'
import {
  draggedChatIds,
  groupSessionsByFolder,
  moveFolder,
  shiftFolder,
  CHAT_DRAG_TYPE
} from './groupSessions'

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

describe('moveFolder', () => {
  const ids = ['a', 'b', 'c', 'd']
  it('places the folder before or after the target', () => {
    expect(moveFolder(ids, 'd', 'b', 'before')).toEqual(['a', 'd', 'b', 'c'])
    expect(moveFolder(ids, 'a', 'c', 'after')).toEqual(['b', 'c', 'a', 'd'])
    expect(moveFolder(ids, 'a', 'd', 'after')).toEqual(['b', 'c', 'd', 'a'])
  })
  it('leaves the list alone for itself or unknown ids', () => {
    expect(moveFolder(ids, 'b', 'b', 'before')).toBe(ids)
    expect(moveFolder(ids, 'x', 'b', 'before')).toBe(ids)
    expect(moveFolder(ids, 'b', 'x', 'after')).toBe(ids)
  })
})

describe('shiftFolder', () => {
  const ids = ['a', 'b', 'c']
  it('moves one place and stops at the ends', () => {
    expect(shiftFolder(ids, 'b', -1)).toEqual(['b', 'a', 'c'])
    expect(shiftFolder(ids, 'b', 1)).toEqual(['a', 'c', 'b'])
    expect(shiftFolder(ids, 'a', -1)).toBe(ids)
    expect(shiftFolder(ids, 'c', 1)).toBe(ids)
    expect(shiftFolder(ids, 'x', 1)).toBe(ids)
  })
})
