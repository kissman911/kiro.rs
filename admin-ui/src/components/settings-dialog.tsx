import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Settings2 } from 'lucide-react'
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
import { Switch } from '@/components/ui/switch'
import { useRuntimeSettings, useUpdateRuntimeSettings } from '@/hooks/use-credentials'
import { extractErrorMessage } from '@/lib/utils'

interface SettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { data, isLoading, refetch } = useRuntimeSettings()
  const { mutateAsync: updateSettings, isPending } = useUpdateRuntimeSettings()

  const [cooldownMinutes, setCooldownMinutes] = useState('')
  const [extractThinking, setExtractThinking] = useState(true)
  const [twoPhaseFlow, setTwoPhaseFlow] = useState(false)

  // 对话框打开或数据变化时同步表单
  useEffect(() => {
    if (open) {
      refetch()
    }
  }, [open, refetch])

  useEffect(() => {
    if (data) {
      // 冷却时长以分钟展示，去掉多余小数
      setCooldownMinutes(String(Number(data.suspiciousCooldownMinutes.toFixed(2))))
      setExtractThinking(data.extractThinking)
      setTwoPhaseFlow(data.nativeLikeTwoPhaseFlow)
    }
  }, [data])

  // 校验冷却输入：空、非数字、负数、超上限都视为非法
  const trimmedMinutes = cooldownMinutes.trim()
  const parsedMinutes = Number(trimmedMinutes)
  const cooldownError =
    trimmedMinutes === ''
      ? '请输入冷却时间'
      : !Number.isFinite(parsedMinutes) || parsedMinutes < 0
        ? '必须是非负数字'
        : parsedMinutes > 1440
          ? '不能超过 1440 分钟（24 小时）'
          : null

  const handleSave = async () => {
    if (cooldownError) {
      toast.error(`冷却时间无效：${cooldownError}`)
      return
    }
    const minutes = parsedMinutes

    try {
      await updateSettings({
        suspiciousCooldownMinutes: minutes,
        extractThinking,
        nativeLikeTwoPhaseFlow: twoPhaseFlow,
      })
      toast.success('设置已保存并生效')
      onOpenChange(false)
    } catch (error) {
      toast.error(`保存失败: ${extractErrorMessage(error)}`)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Settings2 className="h-5 w-5" />
            运行时设置
          </DialogTitle>
          <DialogDescription>
            这些设置会立即生效并写回 config.json，无需重启服务。
          </DialogDescription>
        </DialogHeader>

        {isLoading ? (
          <div className="py-8 text-center text-muted-foreground">加载中...</div>
        ) : (
          <div className="space-y-5 py-2">
            {/* 风控冷却时间 */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-sm font-medium" htmlFor="cooldown-minutes">
                  风控冷却时间（分钟）
                </label>
                <div className="flex items-center gap-2">
                  <Input
                    id="cooldown-minutes"
                    type="number"
                    min="0"
                    max="1440"
                    step="0.5"
                    value={cooldownMinutes}
                    onChange={(e) => setCooldownMinutes(e.target.value)}
                    className={`w-24 text-right${cooldownError ? ' border-destructive focus-visible:ring-destructive' : ''}`}
                  />
                  <span className="text-sm text-muted-foreground">分钟</span>
                </div>
              </div>
              {cooldownError ? (
                <p className="text-xs text-destructive">{cooldownError}</p>
              ) : (
                <p className="text-xs text-muted-foreground">
                  凭据触发 Kiro 上游 suspicious activity 429 后的冷却时长。设为 0 使用默认 10 分钟。
                </p>
              )}
            </div>

            {/* thinking 块提取 */}
            <div className="flex items-center justify-between rounded-md border bg-muted/30 px-3 py-2">
              <div className="pr-4">
                <div className="text-sm font-medium">提取 thinking 块</div>
                <p className="text-xs text-muted-foreground">
                  非流式响应中解析 &lt;thinking&gt; 标签为独立内容块。
                </p>
              </div>
              <Switch checked={extractThinking} onCheckedChange={setExtractThinking} />
            </div>

            {/* 双阶段执行（实验） */}
            <div className="flex items-center justify-between rounded-md border bg-muted/30 px-3 py-2">
              <div className="pr-4">
                <div className="text-sm font-medium">
                  原生化双阶段执行
                  <span className="ml-2 text-xs text-amber-600">实验</span>
                </div>
                <p className="text-xs text-muted-foreground">
                  启用后按原生化双阶段流程执行请求。不确定请保持关闭。
                </p>
              </div>
              <Switch checked={twoPhaseFlow} onCheckedChange={setTwoPhaseFlow} />
            </div>
          </div>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={isPending}>
            取消
          </Button>
          <Button onClick={handleSave} disabled={isPending || isLoading || !!cooldownError}>
            {isPending ? '保存中...' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
