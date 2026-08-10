import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getCreditLedger,
  sampleCredits,
  startCreditRound,
  resetCreditLedger,
} from '@/api/credentials'

const KEY = ['credit-ledger']

// 查询积分账本（本轮明细 + 汇总 + 历史轮）
export function useCreditLedger() {
  return useQuery({
    queryKey: KEY,
    queryFn: getCreditLedger,
    refetchInterval: 60000,
  })
}

// 立即采样一次用量（面板手动刷新）
export function useSampleCredits() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: sampleCredits,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEY })
      qc.invalidateQueries({ queryKey: ['credentials'] })
    },
  })
}

// 开启新一轮车队统计
export function useStartCreditRound() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (note?: string) => startCreditRound(note),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

// 清空积分账本（含历史轮次）
export function useResetCreditLedger() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: resetCreditLedger,
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}
