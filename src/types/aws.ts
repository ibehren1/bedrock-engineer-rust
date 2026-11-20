/** HTTP(S) proxy used for AWS and web requests (settings key `aws.proxyConfig`). */
export type ProxyConfiguration = {
  enabled: boolean
  host?: string
  port?: number
  username?: string
  password?: string
  protocol?: 'http' | 'https'
}

/**
 * How the app signs in to AWS: access key + secret key, a named profile from `~/.aws`, or an
 * Amazon Bedrock API key (bearer token; Bedrock Runtime and Bedrock control plane only).
 */
export type AwsAuthMode = 'accessKey' | 'profile' | 'apiKey'

/** AWS credentials and region (settings key `aws`). */
export type AWSCredentials = {
  accessKeyId: string
  secretAccessKey: string
  sessionToken?: string
  region: string
  profile?: string
  /** Kept in step with `authMode` (`authMode === 'profile'`) so older builds still read it. */
  useProfile?: boolean
  /** Source of truth for the sign-in method; absent → `useProfile ? 'profile' : 'accessKey'`. */
  authMode?: AwsAuthMode
  /** Amazon Bedrock API key (secret: always blank in what Rust sends the renderer). */
  bedrockApiKey?: string
  proxyConfig?: ProxyConfiguration
  /**
   * Set by Rust in everything it sends the renderer, where `secretAccessKey`, `sessionToken`,
   * `bedrockApiKey` and `proxyConfig.password` are always blank: whether each one is saved.
   */
  hasSecretAccessKey?: boolean
  hasSessionToken?: boolean
  hasProxyPassword?: boolean
  hasBedrockApiKey?: boolean
  /** In a write: remove the saved secret (a blank one keeps it). */
  clearSecretAccessKey?: boolean
  clearSessionToken?: boolean
  clearProxyPassword?: boolean
  clearBedrockApiKey?: boolean
}
