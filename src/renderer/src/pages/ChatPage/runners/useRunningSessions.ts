import { useSyncExternalStore } from 'react'
import { getRunningSessionIds, subscribeToRunningSessions } from './sessionRunners'

/**
 * Ids of the sessions with an agent turn in flight. Now that a turn survives switching
 * chats, the history list needs this to show which ones are still working.
 */
export function useRunningSessions(): readonly string[] {
  return useSyncExternalStore(subscribeToRunningSessions, getRunningSessionIds)
}
