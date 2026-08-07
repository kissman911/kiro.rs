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
  SetEndpointRequest,
  SetRateLimitsRequest,
  AddCredentialRequest,
  AddCredentialResponse,
  RateLimitRule,
  VersionInfoResponse,
  RuntimeSettingsResponse,
  UpdateRuntimeSettingsRequest,
  ProxyPoolResponse,
  ProxyEntryView,
  AddProxyRequest,
  UpdateProxyRequest,
  ProxyPoolSettingsResponse,
  UpdateProxyPoolSettingsRequest,
  CarpoolSettings,
  UpdateCarpoolSettingsRequest,
  ProxyTestResponse,
  BatchAddProxyResponse,
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

// 设置凭据端点（kirors-b 专属：ide / cli / aws 环境隔离切换）
export async function setCredentialEndpoint(
  id: number,
  endpoint: string | null
): Promise<SuccessResponse> {
  const { data } = await api.put<SuccessResponse>(
    `/credentials/${id}/endpoint`,
    { endpoint } as SetEndpointRequest
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

// ============ 代理池 ============

// 获取代理池（列表 + 统计 + 设置）
export async function getProxyPool(): Promise<ProxyPoolResponse> {
  const { data } = await api.get<ProxyPoolResponse>('/proxy-pool')
  return data
}

// 添加代理
export async function addProxy(req: AddProxyRequest): Promise<ProxyEntryView> {
  const { data } = await api.post<ProxyEntryView>('/proxy-pool', req)
  return data
}

// 批量添加代理
export async function batchAddProxy(
  lines: string[]
): Promise<BatchAddProxyResponse> {
  const { data } = await api.post<BatchAddProxyResponse>(
    '/proxy-pool/batch',
    { lines }
  )
  return data
}

// 更新代理
export async function updateProxy(
  id: number,
  req: UpdateProxyRequest
): Promise<ProxyEntryView> {
  const { data } = await api.put<ProxyEntryView>(`/proxy-pool/${id}`, req)
  return data
}

// 删除代理
export async function deleteProxy(id: number): Promise<SuccessResponse> {
  const { data } = await api.delete<SuccessResponse>(`/proxy-pool/${id}`)
  return data
}

// 启用/禁用代理
export async function setProxyDisabled(
  id: number,
  disabled: boolean
): Promise<ProxyEntryView> {
  const { data } = await api.post<ProxyEntryView>(`/proxy-pool/${id}/disabled`, {
    disabled,
  })
  return data
}

// 探测代理
export async function testProxy(id: number): Promise<ProxyTestResponse> {
  const { data } = await api.post<ProxyTestResponse>(`/proxy-pool/${id}/test`)
  return data
}

// 获取代理池设置
export async function getProxyPoolSettings(): Promise<ProxyPoolSettingsResponse> {
  const { data } = await api.get<ProxyPoolSettingsResponse>('/proxy-pool/settings')
  return data
}

// 更新代理池设置
export async function updateProxyPoolSettings(
  req: UpdateProxyPoolSettingsRequest
): Promise<ProxyPoolSettingsResponse> {
  const { data } = await api.put<ProxyPoolSettingsResponse>('/proxy-pool/settings', req)
  return data
}

// 拼车补号配置
export async function getCarpoolSettings(): Promise<CarpoolSettings> {
  const { data } = await api.get<CarpoolSettings>('/carpool/settings')
  return data
}

export async function updateCarpoolSettings(
  req: UpdateCarpoolSettingsRequest
): Promise<CarpoolSettings> {
  const { data } = await api.put<CarpoolSettings>('/carpool/settings', req)
  return data
}
