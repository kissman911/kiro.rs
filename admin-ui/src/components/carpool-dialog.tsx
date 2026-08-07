import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useCarpoolSettings, useUpdateCarpoolSettings } from '@/hooks/use-carpool'
import { extractErrorMessage } from '@/lib/utils'

interface CarpoolDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function CarpoolDialog({ open, onOpenChange }: CarpoolDialogProps) {
  const { data: cfg, isLoading } = useCarpoolSettings()
  const update = useUpdateCarpoolSettings()

  // 本地编辑态（避免每次输入都打请求；保存时统一提交）
  const [getUrl, setGetUrl] = useState('')
  const [targetActive, setTargetActive] = useState('3')
  const [pollInterval, setPollInterval] = useState('30')
  const [recentWindow, setRecentWindow] = useState('20')
  const [minSample, setMinSample] = useState('10')
  const [disableErrRatio, setDisableErrRatio] = useState('0.4')
  const [healthyErrRatio, setHealthyErrRatio] = useState('0.2')

  // 配置加载后同步到本地编辑态
  useEffect(() => {
    if (cfg) {
      setGetUrl(cfg.getUrl)
      setTargetActive(String(cfg.targetActive))
      setPollInterval(String(cfg.pollInterval))
      setRecentWindow(String(cfg.recentWindow))
      setMinSample(String(cfg.minSample))
      setDisableErrRatio(String(cfg.disableErrRatio))
      setHealthyErrRatio(String(cfg.healthyErrRatio))
    }
  }, [cfg])

  const toggle = (field: 'enabled' | 'dryRun', value: boolean, label: string) => {
    update.mutate(
      { [field]: value },
      {
        onSuccess: () => toast.success(`${label}已${value ? '开启' : '关闭'}`),
        onError: (err) => toast.error(`更新失败: ${extractErrorMessage(err)}`),
      }
    )
  }

  const handleSave = () => {
    const url = getUrl.trim()
    if (url && !/^https?:\/\//.test(url)) {
      toast.error('接口 URL 必须以 http:// 或 https:// 开头')
      return
    }
    const ta = parseInt(targetActive, 10)
    const pi = parseInt(pollInterval, 10)
    const rw = parseInt(recentWindow, 10)
    const ms = parseInt(minSample, 10)
    const der = parseFloat(disableErrRatio)
    const her = parseFloat(healthyErrRatio)
    if (Number.isNaN(ta) || ta < 0) return toast.error('目标活号数非法')
    if (Number.isNaN(pi) || pi < 5) return toast.error('轮询间隔最小 5 秒')
    if (Number.isNaN(rw) || rw < 1) return toast.error('健康度窗口非法')
    if (Number.isNaN(ms) || ms < 0) return toast.error('最小样本数非法')
    if (Number.isNaN(der) || der < 0 || der > 1) return toast.error('禁用错误率需在 0~1')
    if (Number.isNaN(her) || her < 0 || her > 1) return toast.error('健康错误率需在 0~1')

    update.mutate(
      {
        getUrl: url,
        targetActive: ta,
        pollInterval: pi,
        recentWindow: rw,
        minSample: ms,
        disableErrRatio: der,
        healthyErrRatio: her,
      },
      {
        onSuccess: () => toast.success('拼车配置已保存，daemon 下轮生效'),
        onError: (err) => toast.error(`保存失败: ${extractErrorMessage(err)}`),
      }
    )
  }

  const tokenTail = (() => {
    const m = cfg?.getUrl?.match(/token=([^&]+)/)
    if (!m) return null
    const t = m[1]
    return t.length > 8 ? `…${t.slice(-8)}` : t
  })()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>拼车补号配置</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2 overflow-y-auto flex-1 pr-1">
          {isLoading && <p className="text-sm text-muted-foreground">加载中...</p>}

          {/* 开关区 */}
          <div className="flex flex-wrap items-center gap-4 text-sm">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={cfg?.enabled ?? false}
                onChange={(e) => toggle('enabled', e.target.checked, '自动补号')}
              />
              <span className="font-medium">启用自动补号</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={cfg?.dryRun ?? false}
                onChange={(e) => toggle('dryRun', e.target.checked, '演练模式')}
              />
              演练模式（只打印不改动）
            </label>
            {tokenTail && (
              <span className="ml-auto text-xs text-muted-foreground">
                当前 token {tokenTail}
              </span>
            )}
          </div>

          {/* 接口 URL */}
          <div className="space-y-1">
            <label className="text-xs text-muted-foreground">自动提 JSON 接口 URL（含 token）</label>
            <Input
              placeholder="https://car.mercarimx.com/get?token=..."
              value={getUrl}
              onChange={(e) => setGetUrl(e.target.value)}
            />
          </div>

          {/* 数值参数 */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">目标活号数</label>
              <Input value={targetActive} onChange={(e) => setTargetActive(e.target.value)} />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">轮询间隔（秒，≥5）</label>
              <Input value={pollInterval} onChange={(e) => setPollInterval(e.target.value)} />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">健康度窗口（最近 N 次）</label>
              <Input value={recentWindow} onChange={(e) => setRecentWindow(e.target.value)} />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">最小样本数</label>
              <Input value={minSample} onChange={(e) => setMinSample(e.target.value)} />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">禁用错误率阈值（0~1）</label>
              <Input value={disableErrRatio} onChange={(e) => setDisableErrRatio(e.target.value)} />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-muted-foreground">健康错误率阈值（0~1）</label>
              <Input value={healthyErrRatio} onChange={(e) => setHealthyErrRatio(e.target.value)} />
            </div>
          </div>

          <p className="text-xs text-muted-foreground">
            改动保存后由外部 feeder daemon 下一轮轮询拉取生效（约 {cfg?.pollInterval ?? 30} 秒内），无需重启。
          </p>
        </div>

        <div className="flex justify-end gap-2 pt-2 border-t">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
          <Button onClick={handleSave} disabled={update.isPending}>
            {update.isPending ? '保存中...' : '保存配置'}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
