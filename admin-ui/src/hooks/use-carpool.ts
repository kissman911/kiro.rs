import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getCarpoolSettings, updateCarpoolSettings } from '@/api/credentials'
import type { UpdateCarpoolSettingsRequest } from '@/types/api'

const KEY = ['carpool-settings']

// 查询拼车补号配置
export function useCarpoolSettings() {
  return useQuery({
    queryKey: KEY,
    queryFn: getCarpoolSettings,
    refetchInterval: 30000,
  })
}

// 更新拼车补号配置
export function useUpdateCarpoolSettings() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (req: UpdateCarpoolSettingsRequest) => updateCarpoolSettings(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}
