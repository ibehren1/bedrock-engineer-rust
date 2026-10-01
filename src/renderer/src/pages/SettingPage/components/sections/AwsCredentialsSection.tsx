import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FcKey } from 'react-icons/fc'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'
import { SettingSelect } from '../SettingSelect'
import { IAMPolicyModal } from '../IAMPolicyModal'
import { buildRegionGroups } from '../../utils/regionOptions'

export const AwsCredentialsSection: React.FC = () => {
  const { t } = useTranslation()
  const {
    awsRegion,
    setAwsRegion,
    awsAccessKeyId,
    setAwsAccessKeyId,
    awsSecretAccessKey,
    setAwsSecretAccessKey,
    awsSessionToken,
    setAwsSessionToken,
    useAwsProfile,
    setUseAwsProfile,
    awsProfile,
    setAwsProfile
  } = useSettings()
  const [isPolicyModalOpen, setIsPolicyModalOpen] = useState(false)

  const regionGroups = buildRegionGroups(t)

  const regionSelect = (
    <SettingSelect
      label={t('AWS Region')}
      value={awsRegion}
      options={[{ value: '', label: t('Select a region') }]}
      groups={regionGroups}
      onChange={(e) => setAwsRegion(e.target.value)}
    />
  )

  return (
    <SettingSection title={t('AWS Settings')} icon={FcKey}>
      <div className="space-y-2">
        <p className="text-xs text-ink-muted">
          {t('This application requires specific IAM permissions to access Amazon Bedrock.')}{' '}
          <button
            onClick={() => setIsPolicyModalOpen(true)}
            className="text-accent hover:underline"
          >
            {t('View required IAM policies')}
          </button>
        </p>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <label className="inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                className="form-checkbox h-4 w-4 text-accent rounded-control border-strong
                  focus:ring-accent ring-offset-surface
                  focus:ring-2 bg-raised border-subtle"
                checked={useAwsProfile}
                onChange={(e) => setUseAwsProfile(e.target.checked)}
              />
              <span className="ml-2 text-sm text-ink">{t('Use AWS Profile')}</span>
            </label>
          </div>

          {useAwsProfile ? (
            <div className="space-y-2 p-2.5 border border-subtle rounded-control">
              <p className="text-xs text-ink-muted">{t('Use credentials from ~/.aws')}</p>

              <SettingInput
                label={t('AWS Profile Name')}
                type="string"
                placeholder="default"
                value={awsProfile}
                onChange={(e) => setAwsProfile(e.target.value)}
              />

              {regionSelect}
            </div>
          ) : (
            <div className="space-y-2 p-2.5 border border-subtle rounded-control">
              <SettingInput
                label={t('AWS Access Key ID')}
                type="string"
                placeholder="AKXXXXXXXXXXXXXXXXXX"
                value={awsAccessKeyId}
                onChange={(e) => setAwsAccessKeyId(e.target.value)}
              />

              <SettingInput
                label={t('AWS Secret Access Key')}
                type="password"
                placeholder="****************************************"
                value={awsSecretAccessKey}
                onChange={(e) => setAwsSecretAccessKey(e.target.value)}
              />

              <SettingInput
                label={t('AWS Session Token (optional)')}
                type="password"
                placeholder="****************************************"
                value={awsSessionToken}
                onChange={(e) => setAwsSessionToken(e.target.value)}
              />

              {regionSelect}
            </div>
          )}
        </div>
      </div>

      <IAMPolicyModal isOpen={isPolicyModalOpen} onClose={() => setIsPolicyModalOpen(false)} />
    </SettingSection>
  )
}
