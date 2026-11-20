import React, { useCallback, useEffect, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FcKey } from 'react-icons/fc'
import { FiAlertTriangle, FiRefreshCw } from 'react-icons/fi'
import type { AwsAuthMode } from '@/types/aws'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { SettingSection } from '../SettingSection'
import { SettingInput } from '../SettingInput'
import { SettingSelect } from '../SettingSelect'
import { IAMPolicyModal } from '../IAMPolicyModal'
import { buildRegionGroups } from '../../utils/regionOptions'
import { SavedSecretNote } from '../SavedSecretNote'

const SECRET_PLACEHOLDER = '****************************************'

/** The features the backend refuses under a Bedrock API key (they need SigV4). */
const API_KEY_UNAVAILABLE = [
  'agents',
  'knowledgeBases',
  'flows',
  'translate',
  'video',
  'agentFiles'
] as const

/** One choice of the sign-in radio group; its fields show only while it is selected. */
const AuthCard: React.FC<{
  mode: AwsAuthMode
  name: string
  selected: boolean
  title: string
  description: string
  onSelect: (mode: AwsAuthMode) => void
  children: React.ReactNode
}> = ({ mode, name, selected, title, description, onSelect, children }) => {
  const id = useId()
  return (
    <div
      className={`rounded-container border transition-colors ${
        selected ? 'border-accent bg-accent-tint' : 'border-strong hover:bg-surface-2'
      }`}
    >
      <label htmlFor={id} className="flex items-start gap-3 p-3 cursor-pointer">
        <input
          id={id}
          type="radio"
          name={name}
          value={mode}
          checked={selected}
          onChange={() => onSelect(mode)}
          aria-describedby={`${id}-desc`}
          className="mt-0.5 w-4 h-4 shrink-0 text-accent bg-raised border-strong cursor-pointer
            focus:ring-accent focus:ring-2 ring-offset-surface"
        />
        <span className="min-w-0">
          <span className="block text-sm font-medium text-ink">{title}</span>
          <span id={`${id}-desc`} className="block text-xs text-ink-muted">
            {description}
          </span>
        </span>
      </label>
      {selected && (
        <div className="space-y-2 px-3 pb-3 pt-2 border-t border-subtle bg-surface rounded-b-container">
          {children}
        </div>
      )}
    </div>
  )
}

export const AwsCredentialsSection: React.FC = () => {
  const { t } = useTranslation()
  const {
    awsRegion,
    setAwsRegion,
    awsAuthMode,
    setAwsAuthMode,
    awsAccessKeyId,
    setAwsAccessKeyId,
    awsSecretAccessKey,
    setAwsSecretAccessKey,
    awsSessionToken,
    setAwsSessionToken,
    bedrockApiKey,
    setBedrockApiKey,
    savedAwsSecrets,
    clearSavedAwsSecret,
    awsProfile,
    setAwsProfile
  } = useSettings()
  const [isPolicyModalOpen, setIsPolicyModalOpen] = useState(false)
  const groupName = useId()
  const groupLabelId = `${groupName}-label`

  // Profiles found on disk; null until the first load finishes.
  const [profiles, setProfiles] = useState<string[] | null>(null)
  const [profilesLoading, setProfilesLoading] = useState(false)
  const [profilesError, setProfilesError] = useState<string | null>(null)

  const loadProfiles = useCallback(async () => {
    setProfilesLoading(true)
    setProfilesError(null)
    try {
      const names = await window.api.aws.listProfiles()
      setProfiles(Array.isArray(names) ? names : [])
    } catch (e) {
      setProfiles([])
      setProfilesError(e instanceof Error ? e.message : String(e))
    } finally {
      setProfilesLoading(false)
    }
  }, [])

  const profileSelected = awsAuthMode === 'profile'
  useEffect(() => {
    if (profileSelected) void loadProfiles()
  }, [profileSelected, loadProfiles])

  // A cleared profile ('') takes the first one found; any other saved name is left alone,
  // even when it isn't on disk (it is shown as "not found" instead).
  useEffect(() => {
    if (profileSelected && profiles && profiles.length > 0 && awsProfile === '') {
      setAwsProfile(profiles[0])
    }
    // setAwsProfile is recreated every render; the values above decide when this runs.
  }, [profileSelected, profiles, awsProfile])

  const profileOptions = (() => {
    const found = profiles ?? []
    const options = found.map((name) => ({ value: name, label: name }))
    if (awsProfile && !found.includes(awsProfile)) {
      // Still loading: show the saved name plainly rather than flashing "not found".
      const label =
        profiles === null ? awsProfile : t('awsAuth.profile.notFound', { name: awsProfile })
      options.unshift({ value: awsProfile, label })
    }
    if (!awsProfile) options.unshift({ value: '', label: t('awsAuth.profile.select') })
    return options
  })()

  const regionSelect = (
    <SettingSelect
      label={t('AWS Region')}
      value={awsRegion}
      options={[{ value: '', label: t('Select a region') }]}
      groups={buildRegionGroups(t)}
      onChange={(e) => setAwsRegion(e.target.value)}
    />
  )

  return (
    <SettingSection title={t('AWS Authentication')} icon={FcKey}>
      <div className="space-y-3">
        <p className="text-xs text-ink-muted">
          {t('This application requires specific IAM permissions to access Amazon Bedrock.')}{' '}
          <button
            type="button"
            onClick={() => setIsPolicyModalOpen(true)}
            className="text-accent hover:underline"
          >
            {t('View required IAM policies')}
          </button>
        </p>

        <div className="space-y-1">
          <span id={groupLabelId} className="block text-sm font-medium text-ink">
            {t('awsAuth.method')}
          </span>
          <div role="radiogroup" aria-labelledby={groupLabelId} className="space-y-2">
            <AuthCard
              mode="accessKey"
              name={groupName}
              selected={awsAuthMode === 'accessKey'}
              title={t('awsAuth.accessKey.title')}
              description={t('awsAuth.accessKey.description')}
              onSelect={setAwsAuthMode}
            >
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
                placeholder={
                  savedAwsSecrets.secretAccessKey
                    ? t('savedSecret.placeholder')
                    : SECRET_PLACEHOLDER
                }
                value={awsSecretAccessKey}
                onChange={(e) => setAwsSecretAccessKey(e.target.value)}
              />
              <SavedSecretNote
                saved={savedAwsSecrets.secretAccessKey && !awsSecretAccessKey}
                onClear={() => clearSavedAwsSecret('secretAccessKey')}
              />

              <SettingInput
                label={t('AWS Session Token (optional)')}
                type="password"
                placeholder={
                  savedAwsSecrets.sessionToken ? t('savedSecret.placeholder') : SECRET_PLACEHOLDER
                }
                value={awsSessionToken}
                onChange={(e) => setAwsSessionToken(e.target.value)}
              />
              <SavedSecretNote
                saved={savedAwsSecrets.sessionToken && !awsSessionToken}
                onClear={() => clearSavedAwsSecret('sessionToken')}
              />
            </AuthCard>

            <AuthCard
              mode="profile"
              name={groupName}
              selected={profileSelected}
              title={t('awsAuth.profile.title')}
              description={t('awsAuth.profile.description')}
              onSelect={setAwsAuthMode}
            >
              <div className="flex items-end gap-2">
                <div className="flex-1 min-w-0">
                  <SettingSelect
                    label={t('AWS Profile Name')}
                    description={t('awsAuth.profile.hint')}
                    value={awsProfile}
                    options={profileOptions}
                    disabled={profileOptions.length === 0}
                    onChange={(e) => setAwsProfile(e.target.value)}
                  />
                </div>
                <button
                  type="button"
                  onClick={() => void loadProfiles()}
                  disabled={profilesLoading}
                  title={t('awsAuth.profile.refresh')}
                  aria-label={t('awsAuth.profile.refresh')}
                  className="mb-0.5 p-3 rounded-container border border-strong text-ink-muted
                    hover:text-ink hover:bg-surface-2 disabled:opacity-50
                    focus:outline-none focus:ring-2 focus:ring-accent"
                >
                  <FiRefreshCw className={`size-4 ${profilesLoading ? 'animate-spin' : ''}`} />
                </button>
              </div>

              {profilesLoading && profiles === null && (
                <p className="text-xs text-ink-muted" role="status">
                  {t('awsAuth.profile.loading')}
                </p>
              )}
              {profilesError && (
                <p className="text-xs text-danger" role="alert">
                  {t('awsAuth.profile.loadError', { error: profilesError })}
                </p>
              )}
              {profiles !== null && profiles.length === 0 && !profilesError && (
                <div className="space-y-1.5 p-2.5 rounded-control border border-subtle bg-sunken text-xs text-ink-muted">
                  <p>{t('awsAuth.profile.none')}</p>
                  <code className="block font-mono text-ink">
                    aws configure --profile &lt;name&gt;
                  </code>
                  <code className="block font-mono text-ink">aws configure sso</code>
                </div>
              )}
            </AuthCard>

            <AuthCard
              mode="apiKey"
              name={groupName}
              selected={awsAuthMode === 'apiKey'}
              title={t('awsAuth.apiKey.title')}
              description={t('awsAuth.apiKey.description')}
              onSelect={setAwsAuthMode}
            >
              <SettingInput
                label={t('awsAuth.apiKey.label')}
                description={t('awsAuth.apiKey.hint')}
                type="password"
                autoComplete="off"
                placeholder={
                  savedAwsSecrets.bedrockApiKey ? t('savedSecret.placeholder') : SECRET_PLACEHOLDER
                }
                value={bedrockApiKey}
                onChange={(e) => setBedrockApiKey(e.target.value)}
              />
              <SavedSecretNote
                saved={savedAwsSecrets.bedrockApiKey && !bedrockApiKey}
                onClear={() => clearSavedAwsSecret('bedrockApiKey')}
              />

              <div className="flex gap-2 p-2.5 rounded-control border border-warning bg-warning-soft text-xs">
                <FiAlertTriangle className="size-4 shrink-0 text-warning" aria-hidden="true" />
                <div className="space-y-1 text-ink">
                  <p className="font-medium">{t('awsAuth.apiKey.unavailableTitle')}</p>
                  <ul className="list-disc ps-4">
                    {API_KEY_UNAVAILABLE.map((key) => (
                      <li key={key}>{t(`awsAuth.apiKey.unavailable.${key}`)}</li>
                    ))}
                  </ul>
                  <p className="text-ink-muted">{t('awsAuth.apiKey.unavailableHint')}</p>
                </div>
              </div>
            </AuthCard>
          </div>
        </div>

        {regionSelect}
      </div>

      <IAMPolicyModal isOpen={isPolicyModalOpen} onClose={() => setIsPolicyModalOpen(false)} />
    </SettingSection>
  )
}
