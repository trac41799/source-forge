import { useState, useEffect } from 'react'
import { useAgentStore } from '../../stores/agentStore'
import { useProjectStore } from '../../stores/projectStore'
import type { AgentConfig } from '../../lib/types'
import { AgentNotInstalledError } from '../../lib/types'
import PtyTerminal from '../terminal/PtyTerminal'
import StatusChip, { inferStatus } from './StatusChip'

interface AgentPanelProps {
  config: AgentConfig
  isOrchestrator?: boolean
  waveEligible?: boolean
}

function formatElapsed(startedAt: Date): string {
  const diff = Date.now() - startedAt.getTime()
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  
  if (hours > 0) return `${hours}h ${minutes % 60}m`
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`
  return `${seconds}s`
}

export default function AgentPanel({ config, isOrchestrator = false, waveEligible = false }: AgentPanelProps) {
  const { agents, spawnAgent, killAgent, updateStatus, appendOutput } = useAgentStore()
  const session = agents.get(config.id)
  const [elapsed, setElapsed] = useState<string>('')
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  
  useEffect(() => {
    if (!session) {
      setElapsed('')
      setErrorMessage(null)
      return
    }
    
    const update = () => setElapsed(formatElapsed(session.startedAt))
    update()
    const interval = setInterval(update, 1000)
    return () => clearInterval(interval)
  }, [session?.startedAt])
  
  const handleSpawn = async () => {
    setErrorMessage(null)
    try {
      const projectPath = useProjectStore.getState().currentProject?.path || '.'
      await spawnAgent(config, projectPath)
    } catch (e) {
      if (e instanceof AgentNotInstalledError) {
        setErrorMessage(e.message)
      } else {
        setErrorMessage(`Failed to spawn ${config.label}: ${String(e)}`)
      }
      console.error('Failed to spawn agent:', e)
    }
  }
  
  const handleKill = async () => {
    try {
      await killAgent(config.id)
    } catch (e) {
      console.error('Failed to kill agent:', e)
    }
  }
  
  const handleOutput = (text: string) => {
    appendOutput(config.id, text)
    const inferred = inferStatus([text])
    updateStatus(config.id, inferred)
  }
  
  const isActive = session?.status === 'thinking' || session?.status === 'writing' || session?.status === 'running tests'
  const panelClass = `flex flex-col h-full ${isOrchestrator ? 'min-h-[400px]' : 'min-h-[300px]'} glass-card overflow-hidden ${isActive ? 'animate-border-glow' : ''}`
  const sectionClass = `flex items-center justify-between px-3 py-2 border-b border-glass-border`
  const footerClass = `flex items-center gap-2 px-3 py-1.5 border-t border-glass-border`
  
  if (!session) {
    return (
      <div className={panelClass}>
        <div className={sectionClass}>
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-foreground">{config.label}</span>
            {isOrchestrator && (
              <span className="text-xs px-1.5 py-0.5 rounded-md bg-indigo-500/15 text-indigo-300 border border-indigo-500/20 font-medium">
                Orchestrator
              </span>
            )}
            {!isOrchestrator && waveEligible && (
              <span className="text-xs px-1.5 py-0.5 rounded-md bg-cyan-500/10 text-cyan-300 border border-cyan-500/15 font-medium">
                Wave
              </span>
            )}
          </div>
          <button
            onClick={handleSpawn}
            className="gradient-primary px-3 py-1 text-xs text-white rounded-lg shadow-glow-sm hover:shadow-glow-lg hover:-translate-y-px active:scale-[0.98] transition-all duration-200"
          >
            Spawn
          </button>
        </div>
        <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm gap-2">
          {errorMessage ? (
            <div className="px-4 py-3 mx-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-300 text-xs max-w-[90%] text-center">
              <div className="font-medium mb-1 text-red-400">Failed to spawn</div>
              <div className="whitespace-pre-wrap break-words">{errorMessage}</div>
            </div>
          ) : (
            <span>Agent not spawned</span>
          )}
        </div>
      </div>
    )
  }
  
  return (
    <div className={panelClass}>
      <div className={sectionClass}>
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground">{config.label}</span>
          {isOrchestrator && (
            <span className="text-xs px-1.5 py-0.5 rounded-md bg-indigo-500/15 text-indigo-300 border border-indigo-500/20 font-medium">
              Orchestrator
            </span>
          )}
          {!isOrchestrator && waveEligible && (
            <span className="text-xs px-1.5 py-0.5 rounded-md bg-cyan-500/10 text-cyan-300 border border-cyan-500/15 font-medium">
              Wave
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <StatusChip status={session.status} />
          {elapsed && <span className="text-xs text-muted-foreground">{elapsed}</span>}
        </div>
      </div>
      
      <div className="flex-1 min-h-[200px]">
        <PtyTerminal agentId={config.id} onOutput={handleOutput} />
      </div>
      
      <div className={footerClass}>
        <button
          onClick={handleKill}
          className="px-2 py-1 text-xs bg-red-500/15 text-red-400 border border-red-500/20 rounded-md hover:bg-red-500/25 transition-colors"
        >
          Kill
        </button>
        <button
          onClick={handleSpawn}
          className="gradient-primary px-2 py-1 text-xs text-white rounded-md shadow-glow-sm hover:shadow-glow-lg hover:-translate-y-px transition-all duration-200"
        >
          Restart
        </button>
        <button
          onClick={() => useAgentStore.getState().writeToAgent(config.id, '\x03')}
          className="px-2 py-1 text-xs bg-glass-20 text-muted-foreground border border-glass-border rounded-md glass-hover"
        >
          Clear
        </button>
      </div>
    </div>
  )
}
