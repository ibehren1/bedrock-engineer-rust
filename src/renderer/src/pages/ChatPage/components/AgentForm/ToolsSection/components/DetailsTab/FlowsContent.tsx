import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FlowConfig, InputType } from '@/types/agent-chat'
import JSONEditor from '@renderer/components/JSONViewer/JSONEditor'
import JSONViewer from '@renderer/components/JSONViewer'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { Button, Input, Label, Select, Textarea } from '@renderer/components/ui'

// オブジェクト型のサンプルスキーマ
const OBJECT_SAMPLES = {
  simple: {
    type: 'object',
    properties: {
      name: {
        type: 'string',
        description: 'User Name'
      },
      age: {
        type: 'number',
        description: 'Age'
      },
      active: {
        type: 'boolean',
        description: 'State of Active'
      }
    },
    required: ['name']
  },
  nested: {
    type: 'object',
    properties: {
      user: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          contact: {
            type: 'object',
            properties: {
              email: { type: 'string' },
              phone: { type: 'string' }
            }
          }
        }
      },
      settings: {
        type: 'object',
        properties: {
          notifications: { type: 'boolean' },
          theme: { type: 'string' }
        }
      }
    }
  },
  complex: {
    type: 'object',
    properties: {
      id: {
        type: 'string',
        format: 'uuid',
        description: 'uuid'
      },
      profile: {
        type: 'object',
        properties: {
          firstName: { type: 'string' },
          lastName: { type: 'string' },
          birthDate: { type: 'string', format: 'date' },
          address: {
            type: 'object',
            properties: {
              street: { type: 'string' },
              city: { type: 'string' },
              zipCode: { type: 'string' },
              country: { type: 'string' }
            }
          }
        },
        required: ['firstName', 'lastName']
      },
      preferences: {
        type: 'object',
        properties: {
          theme: {
            type: 'string',
            enum: ['light', 'dark', 'system']
          },
          notifications: { type: 'boolean' },
          language: { type: 'string' }
        }
      },
      tags: {
        type: 'array',
        items: { type: 'string' }
      }
    },
    required: ['id', 'profile']
  }
}

// 配列型のサンプルスキーマ
const ARRAY_SAMPLES = {
  simple: {
    type: 'array',
    items: {
      type: 'string'
    }
  },
  objects: {
    type: 'array',
    items: {
      type: 'object',
      properties: {
        id: { type: 'number' },
        name: { type: 'string' },
        tags: {
          type: 'array',
          items: { type: 'string' }
        }
      },
      required: ['id', 'name']
    }
  },
  complex: {
    type: 'array',
    items: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        timestamp: { type: 'string', format: 'date-time' },
        data: {
          type: 'object',
          properties: {
            title: { type: 'string' },
            description: { type: 'string' },
            priority: {
              type: 'string',
              enum: ['low', 'medium', 'high', 'critical']
            },
            completed: { type: 'boolean' },
            assignee: {
              type: 'object',
              properties: {
                id: { type: 'string' },
                name: { type: 'string' },
                email: { type: 'string' }
              }
            },
            tags: {
              type: 'array',
              items: { type: 'string' }
            }
          },
          required: ['title', 'priority']
        }
      },
      required: ['id', 'timestamp', 'data']
    }
  }
}

export interface FlowsContentProps {
  flows: FlowConfig[]
  onChange: (flows: FlowConfig[]) => void
}

export const FlowsContent: React.FC<FlowsContentProps> = ({ flows, onChange }) => {
  const { t } = useTranslation()
  const [flowIdentifier, setFlowIdentifier] = useState('')
  const [flowAliasIdentifier, setFlowAliasIdentifier] = useState('')
  const [description, setDescription] = useState('')
  const [inputType, setInputType] = useState<InputType>('string')
  const [schema, setSchema] = useState<object>({})
  const [schemaError, setSchemaError] = useState('')
  const [editingIndex, setEditingIndex] = useState<number | null>(null)

  // スキーマが有効なJSONか検証する関数
  const validateSchema = (schema: any): boolean => {
    try {
      // 簡単な検証: JSON Schemaのルール準拠チェック
      if (schema && typeof schema === 'object') {
        return true
      }
      return false
    } catch (e) {
      setSchemaError(e instanceof Error ? e.message : 'Invalid schema')
      return false
    }
  }

  // Flow の編集を開始する関数
  const startEditing = (index: number) => {
    const flow = flows[index]
    setFlowIdentifier(flow.flowIdentifier)
    setFlowAliasIdentifier(flow.flowAliasIdentifier)
    setDescription(flow.description || '')
    setInputType(flow.inputType || 'string')
    setSchema(flow.schema || {})
    setEditingIndex(index)
    // フォームまでスクロール
    document.querySelector('.flex-col.gap-2.p-2\\.5.border')?.scrollIntoView({ behavior: 'smooth' })
  }

  // 編集をキャンセルする関数
  const cancelEditing = () => {
    resetForm()
    setEditingIndex(null)
  }

  // Flow を追加または更新する関数
  const saveFlow = () => {
    if (!flowIdentifier || !flowAliasIdentifier) return

    // objectまたはarrayの場合はスキーマのバリデーションを行う
    if ((inputType === 'object' || inputType === 'array') && !validateSchema(schema)) {
      return
    }

    const flowData: FlowConfig = {
      flowIdentifier,
      flowAliasIdentifier,
      description,
      inputType,
      // objectまたはarrayの場合のみschemaを追加
      ...(inputType === 'object' || inputType === 'array' ? { schema } : {})
    }

    if (editingIndex !== null) {
      // 既存の Flow を更新
      const updatedFlows = [...flows]
      updatedFlows[editingIndex] = flowData
      onChange(updatedFlows)
      setEditingIndex(null)
    } else {
      // 新しい Flow を追加
      onChange([...flows, flowData])
    }

    // フォームをリセット
    resetForm()
  }

  // フォームリセット
  const resetForm = () => {
    setFlowIdentifier('')
    setFlowAliasIdentifier('')
    setDescription('')
    setInputType('string')
    setSchema({})
    setSchemaError('')
    setEditingIndex(null)
  }

  // Flow を削除する関数
  const removeFlow = (index: number) => {
    // 編集中の Flow を削除した場合は編集モードを終了
    if (editingIndex === index) {
      resetForm()
    }
    const updatedFlows = [...flows]
    updatedFlows.splice(index, 1)
    onChange(updatedFlows)
  }

  return (
    <div>
      {/* 新しい Flow を登録するフォーム */}
      <div className="flex flex-col gap-2 p-2.5 border border-subtle rounded-control">
        <h4 className="font-medium text-sm mb-2 text-ink">
          {editingIndex !== null ? t('Edit Bedrock Flow') : t('Add New Bedrock Flow')}
        </h4>

        <div className="flex-grow">
          <Label>{t('Flow Identifier')}</Label>
          <Input
            type="text"
            value={flowIdentifier}
            onChange={(e) => setFlowIdentifier(e.target.value)}
            placeholder="e.g., FLOW123456"
          />
        </div>

        <div className="flex-grow">
          <Label>{t('Flow Alias Identifier')}</Label>
          <Input
            type="text"
            value={flowAliasIdentifier}
            onChange={(e) => setFlowAliasIdentifier(e.target.value)}
            placeholder="e.g., ALIAS123456"
          />
        </div>

        <div className="flex-grow">
          <Label>{t('Description')}</Label>
          <Textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="e.g., Data processing workflow"
            rows={3}
          />
        </div>

        {/* 新規: 入力型の選択 */}
        <div className="flex-grow">
          <Label>{t('Input Type')}</Label>
          <Select value={inputType} onChange={(e) => setInputType(e.target.value as InputType)}>
            <option value="string">{t('String')}</option>
            <option value="number">{t('Number')}</option>
            <option value="boolean">{t('Boolean')}</option>
            <option value="object">{t('Object')}</option>
            <option value="array">{t('Array')}</option>
          </Select>
        </div>

        {/* 新規: オブジェクトまたは配列の場合はJSONスキーマエディタを表示 */}
        {(inputType === 'object' || inputType === 'array') && (
          <div className="flex-grow">
            <Label>{t('JSON Schema')}</Label>

            <div className="border border-subtle rounded-control p-2.5 bg-surface-2">
              <div className="mb-3">
                <p className="text-xs text-ink-muted mb-2">
                  {inputType === 'object'
                    ? t('Define the structure of the object that will be sent to the Flow.')
                    : t('Define the structure of the array that will be sent to the Flow.')}
                </p>
              </div>

              {/* サンプルリンク部分を改善 */}
              <div className="mb-3">
                <p className="text-xs font-medium text-ink mb-1">
                  {t('flow.sample.title', 'サンプルテンプレート')}:
                </p>
                <div className="flex flex-wrap gap-2 mb-2">
                  {inputType === 'object' ? (
                    <>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(OBJECT_SAMPLES.simple)}
                        title={t(
                          'flow.sample.object.simple.tooltip',
                          'シンプルなオブジェクト（名前、年齢、状態）'
                        )}
                      >
                        {t('flow.sample.object.simple', 'シンプル')}
                      </button>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(OBJECT_SAMPLES.nested)}
                        title={t(
                          'flow.sample.object.nested.tooltip',
                          'ネストしたオブジェクト（ユーザー情報と設定）'
                        )}
                      >
                        {t('flow.sample.object.nested', 'ネスト')}
                      </button>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(OBJECT_SAMPLES.complex)}
                        title={t(
                          'flow.sample.object.complex.tooltip',
                          '複雑なオブジェクト（プロフィール、設定、タグなど）'
                        )}
                      >
                        {t('flow.sample.object.complex', '複雑')}
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(ARRAY_SAMPLES.simple)}
                        title={t('flow.sample.array.simple.tooltip', '文字列の配列')}
                      >
                        {t('flow.sample.array.simple', 'シンプル')}
                      </button>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(ARRAY_SAMPLES.objects)}
                        title={t(
                          'flow.sample.array.objects.tooltip',
                          'オブジェクトの配列（ID、名前、タグ）'
                        )}
                      >
                        {t('flow.sample.array.objects', 'オブジェクト')}
                      </button>
                      <button
                        className="text-xs px-2 py-1 bg-accent-tint text-accent rounded-control hover:bg-accent-tint-strong"
                        onClick={() => setSchema(ARRAY_SAMPLES.complex)}
                        title={t(
                          'flow.sample.array.complex.tooltip',
                          '複雑なオブジェクトの配列（タスクデータなど）'
                        )}
                      >
                        {t('flow.sample.array.complex', '複雑')}
                      </button>
                    </>
                  )}
                </div>
              </div>

              <div className="mb-1">
                <p className="text-xs font-medium text-ink mb-1">
                  {t('flow.editor.title', 'スキーマエディタ')}:
                </p>
              </div>

              <JSONEditor
                value={schema}
                onChange={setSchema}
                height="200px"
                error={schemaError}
                defaultValue={
                  inputType === 'object'
                    ? {
                        type: 'object',
                        properties: {
                          // デフォルトのオブジェクトスキーマ
                        },
                        required: []
                      }
                    : {
                        type: 'array',
                        items: {
                          // デフォルトの配列要素スキーマ
                        }
                      }
                }
              />
              {schemaError && <p className="text-xs text-danger mt-1">{schemaError}</p>}

              <div className="mt-3 bg-accent-tint p-2 rounded-control text-xs text-ink-muted">
                <p className="font-medium text-accent mb-1">{t('flow.hint.title', 'ヒント')}:</p>
                <p>
                  {t(
                    'flow.hint.description',
                    'JSON Schemaを使用して、Flowに送信するデータの構造を定義します。これにより、AIがFlowに正しい形式のデータを送信できるようになります。'
                  )}
                </p>
              </div>
            </div>
          </div>
        )}

        <div className="flex gap-2">
          <Button
            onClick={saveFlow}
            disabled={!flowIdentifier || !flowAliasIdentifier}
            variant="primary"
          >
            {editingIndex !== null ? t('Update Flow') : t('Add Flow')}
          </Button>
          {editingIndex !== null && (
            <button
              onClick={cancelEditing}
              className="px-2.5 py-1 text-sm text-ink bg-raised rounded-control hover:bg-sunken"
            >
              {t('Cancel')}
            </button>
          )}
        </div>
      </div>

      {/* 登録済みの Flow 一覧 */}
      <div className="space-y-3 mt-3">
        <h4 className="font-medium text-sm text-ink">{t('Registered Bedrock Flows')}</h4>

        {flows.length === 0 ? (
          <p className="text-sm text-ink-muted italic">{t('No Bedrock Flows registered yet')}</p>
        ) : (
          flows.map((flow, index) => (
            <div
              key={index}
              className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
            >
              <div className="flex items-center justify-between">
                <span className="font-mono">
                  {t('Flow Identifier')}: {flow.flowIdentifier}
                </span>
                <div className="flex gap-2">
                  <button
                    onClick={() => startEditing(index)}
                    className="text-accent hover:text-accent p-1"
                    title="Edit"
                    aria-label="Edit flow"
                  >
                    <EditIcon />
                  </button>
                  <button
                    onClick={() => removeFlow(index)}
                    className="text-danger hover:text-danger-strong p-1"
                    title="Remove"
                    aria-label="Remove flow"
                  >
                    <RemoveIcon />
                  </button>
                </div>
              </div>
              <p className="text-xs text-ink-muted mt-1 whitespace-pre-line">{flow.description}</p>
              {/* 情報を横並びにする新しいレイアウト */}
              <div className="flex gap-4 mt-1">
                <div className="w-[15rem]">
                  <div>
                    <span className="text-xs text-ink-muted">
                      {t('Flow Alias Identifier')}:{' '}
                      <span className="font-mono">{flow.flowAliasIdentifier}</span>
                    </span>
                  </div>

                  {flow.inputType && (
                    <div>
                      <span className="text-xs text-ink-muted">
                        {t('Input Type')}: <span className="font-mono">{flow.inputType}</span>
                      </span>
                    </div>
                  )}
                </div>

                {flow.schema && (
                  <div className="flex flex-col w-full">
                    <JSONViewer
                      data={flow.schema}
                      title={t('Schema')}
                      maxHeight="400px"
                      showCopyButton={true}
                    />
                  </div>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}
