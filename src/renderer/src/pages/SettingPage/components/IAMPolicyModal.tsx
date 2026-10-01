import React from 'react'
import { useTranslation } from 'react-i18next'

interface IAMPolicyModalProps {
  isOpen: boolean
  onClose: () => void
}

export const IAMPolicyModal: React.FC<IAMPolicyModalProps> = ({ isOpen, onClose }) => {
  const { t } = useTranslation()

  if (!isOpen) return null

  const policies = [
    {
      name: 'Recommended Policy (Complete)',
      description: t(
        'Complete permissions for all Bedrock Engineer features including translation and video generation'
      ),
      example: `{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "bedrock:InvokeModel",
        "bedrock:InvokeModelWithResponseStream",
        "bedrock:GetFoundationModel",
        "bedrock:ListFoundationModels",
        "bedrock:ApplyGuardrail",
        "bedrock:InvokeAgent",
        "bedrock:Retrieve",
        "bedrock:RetrieveAndGenerate",
        "bedrock:InvokeFlow",
        "bedrock:ListPromptRouters",
        "bedrock:GetAsyncInvoke",
        "bedrock:ListInferenceProfiles",
        "bedrock:GetInferenceProfile"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "translate:TranslateText"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:ListBucket"
      ],
      "Resource": [
        "arn:aws:s3:::*/*",
        "arn:aws:s3:::*"
      ]
    },
    {
      "Effect": "Allow",
      "Action": [
        "sts:GetCallerIdentity"
      ],
      "Resource": "*"
    }
  ]
}
`
    },
    {
      name: 'Basic Policy (LLM Only)',
      description: t('Minimal permissions for basic LLM interactions only'),
      example: `{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "bedrock:InvokeModel",
        "bedrock:InvokeModelWithResponseStream",
        "bedrock:GetFoundationModel",
        "bedrock:ListFoundationModels"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "sts:GetCallerIdentity"
      ],
      "Resource": "*"
    }
  ]
}
`
    }
  ]

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto">
      <div className="flex items-center justify-center min-h-screen pt-4 px-4 pb-20 text-center sm:block sm:p-0">
        <div className="fixed inset-0 transition-opacity" aria-hidden="true">
          <div className="absolute inset-0 bg-black opacity-50" />
        </div>

        <div className="inline-block align-bottom bg-surface rounded-container text-left overflow-hidden shadow-xl transform transition-all sm:my-8 sm:align-middle sm:max-w-4xl sm:w-full">
          <div className="px-4 pt-5 pb-4 sm:p-3 sm:pb-4">
            <div className="sm:flex sm:items-start">
              <div className="mt-3 text-center sm:mt-0 sm:text-left w-full">
                <div className="flex justify-between">
                  <h3 className="text-heading leading-6 font-medium text-ink">
                    {t('Required IAM Policies for Amazon Bedrock')}
                  </h3>
                  <button
                    type="button"
                    onClick={onClose}
                    className="w-full inline-flex justify-center rounded-control border border-transparent
 shadow-sm px-2.5 py-1 bg-accent text-base font-medium text-accent-fg
                hover:bg-accent-strong focus:outline-none focus:ring-2
                focus:ring-offset-2 focus:ring-accent sm:ml-3 sm:w-auto sm:text-sm"
                  >
                    {t('Close')}
                  </button>
                </div>
                <div className="bg-accent-tint border-l-4 border-accent p-2.5 mt-4">
                  <div className="flex">
                    <div className="ml-3">
                      <p className="text-sm text-accent">
                        {t('For more information about IAM policies for Amazon Bedrock, visit the')}{' '}
                        <a
                          href="https://docs.aws.amazon.com/bedrock/latest/userguide/security-iam.html"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="font-medium underline hover:text-accent"
                        >
                          {t('Amazon Bedrock documentation')}
                        </a>
                      </p>
                    </div>
                  </div>
                </div>
                <div className="mt-4 space-y-2">
                  {policies.map((policy, index) => (
                    <div
                      key={index}
                      className="bg-surface-2 p-2.5 rounded-container border border-subtle"
                    >
                      <h4 className="text-base font-medium text-ink mb-2">{policy.name}</h4>
                      <p className="text-sm text-ink-muted mb-2">{policy.description}</p>

                      <div className="relative">
                        <pre className="bg-raised p-3 rounded-control text-sm overflow-x-auto">
                          <code className="text-ink">{policy.example}</code>
                        </pre>
                        <button
                          onClick={() => navigator.clipboard.writeText(policy.example)}
                          className="absolute top-2 right-2 px-2 py-1 text-xs text-accent
                            hover:text-accent bg-surface
                            rounded-control border border-subtle"
                        >
                          {t('Copy')}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
