import React from 'react'
import { Accordion } from 'flowbite-react'
import { FaShieldAlt } from 'react-icons/fa'
import CodeRenderer from '../../Code/CodeRenderer'

// GuardContent の型定義
export interface GuardContentProps {
  content?: {
    text?: {
      text?: string
    }
    // 将来的な拡張のために他のフィールドも追加可能
    [key: string]: any
  }
}

export const GuardContent: React.FC<GuardContentProps> = ({ content }) => {
  // コンテンツがない場合はnullを返す
  if (!content) return null

  // テキストコンテンツを取得
  const textContent = content.text?.text || ''

  return (
    <div className="flex flex-col gap-2 w-full">
      <div className="flex items-center gap-2">
        <FaShieldAlt className="text-accent" />
        <span className="text-sm font-medium text-accent">Guardrail Protected Content</span>
      </div>

      <div className="text-content">
        <CodeRenderer text={textContent} />
      </div>
    </div>
  )
}

// Accordionを使った折りたたみ式の表示タイプ
export const GuardContentCollapsible: React.FC<GuardContentProps> = ({ content }) => {
  // コンテンツがない場合はnullを返す
  if (!content) return null

  // テキストコンテンツを取得
  const textContent = content.text?.text || ''

  return (
    <Accordion className="w-full" collapseAll>
      <Accordion.Panel>
        <Accordion.Title>
          <div className="flex gap-2 items-center">
            <FaShieldAlt className="text-accent" />
            <span className="text-sm font-medium">Guard Protected Content</span>
          </div>
        </Accordion.Title>
        <Accordion.Content>
          <div className="text-content">
            <CodeRenderer text={textContent} />
          </div>
        </Accordion.Content>
      </Accordion.Panel>
    </Accordion>
  )
}

export default GuardContent
