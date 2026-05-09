import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Gauge, Plus, Trash2 } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { useSetRateLimits } from '@/hooks/use-credentials'
import type { CredentialStatusItem, RateLimitRule } from '@/types/api'
import { extractErrorMessage } from '@/lib/utils'

interface RateLimitDialogProps {
  credential: CredentialStatusItem
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface EditableRateLimitRule {
  window: string
  maxRequests: string
}

function toEditableRules(rules?: RateLimitRule[]): EditableRateLimitRule[] {
  if (!rules || rules.length === 0) {
    return [{ window: '1m', maxRequests: '' }]
  }
  return rules.map((rule) => ({
    window: rule.window,
    maxRequests: String(rule.maxRequests),
  }))
}

function parseRules(rules: EditableRateLimitRule[]): RateLimitRule[] | null {
  const parsed: RateLimitRule[] = []
  const seenWindows = new Set<string>()

  for (const rule of rules) {
    const window = rule.window.trim()
    const maxRequestsRaw = rule.maxRequests.trim()

    // 空行忽略，方便用户删到一半不报错
    if (!window && !maxRequestsRaw) continue

    if (!/^\d+[smhd]$/.test(window)) {
      throw new Error('时间窗口格式必须类似 30s / 1m / 5m / 1h / 1d')
    }

    const amount = Number(maxRequestsRaw)
    if (!Number.isInteger(amount) || amount <= 0) {
      throw new Error('请求数必须是大于 0 的整数')
    }

    if (seenWindows.has(window)) {
      throw new Error(`时间窗口重复：${window}`)
    }
    seenWindows.add(window)

    parsed.push({ window, maxRequests: amount })
  }

  return parsed.length > 0 ? parsed : null
}

function describeRules(rules?: RateLimitRule[]): string {
  if (!rules || rules.length === 0) return '未设置，使用全局默认'
  return rules.map((rule) => `${rule.window} / ${rule.maxRequests} 次`).join('，')
}

export function RateLimitDialog({ credential, open, onOpenChange }: RateLimitDialogProps) {
  const [rules, setRules] = useState<EditableRateLimitRule[]>(() => toEditableRules(credential.rateLimits))
  const setRateLimits = useSetRateLimits()

  useEffect(() => {
    if (open) {
      setRules(toEditableRules(credential.rateLimits))
    }
  }, [open, credential.id, credential.rateLimits])

  const preview = useMemo(() => {
    try {
      const parsed = parseRules(rules)
      return describeRules(parsed || undefined)
    } catch {
      return '等待修正格式'
    }
  }, [rules])

  const updateRule = (index: number, patch: Partial<EditableRateLimitRule>) => {
    setRules((current) => current.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)))
  }

  const addRule = () => {
    setRules((current) => [...current, { window: '1m', maxRequests: '' }])
  }

  const removeRule = (index: number) => {
    setRules((current) => {
      const next = current.filter((_, i) => i !== index)
      return next.length > 0 ? next : [{ window: '1m', maxRequests: '' }]
    })
  }

  const handleSave = () => {
    let parsed: RateLimitRule[] | null
    try {
      parsed = parseRules(rules)
    } catch (error) {
      toast.error((error as Error).message)
      return
    }

    setRateLimits.mutate(
      { id: credential.id, rateLimits: parsed },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          onOpenChange(false)
        },
        onError: (error: unknown) => {
          toast.error(`保存失败: ${extractErrorMessage(error)}`)
        },
      }
    )
  }

  const handleClear = () => {
    setRateLimits.mutate(
      { id: credential.id, rateLimits: null },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          onOpenChange(false)
        },
        onError: (error: unknown) => {
          toast.error(`清空失败: ${extractErrorMessage(error)}`)
        },
      }
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Gauge className="h-5 w-5" />
            RPM / 限流设置
          </DialogTitle>
          <DialogDescription>
            为凭据 #{credential.id} 设置独立限流。留空或清空后会回退到 config.json 的全局 defaultRateLimits。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="rounded-lg border bg-muted/30 p-3 text-sm">
            <div className="text-muted-foreground mb-1">当前生效的凭据级规则</div>
            <div className="font-medium">{describeRules(credential.rateLimits)}</div>
          </div>

          <div className="space-y-3">
            <div className="grid grid-cols-[1fr_1fr_40px] gap-2 text-xs text-muted-foreground px-1">
              <span>窗口</span>
              <span>最大请求数</span>
              <span />
            </div>
            {rules.map((rule, index) => (
              <div key={index} className="grid grid-cols-[1fr_1fr_40px] gap-2 items-center">
                <Input
                  value={rule.window}
                  onChange={(e) => updateRule(index, { window: e.target.value })}
                  placeholder="1m"
                  disabled={setRateLimits.isPending}
                />
                <Input
                  type="number"
                  min="1"
                  value={rule.maxRequests}
                  onChange={(e) => updateRule(index, { maxRequests: e.target.value })}
                  placeholder="5"
                  disabled={setRateLimits.isPending}
                />
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-9 w-9 p-0 text-muted-foreground hover:text-destructive"
                  onClick={() => removeRule(index)}
                  disabled={setRateLimits.isPending}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            ))}
          </div>

          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={addRule}
            disabled={setRateLimits.isPending}
          >
            <Plus className="h-4 w-4 mr-1" />
            增加窗口
          </Button>

          <div className="flex flex-wrap items-center gap-2 text-sm">
            <span className="text-muted-foreground">预览：</span>
            <Badge variant="secondary">{preview}</Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            支持单位：s 秒、m 分钟、h 小时、d 天。常用写法：1m / 5 表示每分钟最多 5 次请求。
          </p>
        </div>

        <DialogFooter className="gap-2 sm:gap-0">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={setRateLimits.isPending}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={handleClear}
            disabled={setRateLimits.isPending}
          >
            清空限流
          </Button>
          <Button type="button" onClick={handleSave} disabled={setRateLimits.isPending}>
            {setRateLimits.isPending ? '保存中...' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
