import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getProxyPool,
  addProxy,
  batchAddProxy,
  updateProxy,
  deleteProxy,
  setProxyDisabled,
  testProxy,
  updateProxyPoolSettings,
} from '@/api/credentials'
import type {
  AddProxyRequest,
  UpdateProxyRequest,
  UpdateProxyPoolSettingsRequest,
} from '@/types/api'

const KEY = ['proxy-pool']

// 查询代理池
export function useProxyPool() {
  return useQuery({
    queryKey: KEY,
    queryFn: getProxyPool,
    refetchInterval: 30000,
  })
}

// 添加代理
export function useAddProxy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (req: AddProxyRequest) => addProxy(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 批量添加代理
export function useBatchAddProxy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (lines: string[]) => batchAddProxy(lines),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 更新代理
export function useUpdateProxy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, req }: { id: number; req: UpdateProxyRequest }) =>
      updateProxy(id, req),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 删除代理
export function useDeleteProxy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => deleteProxy(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 启用/禁用代理
export function useSetProxyDisabled() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, disabled }: { id: number; disabled: boolean }) =>
      setProxyDisabled(id, disabled),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 探测代理
export function useTestProxy() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => testProxy(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 更新代理池设置
export function useUpdateProxyPoolSettings() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (req: UpdateProxyPoolSettingsRequest) =>
      updateProxyPoolSettings(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}
