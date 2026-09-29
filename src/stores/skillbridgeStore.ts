import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { IPC } from '@/lib/ipc/commands'
import type { SkillBridgeStatus } from '../lib/types'

interface SkillBridgeStore {
  status: SkillBridgeStatus
  version: string | null
  relayUrl: string | null
  mcpUrl: string | null
  checkStatus: () => Promise<void>
  onboardingDismissed: boolean
  dismissOnboarding: () => void
}

interface SkillBridgeCheckResult {
  status: SkillBridgeStatus
  version?: string
  relay_url?: string
  mcp_url?: string
}

export const useSkillbridgeStore = create<SkillBridgeStore>((set) => ({
  status: 'not-installed',
  version: null,
  relayUrl: null,
  mcpUrl: null,
  onboardingDismissed: false,

  checkStatus: async () => {
    try {
      const result = await invoke<SkillBridgeCheckResult>(IPC.checkSkillbridge)
      set({
        status: result.status,
        version: result.version ?? null,
        relayUrl: result.relay_url ?? null,
        mcpUrl: result.mcp_url ?? null,
      })
    } catch {
      set({ status: 'not-installed' })
    }
  },

  dismissOnboarding: () => {
    set({ onboardingDismissed: true })
  },
}))
