import axios from 'axios'
import { storage } from '@/lib/storage'
import type {
  CredentialsStatusResponse,
  BalanceResponse,
  SuccessResponse,
  SetDisabledRequest,
  SetPriorityRequest,
  SetAllowOverageRequest,
  SetDisplayNameRequest,
  SetRateLimitsRequest,
  AddCredentialRequest,
  AddCredentialResponse,
  RateLimitRule,
  VersionInfoResponse,
  RuntimeSettingsResponse,
  UpdateRuntimeSettingsRequest,
} from '@/types/api'

// 创建 axios 实例
const api = axios.create({
  baseURL: '/api/admin',
  headers: {
    'Content-Type': 'application/json',
  },
})

// 请求拦截器添加 API Key
api.interceptors.request.use((config) => {
  const apiKey = storage.getApiKey()
  if (apiKey) {
    config.headers['x-api-key'] = apiKey
  }
  return config
})

// 获取所有凭据状态
export async function getCredentials(): Promise<CredentialsStatusResponse> {
  const { data } = await api.get<CredentialsStatusResponse>('/credentials')
  return data
}

// 设置凭据禁用状态
export async function setCredentialDisabled(
  id: number,
  disabled: boolean
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/disabled`,
    { disabled } as SetDisabledRequest
  )
  return data
}

// 设置凭据优先级
export async function setCredentialPriority(
  id: number,
  priority: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/priority`,
    { priority } as SetPriorityRequest
  )
  return data
}

// 设置凭据显示名称
export async function setCredentialDisplayName(
  id: number,
  displayName: string | null
): Promise<SuccessResponse> {
  const { data } = await api.put<SuccessResponse>(
    `/credentials/${id}/display-name`,
    { displayName } as SetDisplayNameRequest
  )
  return data
}

// 设置凭据超额模式
export async function setCredentialAllowOverage(
  id: number,
  allowOverage: boolean
): Promise<SuccessResponse> {
  const { data } = await api.put<SuccessResponse>(
    `/credentials/${id}/allow-overage`,
    { allowOverage } as SetAllowOverageRequest
  )
  return data
}

// 设置凭据级限流规则
export async function setCredentialRateLimits(
  id: number,
  rateLimits: RateLimitRule[] | null
): Promise<SuccessResponse> {
  const { data } = await api.put<SuccessResponse>(
    `/credentials/${id}/rate-limits`,
    { rateLimits } as SetRateLimitsRequest
  )
  return data
}

// 重置失败计数
export async function resetCredentialFailure(
  id: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/reset`)
  return data
}

// 手动退出风控冷却
export async function clearCredentialCooldown(
  id: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/clear-cooldown`)
  return data
}

// 强制刷新 Token
export async function forceRefreshToken(
  id: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/refresh`)
  return data
}

// 获取凭据余额
export async function getCredentialBalance(id: number): Promise<BalanceResponse> {
  const { data } = await api.get<BalanceResponse>(`/credentials/${id}/balance`)
  return data
}

// 添加新凭据
export async function addCredential(
  req: AddCredentialRequest
): Promise<AddCredentialResponse> {
  const { data } = await api.post<AddCredentialResponse>('/credentials', req)
  return data
}

// 删除凭据
export async function deleteCredential(id: number): Promise<SuccessResponse> {
  const { data } = await api.delete<SuccessResponse>(`/credentials/${id}`)
  return data
}

// 重置单个凭据的成功次数
export async function resetSuccessCount(id: number): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/reset-stats`)
  return data
}

// 重置所有凭据的成功次数
export async function resetAllSuccessCount(): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>('/credentials/reset-stats')
  return data
}

// 获取负载均衡模式
export async function getLoadBalancingMode(): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.get<{ mode: 'priority' | 'balanced' }>('/config/load-balancing')
  return data
}

// 设置负载均衡模式
export async function setLoadBalancingMode(mode: 'priority' | 'balanced'): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.put<{ mode: 'priority' | 'balanced' }>('/config/load-balancing', { mode })
  return data
}

// 获取运行时设置
export async function getRuntimeSettings(): Promise<RuntimeSettingsResponse> {
  const { data } = await api.get<RuntimeSettingsResponse>('/config/settings')
  return data
}

// 更新运行时设置
export async function updateRuntimeSettings(
  req: UpdateRuntimeSettingsRequest
): Promise<RuntimeSettingsResponse> {
  const { data } = await api.put<RuntimeSettingsResponse>('/config/settings', req)
  return data
}

// 获取版本信息
export async function getVersionInfo(): Promise<VersionInfoResponse> {
  const { data } = await api.get<VersionInfoResponse>('/version')
  return data
}
