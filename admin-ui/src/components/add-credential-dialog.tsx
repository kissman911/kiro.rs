import { useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useAddCredential } from '@/hooks/use-credentials'
import { useProxyPool } from '@/hooks/use-proxy-pool'
import { extractErrorMessage } from '@/lib/utils'

interface AddCredentialDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

type AuthMethod = 'social' | 'idc' | 'api_key' | 'external_idp'

const DEFAULT_PROXY_USERNAME = 'kmkmhuyw'
const DEFAULT_PROXY_PASSWORD = '3d1it5o1kxnu'

export function AddCredentialDialog({ open, onOpenChange }: AddCredentialDialogProps) {
  const [refreshToken, setRefreshToken] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [kiroApiKey, setKiroApiKey] = useState('')
  const [authMethod, setAuthMethod] = useState<AuthMethod>('social')
  const [authRegion, setAuthRegion] = useState('')
  const [apiRegion, setApiRegion] = useState('')
  const [clientId, setClientId] = useState('')
  const [clientSecret, setClientSecret] = useState('')
  const [tokenEndpoint, setTokenEndpoint] = useState('')
  const [issuerUrl, setIssuerUrl] = useState('')
  const [scopes, setScopes] = useState('')
  const [priority, setPriority] = useState('0')
  const [machineId, setMachineId] = useState('')
  const [proxyUrl, setProxyUrl] = useState('')
  const [proxyUsername, setProxyUsername] = useState(DEFAULT_PROXY_USERNAME)
  const [proxyPassword, setProxyPassword] = useState(DEFAULT_PROXY_PASSWORD)
  // 代理来源：auto=池自动分配 / pool=手动指定池内IP / manual=手填
const [proxySource, setProxySource] = useState<'auto' | 'pool' | 'manual'>('auto')
  const [selectedProxyId, setSelectedProxyId] = useState('')
  const [proxyAllowReuse, setProxyAllowReuse] = useState(false)
  const [endpoint, setEndpoint] = useState('')
  const [allowOverage, setAllowOverage] = useState(false)
  const [rateLimitWindow, setRateLimitWindow] = useState('')
  const [rateLimitMaxRequests, setRateLimitMaxRequests] = useState('')

  const { mutate, isPending } = useAddCredential()
  const { data: proxyPool } = useProxyPool()

  const resetForm = () => {
    setRefreshToken('')
    setDisplayName('')
    setKiroApiKey('')
    setAuthMethod('social')
    setAuthRegion('')
    setApiRegion('')
    setClientId('')
    setClientSecret('')
    setTokenEndpoint('')
    setIssuerUrl('')
    setScopes('')
    setPriority('0')
    setMachineId('')
    setProxyUrl('')
    setProxyUsername(DEFAULT_PROXY_USERNAME)
    setProxyPassword(DEFAULT_PROXY_PASSWORD)
    setProxySource('auto')
    setSelectedProxyId('')
    setProxyAllowReuse(false)
    setEndpoint('')
    setAllowOverage(false)
    setRateLimitWindow('')
    setRateLimitMaxRequests('')
  }

  const isApiKey = authMethod === 'api_key'

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    // 验证必填字段
    if (isApiKey) {
      if (!kiroApiKey.trim()) {
        toast.error('请输入 Kiro API Key')
        return
      }
    } else {
      if (!refreshToken.trim()) {
        toast.error('请输入 Refresh Token')
        return
      }
      // IdC/Builder-ID/IAM 需要额外字段
      if (authMethod === 'idc' && (!clientId.trim() || !clientSecret.trim())) {
        toast.error('IdC/Builder-ID/IAM 认证需要填写 Client ID 和 Client Secret')
        return
      }
      if (authMethod === 'external_idp' && (!clientId.trim() || !tokenEndpoint.trim())) {
        toast.error('企业 SSO / M365 认证需要填写 Client ID 和 Token Endpoint')
        return
      }
    }

    let rateLimits
    const hasRateLimit = rateLimitWindow.trim() || rateLimitMaxRequests.trim()
    if (hasRateLimit) {
      const window = rateLimitWindow.trim()
      const maxRequests = Number(rateLimitMaxRequests.trim())
      if (!/^\d+[smhd]$/.test(window)) {
        toast.error('限流窗口格式必须类似 30s / 1m / 5m / 1h / 1d')
        return
      }
      if (!Number.isInteger(maxRequests) || maxRequests <= 0) {
        toast.error('限流请求数必须是大于 0 的整数')
        return
      }
      rateLimits = [{ window, maxRequests }]
    }

    if (proxySource === 'pool' && !selectedProxyId) {
      toast.error('请选择一个代理池中的 IP')
      return
    }

    // 代理字段：根据来源组装
    // - manual：手填 proxyUrl/username/password，不走池
    // - pool：传 proxyId（+ allowReuse），后端从池分配
    // - auto：传 usePool=true，后端自动挑空闲/复用
    const proxyFields =
      proxySource === 'manual'
        ? {
            proxyUrl: proxyUrl.trim() || undefined,
            proxyUsername: proxyUsername.trim() || undefined,
            proxyPassword: proxyPassword.trim() || undefined,
            usePool: false as const,
          }
        : proxySource === 'pool'
          ? {
              proxyId: Number(selectedProxyId),
              proxyAllowReuse: proxyAllowReuse || undefined,
            }
          : { usePool: true as const }

    mutate(
      {
        authMethod,
        displayName: displayName.trim() || undefined,
        refreshToken: isApiKey ? undefined : refreshToken.trim(),
        kiroApiKey: isApiKey ? kiroApiKey.trim() : undefined,
        authRegion: authRegion.trim() || undefined,
        apiRegion: apiRegion.trim() || undefined,
        clientId: isApiKey ? undefined : clientId.trim() || undefined,
        clientSecret: isApiKey || authMethod === 'external_idp' ? undefined : clientSecret.trim() || undefined,
        tokenEndpoint: authMethod === 'external_idp' ? tokenEndpoint.trim() || undefined : undefined,
        issuerUrl: authMethod === 'external_idp' ? issuerUrl.trim() || undefined : undefined,
        scopes: authMethod === 'external_idp' ? scopes.trim() || undefined : undefined,
        provider: authMethod === 'external_idp' ? 'ExternalIdp' : undefined,
        priority: parseInt(priority) || 0,
        machineId: machineId.trim() || undefined,
        ...proxyFields,
        endpoint: endpoint.trim() || undefined,
        allowOverage: allowOverage || undefined,
        rateLimits,
      },
      {
        onSuccess: (data) => {
          toast.success(data.message)
          onOpenChange(false)
          resetForm()
        },
        onError: (error: unknown) => {
          toast.error(`添加失败: ${extractErrorMessage(error)}`)
        },
      }
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>添加凭据</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col min-h-0 flex-1">
          <div className="space-y-4 py-4 overflow-y-auto flex-1 pr-1">
            {/* 认证方式 */}
            <div className="space-y-2">
              <label htmlFor="authMethod" className="text-sm font-medium">
                认证方式
              </label>
              <select
                id="authMethod"
                value={authMethod}
                onChange={(e) => setAuthMethod(e.target.value as AuthMethod)}
                disabled={isPending}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <option value="social">Social</option>
                <option value="idc">IdC/Builder-ID/IAM</option>
                <option value="external_idp">企业 SSO / Microsoft 365</option>
                <option value="api_key">API Key</option>
              </select>
            </div>

            {/* 自定义名称 */}
            <div className="space-y-2">
              <label htmlFor="displayName" className="text-sm font-medium">
                名称（可选）
              </label>
              <Input
                id="displayName"
                placeholder="自定义名称，便于区分凭据"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                disabled={isPending}
                maxLength={60}
              />
              <p className="text-xs text-muted-foreground">
                仅用于管理界面展示，留空则显示邮箱或凭据编号
              </p>
            </div>

            {/* Kiro API Key (API Key 模式) */}
            {isApiKey && (
              <div className="space-y-2">
                <label htmlFor="kiroApiKey" className="text-sm font-medium">
                  Kiro API Key <span className="text-red-500">*</span>
                </label>
                <Input
                  id="kiroApiKey"
                  type="password"
                  placeholder="格式: ksk_xxxxxxxx"
                  value={kiroApiKey}
                  onChange={(e) => setKiroApiKey(e.target.value)}
                  disabled={isPending}
                />
              </div>
            )}

            {/* Refresh Token (OAuth 模式) */}
            {!isApiKey && (
              <div className="space-y-2">
                <label htmlFor="refreshToken" className="text-sm font-medium">
                  Refresh Token <span className="text-red-500">*</span>
                </label>
                <Input
                  id="refreshToken"
                  type="password"
                  placeholder="请输入 Refresh Token"
                  value={refreshToken}
                  onChange={(e) => setRefreshToken(e.target.value)}
                  disabled={isPending}
                />
              </div>
            )}

            {/* Region 配置 */}
            <div className="space-y-2">
              <label className="text-sm font-medium">Region 配置</label>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <Input
                    id="authRegion"
                    placeholder="Auth Region"
                    value={authRegion}
                    onChange={(e) => setAuthRegion(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <div>
                  <Input
                    id="apiRegion"
                    placeholder="API Region"
                    value={apiRegion}
                    onChange={(e) => setApiRegion(e.target.value)}
                    disabled={isPending}
                  />
                </div>
              </div>
              <p className="text-xs text-muted-foreground">
                均可留空使用全局配置。Auth Region 用于 Token 刷新，API Region 用于 API 请求
              </p>
            </div>

            {/* IdC/Builder-ID/IAM 额外字段 */}
            {authMethod === 'idc' && (
              <>
                <div className="space-y-2">
                  <label htmlFor="clientId" className="text-sm font-medium">
                    Client ID <span className="text-red-500">*</span>
                  </label>
                  <Input
                    id="clientId"
                    placeholder="请输入 Client ID"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="clientSecret" className="text-sm font-medium">
                    Client Secret <span className="text-red-500">*</span>
                  </label>
                  <Input
                    id="clientSecret"
                    type="password"
                    placeholder="请输入 Client Secret"
                    value={clientSecret}
                    onChange={(e) => setClientSecret(e.target.value)}
                    disabled={isPending}
                  />
                </div>
              </>
            )}



            {/* 企业 SSO / Microsoft 365 额外字段 */}
            {authMethod === 'external_idp' && (
              <>
                <div className="space-y-2">
                  <label htmlFor="clientId" className="text-sm font-medium">
                    Client ID <span className="text-red-500">*</span>
                  </label>
                  <Input
                    id="clientId"
                    placeholder="请输入 Microsoft 365 / Entra ID Client ID"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="tokenEndpoint" className="text-sm font-medium">
                    Token Endpoint <span className="text-red-500">*</span>
                  </label>
                  <Input
                    id="tokenEndpoint"
                    placeholder="https://login.microsoftonline.com/.../oauth2/v2.0/token"
                    value={tokenEndpoint}
                    onChange={(e) => setTokenEndpoint(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="scopes" className="text-sm font-medium">
                    Scopes
                  </label>
                  <Input
                    id="scopes"
                    placeholder="codewhisperer scopes + offline_access"
                    value={scopes}
                    onChange={(e) => setScopes(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <div className="space-y-2">
                  <label htmlFor="issuerUrl" className="text-sm font-medium">
                    Issuer URL（可选）
                  </label>
                  <Input
                    id="issuerUrl"
                    placeholder="https://login.microsoftonline.com/<tenant>/v2.0"
                    value={issuerUrl}
                    onChange={(e) => setIssuerUrl(e.target.value)}
                    disabled={isPending}
                  />
                </div>
              </>
            )}

            {/* 优先级 */}
            <div className="space-y-2">
              <label htmlFor="priority" className="text-sm font-medium">
                优先级
              </label>
              <Input
                id="priority"
                type="number"
                min="0"
                placeholder="数字越小优先级越高"
                value={priority}
                onChange={(e) => setPriority(e.target.value)}
                disabled={isPending}
              />
              <p className="text-xs text-muted-foreground">
                数字越小优先级越高，默认为 0
              </p>
            </div>

            {/* Machine ID */}
            <div className="space-y-2">
              <label htmlFor="machineId" className="text-sm font-medium">
                Machine ID
              </label>
              <Input
                id="machineId"
                placeholder="留空使用配置中字段, 否则由刷新Token自动派生"
                value={machineId}
                onChange={(e) => setMachineId(e.target.value)}
                disabled={isPending}
              />
              <p className="text-xs text-muted-foreground">
                可选，64 位十六进制字符串，留空使用配置中字段, 否则由刷新Token自动派生
              </p>
            </div>


            {/* 端点 */}
            <div className="space-y-2">
              <label htmlFor="endpoint" className="text-sm font-medium">
                端点
              </label>
              <Input
                id="endpoint"
                placeholder="留空使用默认端点（如 ide / cli）"
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                disabled={isPending}
              />
              <p className="text-xs text-muted-foreground">
                可选。决定该凭据走哪套 Kiro API。留空使用全局 defaultEndpoint
              </p>
            </div>

            {/* 超额配置 */}
            <label className="flex items-start gap-3 rounded-md border p-3 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={allowOverage}
                onChange={(e) => setAllowOverage(e.target.checked)}
                disabled={isPending}
              />
              <span>
                <span className="font-medium">允许超额使用</span>
                <span className="block text-xs text-muted-foreground">
                  本地余额展示按原始限额 + 10000 计算。若上游真实返回额度耗尽，仍会禁用该凭据，避免请求死循环。
                </span>
              </span>
            </label>

            {/* 限流配置 */}
            <div className="space-y-2">
              <label className="text-sm font-medium">RPM / 限流配置</label>
              <div className="grid grid-cols-2 gap-2">
                <Input
                  id="rateLimitWindow"
                  placeholder="窗口，如 1m"
                  value={rateLimitWindow}
                  onChange={(e) => setRateLimitWindow(e.target.value)}
                  disabled={isPending}
                />
                <Input
                  id="rateLimitMaxRequests"
                  type="number"
                  min="1"
                  placeholder="最大请求数，如 5"
                  value={rateLimitMaxRequests}
                  onChange={(e) => setRateLimitMaxRequests(e.target.value)}
                  disabled={isPending}
                />
              </div>
              <p className="text-xs text-muted-foreground">
                可选。例：1m + 5 = 每分钟最多 5 次。不填则使用全局默认限流
              </p>
            </div>

            {/* 代理配置 */}
            <div className="space-y-2">
              <label className="text-sm font-medium">代理来源</label>
              <select
                value={proxySource}
                onChange={(e) => setProxySource(e.target.value as 'auto' | 'pool' | 'manual')}
                disabled={isPending}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <option value="auto">从代理池自动分配（优先空闲 IP）</option>
                <option value="pool">手动指定代理池中的 IP</option>
                <option value="manual">手填代理（不走池）</option>
              </select>

              {proxySource === 'auto' && (
                <p className="text-xs text-muted-foreground">
                  从代理池自动挑空闲 IP（分配前会探测可用性）；无空闲时复用负载最低的在用 IP。
                  {proxyPool && (
                    <span className="block">
                      当前池：{proxyPool.available} 空闲 / {proxyPool.total} 总数
                      {proxyPool.total === 0 && '（池为空，将不分配代理）'}
                    </span>
                  )}
                </p>
              )}

              {proxySource === 'pool' && (
                <>
                  <select
                    value={selectedProxyId}
                    onChange={(e) => setSelectedProxyId(e.target.value)}
                    disabled={isPending}
                    className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <option value="">-- 选择代理 IP --</option>
                    {(proxyPool?.proxies ?? [])
                      .filter((p) => !p.disabled)
                      .map((p) => (
                        <option key={p.id} value={String(p.id)}>
                          #{p.id} {p.label || p.url}
                          {p.free ? '（空闲）' : `（在用 ${p.usageCount}）`}
                        </option>
                      ))}
                  </select>
                  <label className="flex items-center gap-2 text-xs text-muted-foreground">
                    <input
                      type="checkbox"
                      checked={proxyAllowReuse}
                      onChange={(e) => setProxyAllowReuse(e.target.checked)}
                      disabled={isPending}
                    />
                    允许复用已在用的 IP（多凭据共享）
                  </label>
                </>
              )}

              {proxySource === 'manual' && (
                <>
                  <Input
                    id="proxyUrl"
                    placeholder='代理 URL（留空使用全局配置，"direct" 不使用代理）'
                    value={proxyUrl}
                    onChange={(e) => setProxyUrl(e.target.value)}
                    disabled={isPending}
                  />
                  <div className="grid grid-cols-2 gap-2">
                    <Input
                      id="proxyUsername"
                      placeholder="代理用户名"
                      value={proxyUsername}
                      onChange={(e) => setProxyUsername(e.target.value)}
                      disabled={isPending}
                    />
                    <Input
                      id="proxyPassword"
                      type="password"
                      placeholder="代理密码"
                      value={proxyPassword}
                      onChange={(e) => setProxyPassword(e.target.value)}
                      disabled={isPending}
                    />
                  </div>
                  <p className="text-xs text-muted-foreground">
                    留空使用全局代理。输入 "direct" 可显式不使用代理
                  </p>
                </>
              )}
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={isPending}
            >
              取消
            </Button>
            <Button type="submit" disabled={isPending}>
              {isPending ? '添加中...' : '添加'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
