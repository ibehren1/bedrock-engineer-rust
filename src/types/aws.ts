/** HTTP(S) proxy used for AWS and web requests (settings key `aws.proxyConfig`). */
export type ProxyConfiguration = {
  enabled: boolean
  host?: string
  port?: number
  username?: string
  password?: string
  protocol?: 'http' | 'https'
}

/** AWS credentials and region (settings key `aws`). */
export type AWSCredentials = {
  accessKeyId: string
  secretAccessKey: string
  sessionToken?: string
  region: string
  profile?: string
  useProfile?: boolean
  proxyConfig?: ProxyConfiguration
  /**
   * Set by Rust in everything it sends the renderer, where `secretAccessKey`, `sessionToken`
   * and `proxyConfig.password` are always blank: whether each one is saved.
   */
  hasSecretAccessKey?: boolean
  hasSessionToken?: boolean
  hasProxyPassword?: boolean
  /** In a write: remove the saved secret (a blank one keeps it). */
  clearSecretAccessKey?: boolean
  clearSessionToken?: boolean
  clearProxyPassword?: boolean
}
