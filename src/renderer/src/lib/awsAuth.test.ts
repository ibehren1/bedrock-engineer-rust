import { hasAwsCredentials, resolveAwsAuthMode, type AwsCredentialState } from './awsAuth'

const base: AwsCredentialState = {
  authMode: 'accessKey',
  region: 'us-east-1',
  accessKeyId: '',
  secretAccessKey: '',
  bedrockApiKey: '',
  savedSecretAccessKey: false,
  savedBedrockApiKey: false
}

describe('resolveAwsAuthMode', () => {
  it('uses authMode when set', () => {
    expect(resolveAwsAuthMode({ authMode: 'apiKey', useProfile: true })).toBe('apiKey')
    expect(resolveAwsAuthMode({ authMode: 'accessKey', useProfile: true })).toBe('accessKey')
  })

  it('falls back to useProfile for older configs', () => {
    expect(resolveAwsAuthMode({ useProfile: true })).toBe('profile')
    expect(resolveAwsAuthMode({ useProfile: false })).toBe('accessKey')
    expect(resolveAwsAuthMode({})).toBe('accessKey')
    expect(resolveAwsAuthMode(undefined)).toBe('accessKey')
  })

  it('ignores an unknown authMode', () => {
    expect(resolveAwsAuthMode({ authMode: 'bogus' as never, useProfile: true })).toBe('profile')
  })
})

describe('hasAwsCredentials', () => {
  it('always needs a region', () => {
    for (const authMode of ['accessKey', 'profile', 'apiKey'] as const) {
      expect(
        hasAwsCredentials({
          ...base,
          authMode,
          region: '',
          accessKeyId: 'AK',
          secretAccessKey: 's',
          bedrockApiKey: 'k'
        })
      ).toBe(false)
    }
  })

  it('access key mode needs an id and a typed or saved secret', () => {
    expect(hasAwsCredentials({ ...base, accessKeyId: 'AK' })).toBe(false)
    expect(hasAwsCredentials({ ...base, secretAccessKey: 's' })).toBe(false)
    expect(hasAwsCredentials({ ...base, accessKeyId: 'AK', secretAccessKey: 's' })).toBe(true)
    expect(hasAwsCredentials({ ...base, accessKeyId: 'AK', savedSecretAccessKey: true })).toBe(true)
  })

  it('profile mode needs only a region', () => {
    expect(hasAwsCredentials({ ...base, authMode: 'profile' })).toBe(true)
  })

  it('API key mode needs a typed or saved key', () => {
    expect(hasAwsCredentials({ ...base, authMode: 'apiKey', accessKeyId: 'AK' })).toBe(false)
    expect(hasAwsCredentials({ ...base, authMode: 'apiKey', bedrockApiKey: 'k' })).toBe(true)
    expect(hasAwsCredentials({ ...base, authMode: 'apiKey', savedBedrockApiKey: true })).toBe(true)
  })
})
