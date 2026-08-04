import { useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  useProxyPool,
  useAddProxy,
  useBatchAddProxy,
  useDeleteProxy,
  useSetProxyDisabled,
  useTestProxy,
  useUpdateProxyPoolSettings,
} from '@/hooks/use-proxy-pool'
import { extractErrorMessage } from '@/lib/utils'
import type { ProxyEntryView } from '@/types/api'

interface ProxyPoolDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ProxyPoolDialog({ open, onOpenChange }: ProxyPoolDialogProps) {
  const { data: pool, isLoading } = useProxyPool()
  const addProxy = useAddProxy()
  const batchAdd = useBatchAddProxy()
  const deleteProxy = useDeleteProxy()
  const setDisabled = useSetProxyDisabled()
  const test = useTestProxy()
  const updateSettings = useUpdateProxyPoolSettings()

  const [showBatch, setShowBatch] = useState(false)
  const [url, setUrl] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [label, setLabel] = useState('')
  const [batchText, setBatchText] = useState('')
  const [testingId, setTestingId] = useState<number | null>(null)

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault()
    if (!url.trim()) {
      toast.error('请输入代理 URL')
      return
    }
    addProxy.mutate(
      {
        url: url.trim(),
        username: username.trim() || undefined,
        password: password.trim() || undefined,
        label: label.trim() || undefined,
      },
      {
        onSuccess: () => {
          toast.success('代理已添加')
          setUrl('')
          setUsername('')
          setPassword('')
          setLabel('')
        },
        onError: (err) => toast.error(`添加失败: ${extractErrorMessage(err)}`),
      }
    )
  }

  const handleBatchAdd = () => {
    const lines = batchText
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
    if (lines.length === 0) {
      toast.error('请输入至少一行代理')
      return
    }
    batchAdd.mutate(lines, {
      onSuccess: (data) => {
        if (data.added > 0) {
          toast.success(`已添加 ${data.added} 个代理`)
        }
        if (data.errors && data.errors.length > 0) {
          const detail = data.errors
            .map((e) => `第${e.line}行: ${e.error}`)
            .join('\n')
          toast.error(`${data.errors.length} 行导入失败\n${detail}`, {
            duration: 8000,
          })
        }
        if (data.added > 0) {
          setBatchText('')
          if (!data.errors || data.errors.length === 0) setShowBatch(false)
        }
      },
      onError: (err) => toast.error(`批量添加失败: ${extractErrorMessage(err)}`),
    })
  }

  const handleTest = (id: number) => {
    setTestingId(id)
    test.mutate(id, {
      onSuccess: (r) => {
        if (r.success) toast.success(`#${id} ${r.message}${r.ip ? ` (出口 ${r.ip})` : ''}`)
        else toast.error(`#${id} ${r.message}`)
      },
      onError: (err) => toast.error(`探测失败: ${extractErrorMessage(err)}`),
      onSettled: () => setTestingId(null),
    })
  }

  const handleDelete = (p: ProxyEntryView) => {
    if (p.usageCount > 0) {
      toast.error('代理正在使用中，请先删除/改绑对应凭据')
      return
    }
    if (!confirm(`确认删除代理 #${p.id} ${p.label || p.url}？`)) return
    deleteProxy.mutate(p.id, {
      onSuccess: () => toast.success('代理已删除'),
      onError: (err) => toast.error(`删除失败: ${extractErrorMessage(err)}`),
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-3xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>IP 代理池管理</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2 overflow-y-auto flex-1 pr-1">
          {/* 统计 + 设置 */}
          <div className="flex flex-wrap items-center gap-3 text-sm">
            <span>总数 <b>{pool?.total ?? 0}</b></span>
            <span className="text-green-600">空闲 <b>{pool?.available ?? 0}</b></span>
            <span className="text-blue-600">在用 <b>{pool?.assigned ?? 0}</b></span>
            <span className="text-orange-600">共享 <b>{pool?.shared ?? 0}</b></span>
            {(pool?.disabled ?? 0) > 0 && (
              <span className="text-muted-foreground">禁用 <b>{pool?.disabled}</b></span>
            )}
            <label className="ml-auto flex items-center gap-2">
              <input
                type="checkbox"
                checked={pool?.autoAssignEnabled ?? true}
                onChange={(e) =>
                  updateSettings.mutate(
                    { autoAssignEnabled: e.target.checked },
                    {
                      onSuccess: () =>
                        toast.success(`自动分配已${e.target.checked ? '开启' : '关闭'}`),
                    }
                  )
                }
              />
              添加凭据时默认自动分配
            </label>
          </div>

          {/* 探测 URL 设置 */}
          <div className="flex items-center gap-2">
            <label className="text-xs text-muted-foreground whitespace-nowrap">探测 URL</label>
            <Input
              defaultValue={pool?.probeUrl ?? ''}
              placeholder="https://api.ipify.org?format=json"
              onBlur={(e) => {
                const v = e.target.value.trim()
                if (v && v !== pool?.probeUrl) {
                  updateSettings.mutate(
                    { probeUrl: v },
                    { onSuccess: () => toast.success('探测 URL 已更新') }
                  )
                }
              }}
            />
          </div>

          {/* 添加代理 */}
          <form onSubmit={handleAdd} className="space-y-2 rounded-md border p-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">添加代理</span>
              <button
                type="button"
                className="text-xs text-muted-foreground underline"
                onClick={() => setShowBatch((v) => !v)}
              >
                {showBatch ? '单个添加' : '批量导入'}
              </button>
            </div>

            {!showBatch ? (
              <>
                <Input
                  placeholder="http://ip:port 或 socks5://ip:port"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                />
                <div className="grid grid-cols-3 gap-2">
                  <Input placeholder="用户名（可选）" value={username} onChange={(e) => setUsername(e.target.value)} />
                  <Input placeholder="密码（可选）" value={password} onChange={(e) => setPassword(e.target.value)} />
                  <Input placeholder="备注（可选）" value={label} onChange={(e) => setLabel(e.target.value)} />
                </div>
                <Button type="submit" size="sm" disabled={addProxy.isPending}>
                  {addProxy.isPending ? '添加中...' : '添加'}
                </Button>
              </>
            ) : (
              <>
                <textarea
                  className="flex min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  placeholder="每行一个，支持两种格式：&#10;ip:端口:用户名:密码（无协议时按 socks5 处理）&#10;63.246.151.171:5502:user:pass&#10;或 url [用户名] [密码] [备注]&#10;http://1.2.3.4:6051 user pass 美国静态"
                  value={batchText}
                  onChange={(e) => setBatchText(e.target.value)}
                />
                <Button type="button" size="sm" onClick={handleBatchAdd} disabled={batchAdd.isPending}>
                  {batchAdd.isPending ? '导入中...' : '批量导入'}
                </Button>
              </>
            )}
          </form>

          {/* 代理列表 */}
          <div className="space-y-2">
            {isLoading && <p className="text-sm text-muted-foreground">加载中...</p>}
            {!isLoading && (pool?.proxies.length ?? 0) === 0 && (
              <p className="text-sm text-muted-foreground">代理池为空，先添加一个代理。</p>
            )}
            {pool?.proxies.map((p) => (
              <div
                key={p.id}
                className={`rounded-md border p-3 text-sm ${p.disabled ? 'opacity-50' : ''}`}
              >
                <div className="flex items-center gap-2">
                  <span className="font-medium">#{p.id}</span>
                  <span className="truncate">{p.label || p.url}</span>
                  {p.free ? (
                    <span className="rounded bg-green-100 px-1.5 py-0.5 text-xs text-green-700">空闲</span>
                  ) : (
                    <span className="rounded bg-blue-100 px-1.5 py-0.5 text-xs text-blue-700">
                      在用 {p.usageCount}
                    </span>
                  )}
                  {p.disabled && (
                    <span className="rounded bg-gray-200 px-1.5 py-0.5 text-xs">已禁用</span>
                  )}
                  {p.lastCheck && (
                    <span className={`text-xs ${p.lastCheck.ok ? 'text-green-600' : 'text-red-500'}`}>
                      {p.lastCheck.ok
                        ? `✓ ${p.lastCheck.latencyMs ?? '?'}ms${p.lastCheck.ip ? ` ${p.lastCheck.ip}` : ''}`
                        : `✗ ${p.lastCheck.message ?? '失败'}`}
                    </span>
                  )}
                </div>
                <div className="mt-1 flex items-center gap-3 text-xs text-muted-foreground">
                  <span className="truncate">{p.url}</span>
                  {p.username && <span>用户 {p.username}</span>}
                  {p.assignments.length > 0 && (
                    <span>凭据 [{p.assignments.join(', ')}]</span>
                  )}
                </div>
                <div className="mt-2 flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => handleTest(p.id)}
                    disabled={testingId === p.id}
                  >
                    {testingId === p.id ? '探测中...' : '测试'}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() =>
                      setDisabled.mutate(
                        { id: p.id, disabled: !p.disabled },
                        {
                          onSuccess: () =>
                            toast.success(p.disabled ? '已启用' : '已禁用'),
                        }
                      )
                    }
                  >
                    {p.disabled ? '启用' : '禁用'}
                  </Button>
                  {p.usageCount > 0 && (
                    <span
                      className="text-xs text-muted-foreground"
                      title="代理已写入对应凭据，要释放请删除或改绑该凭据，避免账实不一致"
                    >
                      在用凭据 [{p.assignments.join(', ')}]
                    </span>
                  )}
                  <Button
                    size="sm"
                    variant="destructive"
                    onClick={() => handleDelete(p)}
                    disabled={p.usageCount > 0}
                  >
                    删除
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
