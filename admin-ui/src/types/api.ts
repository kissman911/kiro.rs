// 版本信息响应
export interface VersionInfoResponse {
  version: string
  channel: string
  codename: string
  date: string
  summary: string
  packageVersion: string
  gitSha: string
  buildTag: string
  changelog: string
}

// 凭据状态响应
export interface CredentialsStatusResponse {
  total: number
  available: number
  currentId: number
  credentials: CredentialStatusItem[]
}

export interface RateLimitRule {
  window: string
  maxRequests: number
}

// 单个凭据状态
export interface CredentialStatusItem {
  id: number
  priority: number
  disabled: boolean
  failureCount: number
  isCurrent: boolean
  expiresAt: string | null
  authMethod: string | null
  hasProfileArn: boolean
  email?: string
  displayName?: string
  refreshTokenHash?: string
  apiKeyHash?: string
  maskedApiKey?: string
  successCount: number
  lastUsedAt: string | null
  hasProxy: boolean
  proxyUrl?: string
  refreshFailureCount: number
  disabledReason?: string
  endpoint: string
  allowOverage: boolean
  rateLimits?: RateLimitRule[]
  cooldownUntil?: string
  cooldownRemainingSeconds?: number
}

// 余额响应
export interface BalanceResponse {
  id: number
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  effectiveLimit: number
  overageAllowance: number
  remaining: number
  usagePercentage: number
  allowOverage: boolean
  overageActive: boolean
  nextResetAt: number | null
}

// 成功响应
export interface SuccessResponse {
  success: boolean
  message: string
}

// 错误响应
export interface AdminErrorResponse {
  error: {
    type: string
    message: string
  }
}

// 请求类型
export interface SetDisabledRequest {
  disabled: boolean
}

export interface SetPriorityRequest {
  priority: number
}

export interface SetRateLimitsRequest {
  rateLimits: RateLimitRule[] | null
}

export interface SetAllowOverageRequest {
  allowOverage: boolean
}

export interface SetDisplayNameRequest {
  displayName: string | null
}

// 添加凭据请求
export interface AddCredentialRequest {
  refreshToken?: string
  accessToken?: string
  profileArn?: string
  authMethod?: 'social' | 'idc' | 'api_key' | 'external_idp'
  clientId?: string
  clientSecret?: string
  tokenEndpoint?: string
  issuerUrl?: string
  scopes?: string
  provider?: string
  priority?: number
  authRegion?: string
  apiRegion?: string
  machineId?: string
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
  displayName?: string
  kiroApiKey?: string
  endpoint?: string
  allowOverage?: boolean
  rateLimits?: RateLimitRule[]
}

// 添加凭据响应
export interface AddCredentialResponse {
  success: boolean
  message: string
  credentialId: number
  email?: string
}
