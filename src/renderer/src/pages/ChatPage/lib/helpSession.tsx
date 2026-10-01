import React, { createContext, useContext } from 'react'

/**
 * Marks the chat currently on screen as the Help chat.
 *
 * The Help chat overrides the agent, the model and the system prompt for its session only, and
 * those overrides live in ChatPage's state rather than in settings — persisting them would leave
 * the user's ordinary chats stuck on the Help agent. The input bar sits several components below
 * ChatPage and needs to know, so it is passed by context instead of threaded through
 * InputFormContainer and InputForm as a prop each layer would only forward.
 */
export type HelpSession = {
  isHelpSession: boolean
  /** Agent name to show in place of the agent picker. */
  agentName?: string
  /** Model the Help chat actually runs on (the Light Processing Model). */
  modelId?: string
}

const HelpSessionContext = createContext<HelpSession>({ isHelpSession: false })

export const HelpSessionProvider: React.FC<{
  value: HelpSession
  children: React.ReactNode
}> = ({ value, children }) => (
  <HelpSessionContext.Provider value={value}>{children}</HelpSessionContext.Provider>
)

export const useHelpSession = (): HelpSession => useContext(HelpSessionContext)
