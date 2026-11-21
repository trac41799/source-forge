import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { IPC } from '../lib/ipc/commands'
import type { AgentConfig, AgentStatus, AgentInstallStatus } from '../lib/types'
import { AgentNotInstalledError } from '../lib/types'

interface AgentSession {
  config: AgentConfig
  sessionId: string
  status: AgentStatus
  output: string[]
  startedAt: Date
  projectPath: string
}

interface AgentStore {
  agents: Map<string, AgentSession>
  spawnAgent: (config: AgentConfig, projectPath: string) => Promise<void>
  killAgent: (agentId: string) => Promise<void>
  writeToAgent: (agentId: string, text: string) => Promise<void>
  updateStatus: (agentId: string, status: AgentStatus) => void
  appendOutput: (agentId: string, text: string) => void
  checkAgentInstalled: (agentId: string, command: string) => Promise<AgentInstallStatus>
  streamOutput: (agentId: string) => void
}

const MAX_OUTPUT_LINES = 1000

export const useAgentStore = create<AgentStore>((set) => ({
  agents: new Map(),

  checkAgentInstalled: async (agentId: string, command: string) => {
    const status: AgentInstallStatus = await invoke('check_agent_installed', {
      agentId,
      command,
    })
    if (!status.installed) {
      throw new AgentNotInstalledError(command, agentId)
    }
    return status
  },

  spawnAgent: async (config: AgentConfig, projectPath: string) => {
    const sessionId = crypto.randomUUID()
    const newSession: AgentSession = {
      config,
      sessionId,
      status: 'idle',
      output: [],
      startedAt: new Date(),
      projectPath,
    }

    set((state) => {
      const newAgents = new Map(state.agents)
      newAgents.set(config.id, newSession)
      return { agents: newAgents }
    })

    try {
      await invoke('check_agent_installed', {
        agentId: config.id,
        command: config.spawnCmd,
      })

      await invoke(IPC.spawnAgent, {
        agentId: config.id,
        projectPath,
        command: config.spawnCmd,
        args: config.defaultArgs,
        envVars: {},
      })
    } catch (error) {
      set((state) => {
        const newAgents = new Map(state.agents)
        newAgents.delete(config.id)
        return { agents: newAgents }
      })
      throw error
    }
  },

  killAgent: async (agentId: string) => {
    await invoke(IPC.killAgent, { agentId })
    set((state) => {
      const newAgents = new Map(state.agents)
      newAgents.delete(agentId)
      return { agents: newAgents }
    })
  },

  writeToAgent: async (agentId: string, text: string) => {
    await invoke(IPC.writeToAgent, { agentId, text })
  },

  updateStatus: (agentId: string, status: AgentStatus) => {
    set((state) => {
      const newAgents = new Map(state.agents)
      const session = newAgents.get(agentId)
      if (session) {
        newAgents.set(agentId, { ...session, status })
      }
      return { agents: newAgents }
    })
  },

  appendOutput: (agentId: string, text: string) => {
    set((state) => {
      const newAgents = new Map(state.agents)
      const session = newAgents.get(agentId)
      if (session) {
        const newOutput = [...session.output, text]
        if (newOutput.length > MAX_OUTPUT_LINES) {
          newOutput.shift()
        }
        newAgents.set(agentId, { ...session, output: newOutput })
      }
      return { agents: newAgents }
    })
  },

  streamOutput: (agentId: string) => {
    // TODO: Call Tauri command to start streaming
    // For now, just log that streaming was requested
    console.log(`Starting output stream for agent: ${agentId}`)
  },
}))