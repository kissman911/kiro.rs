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

export type RequestEventKind =
  | 'success'
  | 'transientError'
  | 'suspiciousRateLimit'
  | 'hardFailure'
  | 'quotaExhausted'
  | 'refreshFailure'

export interface RequestEventItem {
  kind: RequestEventKind
  at: string
  status?: number
  message?: string
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
  requestHistory: RequestEventItem[]
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

// 可用端点（与后端 KNOWN_ENDPOINTS 保持一致）
export const KIRO_ENDPOINTS = ['ide', 'cli', 'aws'] as const
export type KiroEndpointName = (typeof KIRO_ENDPOINTS)[number]

// 端点展示元信息：区分「AWS 原版旧端点」与「Kiro 新端点」
export const KIRO_ENDPOINT_META: Record<
  KiroEndpointName,
  { label: string; family: 'kiro' | 'aws'; domain: string; hint: string }
> = {
  ide: {
    label: 'Kiro IDE',
    family: 'kiro',
    domain: 'runtime.*.kiro.dev',
    hint: 'Kiro 新端点（IDE）：推理 runtime.*.kiro.dev，额度 management.*.kiro.dev。profileArn 必需。',
  },
  cli: {
    label: 'Kiro CLI',
    family: 'kiro',
    domain: 'runtime.*.kiro.dev',
    hint: 'Kiro 新端点（CLI）：AWS JSON 1.0 协议走根路径，额度 management.*.kiro.dev。',
  },
  aws: {
    label: 'AWS 原版',
    family: 'aws',
    domain: 'q.*.amazonaws.com',
    hint: 'AWS 原版旧端点（上游 hank9999/kiro.rs 同款）：推理与额度均走 q.*.amazonaws.com。profileArn 可缺省，校验更宽松。',
  },
}

export interface SetEndpointRequest {
  endpoint: string | null
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
  // 代理池：手动指定代理 ID / 允许复用在用 IP / 自动分配开关
  proxyId?: number
  proxyAllowReuse?: boolean
  // 手动指定代理探测失败时是否仍强制创建（默认 false = 拒绝）
  proxyAllowProbeFailure?: boolean
  usePool?: boolean
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

// 运行时设置响应
export interface RuntimeSettingsResponse {
  suspiciousCooldownMinutes: number
  suspiciousCooldownSeconds: number
  extractThinking: boolean
  nativeLikeTwoPhaseFlow: boolean
}

// 更新运行时设置请求（字段均可选，只更新传入的字段）
export interface UpdateRuntimeSettingsRequest {
  suspiciousCooldownMinutes?: number
  extractThinking?: boolean
  nativeLikeTwoPhaseFlow?: boolean
}

// ============ 代理池 ============

export interface ProbeResult {
  ok: boolean
  latencyMs?: number
  ip?: string
  message?: string
  at: string
}

export interface ProxyEntryView {
  id: number
  url: string
  username?: string
  label: string
  disabled: boolean
  assignments: number[]
  usageCount: number
  free: boolean
  lastCheck?: ProbeResult
}

export interface ProxyPoolResponse {
  total: number
  available: number
  assigned: number
  shared: number
  disabled: number
  autoAssignEnabled: boolean
  probeUrl: string
  proxies: ProxyEntryView[]
}

export interface AddProxyRequest {
  url: string
  username?: string
  password?: string
  label?: string
}

export interface UpdateProxyRequest {
  url?: string
  username?: string | null
  password?: string | null
  label?: string
}

export interface ProxyPoolSettingsResponse {
  autoAssignEnabled: boolean
  probeUrl: string
}

export interface UpdateProxyPoolSettingsRequest {
  autoAssignEnabled?: boolean
  probeUrl?: string
}

// 拼车补号配置（kirors-b 专属，daemon 消费）
export interface CarpoolSettings {
  enabled: boolean
  getUrl: string
  targetActive: number
  pollInterval: number
  dryRun: boolean
  recentWindow: number
  minSample: number
  disableErrRatio: number
  healthyErrRatio: number
}

export interface UpdateCarpoolSettingsRequest {
  enabled?: boolean
  getUrl?: string
  targetActive?: number
  pollInterval?: number
  dryRun?: boolean
  recentWindow?: number
  minSample?: number
  disableErrRatio?: number
  healthyErrRatio?: number
}

export interface ProxyTestResponse {
  success: boolean
  message: string
  latencyMs?: number
  ip?: string
}

export interface BatchAddError {
  line: number
  content: string
  error: string
}

export interface BatchAddProxyResponse {
  added: number
  proxies: ProxyEntryView[]
  errors: BatchAddError[]
}
