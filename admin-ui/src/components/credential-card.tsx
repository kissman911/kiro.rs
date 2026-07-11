import { useState } from 'react'
import { toast } from 'sonner'
import { RefreshCw, ChevronUp, ChevronDown, Wallet, Trash2, Loader2, Gauge, Mail, Clock3, Tag } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { CredentialStatusItem, BalanceResponse, RateLimitRule, RequestEventItem, RequestEventKind } from '@/types/api'
import {
  useSetDisabled,
  useSetPriority,
  useSetAllowOverage,
  useSetDisplayName,
  useResetFailure,
  useClearCooldown,
  useDeleteCredential,
  useForceRefreshToken,
  useResetSuccessCount,
} from '@/hooks/use-credentials'
import { RateLimitDialog } from '@/components/rate-limit-dialog'

interface CredentialCardProps {
  credential: CredentialStatusItem
  onViewBalance: (id: number) => void
  selected: boolean
  onToggleSelect: () => void
  balance: BalanceResponse | null
  loadingBalance: boolean
}

function formatRateLimits(rules?: RateLimitRule[]): string {
  if (!rules || rules.length === 0) return '使用全局默认'
  return rules.map((rule) => `${rule.window}/${rule.maxRequests}`).join('，')
}


function requestEventLabel(kind: RequestEventKind): string {
  switch (kind) {
    case 'success': return '请求成功'
    case 'transientError': return '上游瞬态异常'
    case 'suspiciousRateLimit': return '账号风控冷却'
    case 'hardFailure': return '凭据硬失败'
    case 'quotaExhausted': return '额度耗尽'
    case 'refreshFailure': return '刷新失败'
    default: return kind
  }
}

function requestEventClass(kind: RequestEventKind): string {
  switch (kind) {
    case 'success':
      return 'bg-emerald-500 hover:bg-emerald-400'
    case 'transientError':
      return 'bg-red-500 hover:bg-red-400'
    case 'suspiciousRateLimit':
      return 'bg-amber-500 hover:bg-amber-400'
    case 'hardFailure':
      return 'bg-rose-700 hover:bg-rose-600'
    case 'quotaExhausted':
      return 'bg-purple-600 hover:bg-purple-500'
    case 'refreshFailure':
      return 'bg-orange-600 hover:bg-orange-500'
    default:
      return 'bg-slate-400 hover:bg-slate-300'
  }
}

function formatRequestEventTitle(event: RequestEventItem): string {
  const parts = [requestEventLabel(event.kind), new Date(event.at).toLocaleString()]
  if (event.status) parts.push(`HTTP ${event.status}`)
  if (event.message) parts.push(event.message)
  return parts.join(' · ')
}

function summarizeRequestHistory(events: RequestEventItem[]): string {
  if (!events.length) return '暂无请求记录'
  const success = events.filter((event) => event.kind === 'success').length
  const transient = events.filter((event) => event.kind === 'transientError').length
  const suspicious = events.filter((event) => event.kind === 'suspiciousRateLimit').length
  const hard = events.filter((event) => event.kind === 'hardFailure' || event.kind === 'refreshFailure').length
  const quota = events.filter((event) => event.kind === 'quotaExhausted').length
  return `近 ${events.length} 次：成功 ${success} / 瞬态 ${transient} / 风控 ${suspicious} / 硬失败 ${hard} / 额度 ${quota}`
}

function formatCooldown(seconds?: number): string {
  if (!seconds || seconds <= 0) return ''
  const minutes = Math.floor(seconds / 60)
  const remainSeconds = seconds % 60
  if (minutes <= 0) return `${remainSeconds} 秒`
  return `${minutes} 分 ${remainSeconds} 秒`
}

function formatLastUsed(lastUsedAt: string | null): string {
  if (!lastUsedAt) return '从未使用'
  const date = new Date(lastUsedAt)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  if (diff < 0) return '刚刚'
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds} 秒前`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  return `${days} 天前`
}

export function CredentialCard({
  credential,
  onViewBalance,
  selected,
  onToggleSelect,
  balance,
  loadingBalance,
}: CredentialCardProps) {
  const [editingPriority, setEditingPriority] = useState(false)
  const [priorityValue, setPriorityValue] = useState(String(credential.priority))
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [showRateLimitDialog, setShowRateLimitDialog] = useState(false)
  const [editingName, setEditingName] = useState(false)
  const [nameValue, setNameValue] = useState(credential.displayName ?? '')

  const setDisabled = useSetDisabled()
  const setPriority = useSetPriority()
  const setAllowOverage = useSetAllowOverage()
  const setDisplayName = useSetDisplayName()
  const resetFailure = useResetFailure()
  const clearCooldown = useClearCooldown()
  const deleteCredential = useDeleteCredential()
  const forceRefresh = useForceRefreshToken()
  const resetSuccess = useResetSuccessCount()

  const handleToggleDisabled = () => {
    setDisabled.mutate(
      { id: credential.id, disabled: !credential.disabled },
      {
        onSuccess: (res) => {
          toast.success(res.message)
        },
        onError: (err) => {
          toast.error('操作失败: ' + (err as Error).message)
        },
      }
    )
  }

  const handlePriorityChange = () => {
    const newPriority = parseInt(priorityValue, 10)
    if (isNaN(newPriority) || newPriority < 0) {
      toast.error('优先级必须是非负整数')
      return
    }
    setPriority.mutate(
      { id: credential.id, priority: newPriority },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingPriority(false)
        },
        onError: (err) => {
          toast.error('操作失败: ' + (err as Error).message)
        },
      }
    )
  }

  const handleSaveName = () => {
    const trimmed = nameValue.trim()
    setDisplayName.mutate(
      { id: credential.id, displayName: trimmed === '' ? null : trimmed },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingName(false)
        },
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handleToggleAllowOverage = () => {
    setAllowOverage.mutate(
      { id: credential.id, allowOverage: !credential.allowOverage },
      {
        onSuccess: (res) => toast.success(res.message),
        onError: (err) => toast.error('操作失败: ' + (err as Error).message),
      }
    )
  }

  const handleReset = () => {
    resetFailure.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
      },
      onError: (err) => {
        toast.error('操作失败: ' + (err as Error).message)
      },
    })
  }

  const handleForceRefresh = () => {
    forceRefresh.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
      },
      onError: (err) => {
        toast.error('刷新失败: ' + (err as Error).message)
      },
    })
  }

  const handleResetSuccess = () => {
    resetSuccess.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
      },
      onError: (err) => {
        toast.error('重置失败: ' + (err as Error).message)
      },
    })
  }

  const handleClearCooldown = () => {
    clearCooldown.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
      },
      onError: (err) => {
        toast.error('退出冷却失败: ' + (err as Error).message)
      },
    })
  }

  const handleDelete = () => {
    if (!credential.disabled) {
      toast.error('请先禁用凭据再删除')
      setShowDeleteDialog(false)
      return
    }

    deleteCredential.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
        setShowDeleteDialog(false)
      },
      onError: (err) => {
        toast.error('删除失败: ' + (err as Error).message)
      },
    })
  }

  const credentialOwner = credential.email?.trim() || '未识别邮箱'
  const isCoolingDown = Boolean(credential.cooldownRemainingSeconds && credential.cooldownRemainingSeconds > 0)
  const cooldownText = formatCooldown(credential.cooldownRemainingSeconds)
  const requestHistory = credential.requestHistory ?? []
  const latestRequestEvent = requestHistory[requestHistory.length - 1]
  const requestHistorySummary = summarizeRequestHistory(requestHistory)

  return (
    <>
      <Card className={credential.isCurrent ? 'ring-2 ring-primary overflow-visible' : 'overflow-visible'}>
        <CardHeader className="pb-3 space-y-3">
          <div className="flex items-start gap-2">
            <Checkbox
              checked={selected}
              onCheckedChange={onToggleSelect}
              className="mt-1 shrink-0"
            />
            <div className="min-w-0 flex-1">
              <CardTitle className="text-lg flex flex-wrap items-center gap-2 leading-7">
                <span
                  className={credential.displayName || credential.email ? 'min-w-0 max-w-full truncate' : 'text-muted-foreground'}
                  title={credential.displayName || credential.email || `凭据 #${credential.id}`}
                >
                  {credential.displayName || credential.email || `凭据 #${credential.id}`}
                </span>
                <span className="text-xs text-muted-foreground font-normal shrink-0">#{credential.id}</span>
                {credential.isCurrent && (
                  <Badge variant="success">当前</Badge>
                )}
                {credential.disabled && (
                  <Badge variant="destructive">已禁用</Badge>
                )}
                {isCoolingDown && (
                  <Badge
                    variant="warning"
                    className="max-w-full gap-1 bg-amber-500 text-white border-transparent"
                    title={credential.cooldownUntil ? `冷却至 ${new Date(credential.cooldownUntil).toLocaleString()}` : undefined}
                  >
                    <Clock3 className="h-3 w-3" />
                    冷却中 {cooldownText}
                  </Badge>
                )}
                {latestRequestEvent && (
                  <Badge
                    variant="outline"
                    className="max-w-full truncate"
                    title={formatRequestEventTitle(latestRequestEvent)}
                  >
                    最近：{requestEventLabel(latestRequestEvent.kind)}
                  </Badge>
                )}
                {credential.disabled && credential.disabledReason && (
                  <Badge variant="outline" className="max-w-full truncate">{credential.disabledReason}</Badge>
                )}
                {credential.authMethod && (
                  <Badge variant="secondary">
                    {credential.authMethod === 'api_key' ? 'API Key' :
                     credential.authMethod === 'idc' ? 'IdC' :
                     credential.authMethod === 'social' ? 'Social' :
                     credential.authMethod}
                  </Badge>
                )}
                {credential.endpoint && (
                  <Badge variant="outline">{credential.endpoint}</Badge>
                )}
                {credential.allowOverage && (
                  <Badge variant="outline" className="text-purple-600 border-purple-300">超额</Badge>
                )}
              </CardTitle>
            </div>
          </div>
          <div className="flex items-center justify-between rounded-md border bg-muted/30 px-3 py-2">
            <span className="text-sm text-muted-foreground">
              {credential.disabled ? '当前已禁用' : isCoolingDown ? `风控冷却中，剩余 ${cooldownText}` : '当前已启用'}
            </span>
            <div className="flex items-center gap-2 shrink-0">
              <span className="text-sm font-medium">启用</span>
              <Switch
                checked={!credential.disabled}
                onCheckedChange={handleToggleDisabled}
                disabled={setDisabled.isPending}
              />
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="rounded-md border bg-muted/20 px-3 py-2 space-y-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-medium">最近 100 次请求状态</span>
              <span className="text-xs text-muted-foreground">{requestHistorySummary}</span>
            </div>
            {requestHistory.length > 0 ? (
              <div
                className="grid gap-0.5"
                style={{ gridTemplateColumns: 'repeat(50, minmax(0, 1fr))' }}
                aria-label="最近请求状态条"
              >
                {requestHistory.map((event, index) => (
                  <span
                    // eslint-disable-next-line react/no-array-index-key
                    key={`${event.at}-${index}`}
                    className={`h-3 min-w-[3px] rounded-sm transition-colors ${requestEventClass(event.kind)}`}
                    title={formatRequestEventTitle(event)}
                  />
                ))}
              </div>
            ) : (
              <div className="h-3 rounded-sm bg-muted" title="暂无请求记录" />
            )}
            <div className="flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
              <span><span className="inline-block h-2 w-2 rounded-sm bg-emerald-500" /> 成功</span>
              <span><span className="inline-block h-2 w-2 rounded-sm bg-red-500" /> 上游瞬态</span>
              <span><span className="inline-block h-2 w-2 rounded-sm bg-amber-500" /> 风控冷却</span>
              <span><span className="inline-block h-2 w-2 rounded-sm bg-rose-700" /> 硬失败</span>
              <span><span className="inline-block h-2 w-2 rounded-sm bg-purple-600" /> 额度</span>
            </div>
          </div>
          {/* 信息网格 */}
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="col-span-2 flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2">
              <Mail className="h-4 w-4 text-muted-foreground" />
              <span className="text-muted-foreground">邮箱 / 归属：</span>
              <span className={credential.email ? 'font-mono font-medium' : 'text-muted-foreground'}>
                {credentialOwner}
              </span>
              {!credential.email && (
                <span className="text-xs text-muted-foreground">
                  可通过 KAM 导入带 email 的凭据，或点击“刷新 Token”尝试自动识别
                </span>
              )}
            </div>
            <div className="col-span-2 flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-2">
              <Tag className="h-4 w-4 text-muted-foreground shrink-0" />
              <span className="text-muted-foreground shrink-0">名称：</span>
              {editingName ? (
                <div className="flex items-center gap-1 flex-1 min-w-0">
                  <Input
                    value={nameValue}
                    onChange={(e) => setNameValue(e.target.value)}
                    className="h-7 text-sm flex-1 min-w-0"
                    placeholder="自定义名称，便于区分"
                    maxLength={60}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleSaveName()
                      if (e.key === 'Escape') {
                        setEditingName(false)
                        setNameValue(credential.displayName ?? '')
                      }
                    }}
                  />
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 w-7 p-0 shrink-0"
                    onClick={handleSaveName}
                    disabled={setDisplayName.isPending}
                  >
                    ✓
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 w-7 p-0 shrink-0"
                    onClick={() => {
                      setEditingName(false)
                      setNameValue(credential.displayName ?? '')
                    }}
                  >
                    ✕
                  </Button>
                </div>
              ) : (
                <span
                  className="font-medium cursor-pointer hover:underline truncate"
                  onClick={() => {
                    setNameValue(credential.displayName ?? '')
                    setEditingName(true)
                  }}
                  title="点击编辑名称"
                >
                  {credential.displayName || '未命名'}
                  <span className="text-xs text-muted-foreground ml-1">(点击编辑)</span>
                </span>
              )}
            </div>
            <div>
              <span className="text-muted-foreground">优先级：</span>
              {editingPriority ? (
                <div className="inline-flex items-center gap-1 ml-1">
                  <Input
                    type="number"
                    value={priorityValue}
                    onChange={(e) => setPriorityValue(e.target.value)}
                    className="w-16 h-7 text-sm"
                    min="0"
                  />
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 w-7 p-0"
                    onClick={handlePriorityChange}
                    disabled={setPriority.isPending}
                  >
                    ✓
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 w-7 p-0"
                    onClick={() => {
                      setEditingPriority(false)
                      setPriorityValue(String(credential.priority))
                    }}
                  >
                    ✕
                  </Button>
                </div>
              ) : (
                <span
                  className="font-medium cursor-pointer hover:underline ml-1"
                  onClick={() => setEditingPriority(true)}
                >
                  {credential.priority}
                  <span className="text-xs text-muted-foreground ml-1">(点击编辑)</span>
                </span>
              )}
            </div>
            <div>
              <span className="text-muted-foreground">失败次数：</span>
              <span className={credential.failureCount > 0 ? 'text-red-500 font-medium' : ''}>
                {credential.failureCount}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">刷新失败：</span>
              <span className={credential.refreshFailureCount > 0 ? 'text-red-500 font-medium' : ''}>
                {credential.refreshFailureCount}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">订阅等级：</span>
              <span className="font-medium">
                {loadingBalance ? (
                  <Loader2 className="inline w-3 h-3 animate-spin" />
                ) : balance?.subscriptionTitle || '未知'}
              </span>
            </div>
            <div>
              <span className="text-muted-foreground">成功次数：</span>
              <span
                className="font-medium cursor-pointer hover:underline"
                onClick={handleResetSuccess}
                title="点击重置成功次数"
              >
                {credential.successCount}
                <span className="text-xs text-muted-foreground ml-1">(点击重置)</span>
              </span>
            </div>
            <div className="col-span-2">
              <span className="text-muted-foreground">最后调用：</span>
              <span className="font-medium">{formatLastUsed(credential.lastUsedAt)}</span>
            </div>
            {isCoolingDown && (
              <div className="col-span-2 flex items-center gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-amber-700 dark:text-amber-300">
                <Clock3 className="h-4 w-4 shrink-0" />
                <span className="text-muted-foreground">风控冷却：</span>
                <span className="font-medium">剩余 {cooldownText}</span>
                {credential.cooldownUntil && (
                  <span className="text-xs text-muted-foreground">至 {new Date(credential.cooldownUntil).toLocaleString()}</span>
                )}
              </div>
            )}
            {credential.maskedApiKey && (
              <div className="col-span-2">
                <span className="text-muted-foreground">API Key：</span>
                <span className="font-mono font-medium">{credential.maskedApiKey}</span>
              </div>
            )}
            <div className="col-span-2">
              <span className="text-muted-foreground">剩余用量：</span>
              {loadingBalance ? (
                <span className="text-sm ml-1">
                  <Loader2 className="inline w-3 h-3 animate-spin" /> 加载中...
                </span>
              ) : balance ? (
                <span className="font-medium ml-1">
                  {balance.remaining.toFixed(2)} / {balance.effectiveLimit.toFixed(2)}
                  {balance.allowOverage && (
                    <span className="text-xs text-purple-600 ml-1">
                      原限额 {balance.usageLimit.toFixed(2)}，超额 +{balance.overageAllowance.toFixed(0)}
                    </span>
                  )}
                  <span className="text-xs text-muted-foreground ml-1">
                    ({(100 - balance.usagePercentage).toFixed(1)}% 剩余)
                  </span>
                </span>
              ) : (
                <span className="text-sm text-muted-foreground ml-1">未知</span>
              )}
            </div>
            <div className="col-span-2 flex items-center gap-2">
              <span className="text-muted-foreground">超额模式：</span>
              <Switch
                checked={credential.allowOverage}
                onCheckedChange={handleToggleAllowOverage}
                disabled={setAllowOverage.isPending}
              />
              <span className="text-xs text-muted-foreground">开启后余额按 +10000 计算</span>
            </div>
            <div className="col-span-2">
              <span className="text-muted-foreground">RPM 限流：</span>
              <span className="font-medium">{formatRateLimits(credential.rateLimits)}</span>
              <button
                type="button"
                className="text-xs text-muted-foreground hover:underline ml-2"
                onClick={() => setShowRateLimitDialog(true)}
              >
                设置
              </button>
            </div>
            {credential.hasProxy && (
              <div className="col-span-2">
                <span className="text-muted-foreground">代理：</span>
                <span className="font-medium">{credential.proxyUrl}</span>
              </div>
            )}
            {credential.hasProfileArn && (
              <div className="col-span-2">
                <Badge variant="secondary">有 Profile ARN</Badge>
              </div>
            )}
          </div>

          {/* 操作按钮 */}
          <div className="grid grid-cols-2 gap-2 pt-3 border-t">
            <Button
              size="sm"
              variant="outline"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={handleReset}
              disabled={resetFailure.isPending || (credential.failureCount === 0 && credential.refreshFailureCount === 0)}
            >
              <RefreshCw className="h-4 w-4 mr-1 shrink-0" />
              重置失败
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={handleForceRefresh}
              disabled={forceRefresh.isPending || credential.disabled || credential.authMethod === 'api_key'}
              title={credential.authMethod === 'api_key' ? 'API Key 凭据无需刷新 Token' : credential.disabled ? '已禁用的凭据无法刷新 Token' : '强制刷新 Token'}
            >
              <RefreshCw className={`h-4 w-4 mr-1 shrink-0 ${forceRefresh.isPending ? 'animate-spin' : ''}`} />
              刷新 Token
            </Button>
            {isCoolingDown && (
              <Button
                size="sm"
                variant="outline"
                className="w-full justify-center whitespace-nowrap border-amber-500/60 px-2 text-xs text-amber-700 hover:bg-amber-500/10 dark:text-amber-300 sm:text-sm"
                onClick={handleClearCooldown}
                disabled={clearCooldown.isPending || credential.disabled}
                title={credential.disabled ? '已禁用的凭据无需退出冷却' : '手动清除运行时风控冷却，让该凭据立即重新参与调度'}
              >
                <Clock3 className="h-4 w-4 mr-1 shrink-0" />
                退出冷却
              </Button>
            )}
            <Button
              size="sm"
              variant="outline"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={() => {
                const newPriority = Math.max(0, credential.priority - 1)
                setPriority.mutate(
                  { id: credential.id, priority: newPriority },
                  {
                    onSuccess: (res) => toast.success(res.message),
                    onError: (err) => toast.error('操作失败: ' + (err as Error).message),
                  }
                )
              }}
              disabled={setPriority.isPending || credential.priority === 0}
            >
              <ChevronUp className="h-4 w-4 mr-1 shrink-0" />
              提高优先级
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={() => {
                const newPriority = credential.priority + 1
                setPriority.mutate(
                  { id: credential.id, priority: newPriority },
                  {
                    onSuccess: (res) => toast.success(res.message),
                    onError: (err) => toast.error('操作失败: ' + (err as Error).message),
                  }
                )
              }}
              disabled={setPriority.isPending}
            >
              <ChevronDown className="h-4 w-4 mr-1 shrink-0" />
              降低优先级
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={() => setShowRateLimitDialog(true)}
            >
              <Gauge className="h-4 w-4 mr-1 shrink-0" />
              RPM 限制
            </Button>
            <Button
              size="sm"
              variant="default"
              className="w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={() => onViewBalance(credential.id)}
            >
              <Wallet className="h-4 w-4 mr-1 shrink-0" />
              查看余额
            </Button>
            <Button
              size="sm"
              variant="destructive"
              className="col-span-2 w-full justify-center whitespace-nowrap px-2 text-xs sm:text-sm"
              onClick={() => setShowDeleteDialog(true)}
              disabled={!credential.disabled}
              title={!credential.disabled ? '需要先禁用凭据才能删除' : undefined}
            >
              <Trash2 className="h-4 w-4 mr-1 shrink-0" />
              删除
            </Button>
          </div>
        </CardContent>
      </Card>

      <RateLimitDialog
        credential={credential}
        open={showRateLimitDialog}
        onOpenChange={setShowRateLimitDialog}
      />

      {/* 删除确认对话框 */}
      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>确认删除凭据</DialogTitle>
            <DialogDescription>
              您确定要删除凭据 #{credential.id} 吗？此操作无法撤销。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setShowDeleteDialog(false)}
              disabled={deleteCredential.isPending}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleteCredential.isPending || !credential.disabled}
            >
              确认删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
