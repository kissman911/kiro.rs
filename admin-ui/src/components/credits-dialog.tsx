import { useState } from 'react'
import { toast } from 'sonner'
import { Coins, RefreshCw, FlagTriangleRight, Loader2 } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  useCreditLedger,
  useSampleCredits,
  useStartCreditRound,
} from '@/hooks/use-credits'
import { extractErrorMessage } from '@/lib/utils'

interface CreditsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

function fmt(n: number): string {
  return n.toFixed(2)
}

function fmtTime(iso?: string): string {
  if (!iso) return '尚未采样'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}

export function CreditsDialog({ open, onOpenChange }: CreditsDialogProps) {
  const { data, isLoading } = useCreditLedger()
  const sample = useSampleCredits()
  const startRound = useStartCreditRound()
  const [note, setNote] = useState('')
  const [confirmingRound, setConfirmingRound] = useState(false)

  const handleSample = () => {
    sample.mutate(undefined, {
      onSuccess: (r) => {
        const msg = `采样 ${r.sampled} 个凭据，本次增量 我 ${fmt(r.myDelta)} / 他人 ${fmt(r.othersDelta)}`
        if (r.failed > 0) toast.warning(`${msg}（${r.failed} 个查询失败）`)
        else toast.success(msg)
      },
      onError: (err) => toast.error(`采样失败: ${extractErrorMessage(err)}`),
    })
  }

  const handleStartRound = () => {
    if (!confirmingRound) {
      setConfirmingRound(true)
      return
    }
    startRound.mutate(note.trim() || undefined, {
      onSuccess: (meta) => {
        toast.success(`已开启第 ${meta.id} 轮，旧轮已归档`)
        setNote('')
        setConfirmingRound(false)
      },
      onError: (err) => {
        toast.error(`开轮失败: ${extractErrorMessage(err)}`)
        setConfirmingRound(false)
      },
    })
  }

  const myShare =
    data && data.total > 0 ? (data.myTotal / data.total) * 100 : 0

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-3xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Coins className="h-5 w-5" />
            积分消耗统计
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2 overflow-y-auto flex-1 pr-1">
          {isLoading && (
            <p className="text-sm text-muted-foreground">加载中...</p>
          )}

          {data && (
            <>
              {/* 本轮汇总 */}
              <div className="rounded-lg border p-3 space-y-2">
                <div className="flex flex-wrap items-center gap-2 text-sm">
                  <Badge variant="outline">第 {data.currentRound.id} 轮</Badge>
                  <span className="text-muted-foreground text-xs">
                    开始于 {fmtTime(data.currentRound.startedAt)}
                  </span>
                  {data.currentRound.source && (
                    <span className="text-muted-foreground text-xs">
                      来源 {data.currentRound.source}
                    </span>
                  )}
                  {data.currentRound.note && (
                    <span className="text-xs">备注：{data.currentRound.note}</span>
                  )}
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-sm">
                  <div>
                    <div className="text-xs text-muted-foreground">我消耗</div>
                    <div className="text-lg font-semibold text-emerald-600 dark:text-emerald-400">
                      {fmt(data.myTotal)}
                    </div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground">他人消耗</div>
                    <div className="text-lg font-semibold text-amber-600 dark:text-amber-400">
                      {fmt(data.othersTotal)}
                    </div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground">本轮合计</div>
                    <div className="text-lg font-semibold">{fmt(data.total)}</div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground">我的占比</div>
                    <div className="text-lg font-semibold">
                      {myShare.toFixed(1)}%
                    </div>
                  </div>
                </div>

                <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                  <span>我的成功请求 {data.myRequests} 次</span>
                  <span>
                    凭据 {data.aliveCount} 活 / {data.deadCount} 死
                  </span>
                  <span>最后采样 {fmtTime(data.lastSampleAt)}</span>
                  <span>自动采样每 {data.sampleIntervalSeconds} 秒</span>
                </div>
              </div>

              {/* 操作区 */}
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleSample}
                  disabled={sample.isPending}
                >
                  {sample.isPending ? (
                    <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                  ) : (
                    <RefreshCw className="h-4 w-4 mr-1" />
                  )}
                  立即采样
                </Button>
                <Input
                  className="h-9 max-w-[220px]"
                  placeholder="新轮备注（可选）"
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
                <Button
                  variant={confirmingRound ? 'destructive' : 'outline'}
                  size="sm"
                  onClick={handleStartRound}
                  disabled={startRound.isPending}
                >
                  {startRound.isPending ? (
                    <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                  ) : (
                    <FlagTriangleRight className="h-4 w-4 mr-1" />
                  )}
                  {confirmingRound ? '确认开新一轮？' : '开新一轮'}
                </Button>
                {confirmingRound && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setConfirmingRound(false)}
                  >
                    取消
                  </Button>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                开新一轮会先补一次采样，把当前消耗记进旧轮再归档；存活凭据以当前用量为新基线继续计数，死号留在旧轮账上。
              </p>

              {/* 本轮明细 */}
              <div className="space-y-2">
                <div className="text-sm font-medium">本轮各凭据明细</div>
                {data.entries.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    还没有数据。首次采样只建基线，第二次采样起才会有消耗。
                  </p>
                ) : (
                  <div className="space-y-1.5">
                    {data.entries.map((e) => (
                      <div
                        key={e.fingerprint}
                        className="rounded-md border px-3 py-2 text-sm"
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="font-medium">#{e.credId}</span>
                          <span className="text-muted-foreground truncate max-w-[200px]">
                            {e.label}
                          </span>
                          {!e.alive && (
                            <Badge variant="destructive" className="text-[10px]">
                              死号 {e.deadReason ?? ''}
                            </Badge>
                          )}
                          {e.resets > 0 && (
                            <Badge variant="outline" className="text-[10px]">
                              重置 {e.resets} 次
                            </Badge>
                          )}
                          <span className="ml-auto font-semibold">
                            {fmt(e.myCredits + e.othersCredits)}
                          </span>
                        </div>
                        <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-muted-foreground">
                          <span className="text-emerald-600 dark:text-emerald-400">
                            我 {fmt(e.myCredits)}
                          </span>
                          <span className="text-amber-600 dark:text-amber-400">
                            他人 {fmt(e.othersCredits)}
                          </span>
                          <span>我的请求 {e.myRequests} 次</span>
                          <span>
                            用量 {fmt(e.lastUsage)} / {fmt(e.usageLimit)}
                          </span>
                          <span>采样 {e.samples} 次</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* 历史轮次 */}
              {data.archived.length > 0 && (
                <div className="space-y-2">
                  <div className="text-sm font-medium">历史轮次</div>
                  <div className="space-y-1">
                    {data.archived.map((r) => (
                      <div
                        key={r.id}
                        className="flex flex-wrap items-center gap-2 rounded-md border px-3 py-1.5 text-xs"
                      >
                        <Badge variant="outline">第 {r.id} 轮</Badge>
                        <span className="text-muted-foreground">
                          {fmtTime(r.startedAt)} → {fmtTime(r.endedAt)}
                        </span>
                        {r.note && <span>{r.note}</span>}
                        <span className="ml-auto">
                          我 {fmt(r.myTotal)} / 他人 {fmt(r.othersTotal)} / 合计{' '}
                          {fmt(r.total)}（{r.credentialCount} 个凭据）
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        <div className="flex justify-end pt-2 border-t">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
