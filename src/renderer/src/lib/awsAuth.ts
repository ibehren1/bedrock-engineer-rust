import type { AWSCredentials, AwsAuthMode } from '@/types/aws'

/** The effective sign-in method of a stored `aws` value (configs saved before `authMode`). */
export function resolveAwsAuthMode(
  aws: Pick<AWSCredentials, 'authMode' | 'useProfile'> | undefined | null
): AwsAuthMode {
  const mode = aws?.authMode
  if (mode === 'accessKey' || mode === 'profile' || mode === 'apiKey') return mode
  return aws?.useProfile ? 'profile' : 'accessKey'
}

export type AwsCredentialState = {
  authMode: AwsAuthMode
  region: string
  accessKeyId: string
  /** What the user typed this session (saved secrets stay in Rust and arrive blank). */
  secretAccessKey: string
  bedrockApiKey: string
  /** Whether Rust holds a saved secret access key / Bedrock API key. */
  savedSecretAccessKey: boolean
  savedBedrockApiKey: boolean
}

/**
 * Whether enough is set to call Bedrock with the chosen sign-in method. A profile is only
 * checked for a region: the SDK resolves the rest (default profile, SSO, env) at call time.
 */
export function hasAwsCredentials(s: AwsCredentialState): boolean {
  if (!s.region) return false
  switch (s.authMode) {
    case 'profile':
      return true
    case 'apiKey':
      return Boolean(s.bedrockApiKey || s.savedBedrockApiKey)
    case 'accessKey':
    default:
      return Boolean(s.accessKeyId && (s.secretAccessKey || s.savedSecretAccessKey))
  }
}
