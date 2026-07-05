export interface NormalizedCredentialInput {
  refreshToken?: string
  accessToken?: string
  clientId?: string
  clientSecret?: string
  profileArn?: string
  tokenEndpoint?: string
  issuerUrl?: string
  scopes?: string
  provider?: string
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
  region?: string
  authRegion?: string
  apiRegion?: string
  priority?: number
  machineId?: string
  kiroApiKey?: string
  authMethod?: string
  endpoint?: string
  allowOverage?: boolean
  startUrl?: string
  email?: string
  userId?: string | null
  nickname?: string
  label?: string
  status?: string
}

const FIELD_ALIASES: Record<string, keyof NormalizedCredentialInput> = {
  refresh_token: 'refreshToken',
  access_token: 'accessToken',
  client_id: 'clientId',
  client_secret: 'clientSecret',
  profile_arn: 'profileArn',
  issuer_url: 'issuerUrl',
  token_endpoint: 'tokenEndpoint',
  auth_method: 'authMethod',
  start_url: 'startUrl',
  machine_id: 'machineId',
  kiro_api_key: 'kiroApiKey',
  proxy_url: 'proxyUrl',
  proxy_username: 'proxyUsername',
  proxy_password: 'proxyPassword',
  auth_region: 'authRegion',
  api_region: 'apiRegion',
  allow_overage: 'allowOverage',
  user_id: 'userId',
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function normalizePrimitive(value: unknown): string | number | boolean | null | undefined {
  if (value === undefined || value === null || value === '') return undefined
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') return value
  return undefined
}

export function normalizeCredentialFields(raw: unknown): NormalizedCredentialInput {
  if (!isRecord(raw)) return {}

  const normalized: Record<string, unknown> = { ...raw }
  for (const [snakeKey, camelKey] of Object.entries(FIELD_ALIASES)) {
    const current = normalized[camelKey]
    if ((current === undefined || current === '') && normalized[snakeKey] !== undefined) {
      normalized[camelKey] = normalized[snakeKey]
    }
  }

  const result: NormalizedCredentialInput = {}
  for (const key of [
    'refreshToken', 'accessToken', 'clientId', 'clientSecret', 'profileArn',
    'tokenEndpoint', 'issuerUrl', 'scopes', 'provider', 'proxyUrl',
    'proxyUsername', 'proxyPassword', 'region', 'authRegion', 'apiRegion',
    'machineId', 'kiroApiKey', 'authMethod', 'endpoint', 'startUrl', 'email',
    'userId', 'nickname', 'label', 'status',
  ] as const) {
    const value = normalizePrimitive(normalized[key])
    if (value !== undefined && value !== null) {
      ;(result as Record<string, unknown>)[key] = value
    }
  }

  const priority = normalized.priority
  if (typeof priority === 'number') result.priority = priority
  else if (typeof priority === 'string' && priority.trim() !== '' && !Number.isNaN(Number(priority))) {
    result.priority = Number(priority)
  }

  const allowOverage = normalized.allowOverage
  if (typeof allowOverage === 'boolean') result.allowOverage = allowOverage
  else if (typeof allowOverage === 'string') result.allowOverage = allowOverage.toLowerCase() === 'true'

  return result
}
