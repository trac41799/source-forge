import { useState, useMemo, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAgentStore } from '../stores/agentStore'
import { usePresetStore } from '../stores/presetStore'
import { useProjectStore } from '../stores/projectStore'
import { useAssetStore } from '../stores/assetStore'
import { useCodebaseStore } from '../stores/codebaseStore'
import { useControlStore, type ControlState } from '../stores/controlStore'
import AgentGrid from '../components/runner/AgentGrid'
import { OrchestratorToggle } from '../components/runner/OrchestratorToggle'
import { AGENT_CONFIGS } from '../lib/agents/configs'
import type { AgentConfig } from '../lib/types'
import { AlertTriangle, CheckCircle2, Pause, Play, Wand2, XCircle, Sliders } from 'lucide-react'

export default function Runner() {
  const navigate = useNavigate()
  const { agents, spawnAgent } = useAgentStore()
  const { presets, addPreset, executePreset } = usePresetStore()
  const { currentProject, switchProject, recentPaths } = useProjectStore()
  const generateProfile = useAssetStore((s) => s.generateProfile)
  const buildRepoMap = useCodebaseStore((s) => s.buildRepoMap)
  const controlSessions = useControlStore((s) => s.sessions)
  const controlConflicts = useControlStore((s) => s.conflicts)
  const controlModeActive = useControlStore((s) => s.controlModeActive)
  const controlDeferredNotice = useControlStore((s) => s.deferredNotice)
  const setControlMode = useControlStore((s) => s.setControlMode)
  const loadControlSessions = useControlStore((s) => s.loadSessions)
  const promoteToControl = useControlStore((s) => s.promoteToControl)
  const setControlState = useControlStore((s) => s.setState)
  const loadConflicts = useControlStore((s) => s.loadConflicts)
  const clearControlNotice = useControlStore((s) => s.clearNotice)
  const [showAgentDropdown, setShowAgentDropdown] = useState(false)
  const [orchestratorMode, setOrchestratorMode] = useState(false)
  const [orchestratorAgent, setOrchestratorAgent] = useState<string | null>(null)
  const [showNewPreset, setShowNewPreset] = useState(false)
  const [newPresetLabel, setNewPresetLabel] = useState('')
  const [newPresetCommand, setNewPresetCommand] = useState('')

  const controlThreadId = currentProject?.id || currentProject?.path || 'default-thread'

  useEffect(() => {
    if (controlModeActive) {
      loadControlSessions(controlThreadId)
      loadConflicts(controlThreadId)
    }
  }, [controlModeActive, controlThreadId, loadControlSessions, loadConflicts])

  const handlePromote = async (panelId: string) => {
    await promoteToControl(controlThreadId, panelId)
  }

  const handleSetControlState = async (sessionId: string, state: ControlState) => {
    await setControlState(sessionId, state)
  }

  const handleRefreshControl = () => {
    loadControlSessions(controlThreadId)
    loadConflicts(controlThreadId)
  }

  const handleToggleControlMode = () => {
    setControlMode(!controlModeActive)
    if (controlModeActive) {
      clearControlNotice()
    }
  }

  const agentLabels = useMemo(
    () =>
      AGENT_CONFIGS.map((a) => ({ id: a.id, label: a.label })),
    []
  )

  const agentCount = agents.size
  const totalFilesChanged = 0
  
  const handleSpawnAgent = async (agent: AgentConfig) => {
    try {
      const projectPath = currentProject?.path || '.'
      await spawnAgent(agent, projectPath)
      setShowAgentDropdown(false)
    } catch (e) {
      console.error('Failed to spawn agent:', e)
    }
  }

  const handleLoadProfile = async () => {
    const projectPath = currentProject?.path || '.'
    await generateProfile(projectPath)
  }

  const handleCreatePreset = () => {
    const label = newPresetLabel.trim()
    const command = newPresetCommand.trim()
    if (!label || !command) return
    addPreset({
      label,
      agent_id: '',
      command,
      tags: [],
      sort_order: presets.length,
    })
    setNewPresetLabel('')
    setNewPresetCommand('')
    setShowNewPreset(false)
  }

  const handleAnalyze = async () => {
    const projectPath = currentProject?.path || '.'
    await buildRepoMap(currentProject?.id || 'default', projectPath)
  }

  const handleDocs = () => {
    navigate('/knowledge')
  }
  
  const handlePresetClick = (presetId: string) => {
    const agentIds = Array.from(agents.keys())
    if (agentIds.length > 0) {
      executePreset(presetId, agentIds[0])
    }
  }
  
  return (
    <div className="flex flex-col h-full min-h-screen">
      {controlDeferredNotice && (
        <div className="px-4 pt-3">
          <div className="p-2 rounded-md border border-yellow-500/30 bg-yellow-500/10 text-yellow-300 text-xs flex items-center justify-between gap-2">
            <span className="flex-1">{controlDeferredNotice}</span>
            <button
              onClick={clearControlNotice}
              className="text-yellow-300/70 hover:text-yellow-300 shrink-0"
              title="Dismiss"
            >
              <XCircle className="size-3.5" />
            </button>
          </div>
        </div>
      )}

      {/* Project Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-glass-border bg-glass-10 backdrop-blur-md">
        <div className="flex items-center gap-3">
          <span className="text-sm font-medium text-muted-foreground">Project:</span>
          <select 
            className="bg-glass-20 border border-glass-border rounded-lg px-3 py-1.5 text-sm text-foreground outline-none focus:border-indigo-500/40 focus:ring-1 focus:ring-indigo-500/20"
            value={currentProject?.path || ''}
            onChange={(e) => switchProject(e.target.value)}
          >
            <option value="">Select project...</option>
            {recentPaths.map((path) => (
              <option key={path} value={path}>{path}</option>
            ))}
          </select>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleToggleControlMode}
            className={
              controlModeActive
                ? 'px-3 py-1.5 text-xs rounded-lg border border-amber-500/50 bg-amber-500/20 text-amber-200 font-medium flex items-center gap-1.5'
                : 'px-3 py-1.5 text-xs text-foreground bg-glass-20 border border-glass-border rounded-lg glass-hover flex items-center gap-1.5'
            }
            title="Toggle Control Mode (per-panel control sessions)"
          >
            <Sliders className="size-3.5" />
            {controlModeActive ? 'Control Mode: On' : 'Control Mode'}
          </button>
          <button
            onClick={() => void handleLoadProfile()}
            className="gradient-primary px-3 py-1.5 text-xs text-white rounded-lg shadow-glow-sm hover:shadow-glow-lg hover:-translate-y-px active:scale-[0.98] transition-all duration-200"
          >
            Load Profile
          </button>
        </div>
      </div>
      
      {controlConflicts.length > 0 && (
        <div className="px-4 pt-3">
          <div className="p-3 rounded-lg border border-amber-500/30 bg-amber-500/10 flex items-start gap-2">
            <AlertTriangle className="size-4 text-amber-400 mt-0.5 shrink-0" />
            <div className="text-xs text-amber-200 min-w-0 flex-1">
              <div className="font-semibold mb-1">
                Cross-thread file ownership conflicts ({controlConflicts.length})
              </div>
              <ul className="list-disc list-inside space-y-0.5 break-all">
                {controlConflicts.map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-auto p-4">
        {/* AGENTS Section */}
        <section className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-3">
              <div className="gradient-accent-bar" style={{ height: 22 }} />
              <h2 className="text-lg font-semibold text-foreground">AGENTS</h2>
            </div>
            <div className="relative">
              <button
                onClick={() => setShowAgentDropdown(!showAgentDropdown)}
                className="gradient-primary px-3 py-1.5 text-xs text-white rounded-lg shadow-glow-sm hover:shadow-glow-lg hover:-translate-y-px active:scale-[0.98] transition-all duration-200"
              >
                + Add Agent
              </button>
              {showAgentDropdown && (
                <div className="absolute right-0 top-full mt-1 w-48 glass-card shadow-lg z-10 p-1 max-h-72 overflow-y-auto">
                  {AGENT_CONFIGS.map((agent) => (
                    <button
                      key={agent.id}
                      onClick={() => handleSpawnAgent(agent)}
                      className="w-full px-3 py-2 text-left text-sm text-foreground glass-hover rounded-md transition-colors first:rounded-t-md last:rounded-b-md flex items-center justify-between gap-2"
                    >
                      <span>{agent.label}</span>
                      <span className="text-[10px] text-muted-foreground">T{agent.tier}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="mb-3">
            <OrchestratorToggle
              enabled={orchestratorMode}
              onToggle={setOrchestratorMode}
              orchestratorAgentId={orchestratorAgent}
              onSetOrchestrator={setOrchestratorAgent}
              agentLabels={agentLabels}
            />
          </div>
          
          <AgentGrid orchestratorId={orchestratorMode ? orchestratorAgent : null} />
        </section>

        {controlModeActive && (
          <section className="mb-6">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <div className="gradient-accent-bar" style={{ height: 22 }} />
                <h2 className="text-lg font-semibold text-foreground">CONTROL SESSIONS</h2>
                <span className="text-xs px-2 py-0.5 rounded-md bg-amber-500/15 text-amber-300 border border-amber-500/20 font-medium">
                  thread: {controlThreadId}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleRefreshControl}
                  className="px-3 py-1.5 text-xs text-foreground bg-glass-20 border border-glass-border rounded-lg glass-hover"
                >
                  Refresh
                </button>
                <button
                  onClick={handleToggleControlMode}
                  className="px-3 py-1.5 text-xs text-muted-foreground glass-hover rounded-md"
                >
                  Exit Control Mode
                </button>
              </div>
            </div>

            <div className="grid gap-2">
              {agentLabels.map(({ id, label }) => {
                const session = controlSessions.find((s) => s.panel_id === id)
                return (
                  <div
                    key={id}
                    className="flex items-center justify-between p-2.5 rounded-lg bg-glass-10 border border-glass-border"
                  >
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <span className="text-sm text-foreground shrink-0">{label}</span>
                      {session ? (
                        <ControlStateBadge state={session.state} />
                      ) : (
                        <span className="text-xs text-muted-foreground">no session</span>
                      )}
                      {session && (
                        <span className="text-xs text-muted-foreground truncate" title={session.docs_dir}>
                          docs: {session.docs_dir}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-1.5 shrink-0">
                      {!session && (
                        <button
                          onClick={() => handlePromote(id)}
                          className="px-2 py-1 text-xs bg-indigo-500/20 text-indigo-200 border border-indigo-500/30 rounded-md hover:bg-indigo-500/30 flex items-center gap-1"
                        >
                          <Wand2 className="size-3" />
                          Promote
                        </button>
                      )}
                      {session?.state === 'promoted' && (
                        <button
                          onClick={() => handleSetControlState(session.id, 'active')}
                          className="px-2 py-1 text-xs bg-emerald-500/15 text-emerald-300 border border-emerald-500/20 rounded-md hover:bg-emerald-500/25 flex items-center gap-1"
                        >
                          <Play className="size-3" />
                          Start
                        </button>
                      )}
                      {session?.state === 'active' && (
                        <button
                          onClick={() => handleSetControlState(session.id, 'paused')}
                          className="px-2 py-1 text-xs bg-amber-500/15 text-amber-300 border border-amber-500/20 rounded-md hover:bg-amber-500/25 flex items-center gap-1"
                        >
                          <Pause className="size-3" />
                          Pause
                        </button>
                      )}
                      {session?.state === 'paused' && (
                        <button
                          onClick={() => handleSetControlState(session.id, 'active')}
                          className="px-2 py-1 text-xs bg-emerald-500/15 text-emerald-300 border border-emerald-500/20 rounded-md hover:bg-emerald-500/25 flex items-center gap-1"
                        >
                          <Play className="size-3" />
                          Resume
                        </button>
                      )}
                      {session && session.state !== 'completed' && (
                        <button
                          onClick={() => handleSetControlState(session.id, 'completed')}
                          className="px-2 py-1 text-xs bg-glass-20 text-muted-foreground border border-glass-border rounded-md hover:bg-glass-30 flex items-center gap-1"
                        >
                          <CheckCircle2 className="size-3" />
                          Complete
                        </button>
                      )}
                      {session?.state === 'completed' && (
                        <span className="text-xs text-muted-foreground px-2 py-1">done</span>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          </section>
        )}
        
        {/* PRESETS Section */}
        <section className="mb-6">
          <div className="flex items-center gap-3 mb-3">
            <div className="gradient-accent-bar" style={{ height: 22 }} />
            <h2 className="text-lg font-semibold text-foreground">PRESETS</h2>
          </div>
          <div className="flex flex-wrap gap-2">
            {presets.map((preset) => (
              <button
                key={preset.id}
                onClick={() => handlePresetClick(preset.id)}
                className="px-3 py-1.5 text-sm bg-glass-20 border border-glass-border rounded-lg text-foreground glass-hover"
              >
                {preset.label}
              </button>
            ))}
            <button
              onClick={() => setShowNewPreset((v) => !v)}
              className="px-3 py-1.5 text-sm bg-glass-10 text-muted-foreground border border-dashed border-glass-border rounded-lg glass-hover"
            >
              + New
            </button>
          </div>
          {showNewPreset && (
            <div className="mt-3 flex flex-wrap items-center gap-2 p-3 rounded-lg bg-glass-10 border border-glass-border">
              <input
                type="text"
                placeholder="Preset label"
                value={newPresetLabel}
                onChange={(e) => setNewPresetLabel(e.target.value)}
                className="px-2.5 py-1.5 text-xs bg-glass-20 border border-glass-border rounded-md text-foreground outline-none"
              />
              <input
                type="text"
                placeholder="Command"
                value={newPresetCommand}
                onChange={(e) => setNewPresetCommand(e.target.value)}
                className="px-2.5 py-1.5 text-xs bg-glass-20 border border-glass-border rounded-md text-foreground outline-none flex-1 min-w-[160px]"
              />
              <button
                onClick={handleCreatePreset}
                className="gradient-primary px-3 py-1.5 text-xs text-white rounded-md"
              >
                Save Preset
              </button>
            </div>
          )}
        </section>
      </div>
      
      {/* Session Footer */}
      <div className="flex items-center justify-between px-4 py-2 border-t border-glass-border bg-glass-10 backdrop-blur-md text-sm">
        <div className="flex items-center gap-4 text-muted-foreground">
          <span className="font-medium text-xs uppercase tracking-wider">Session</span>
          <span>{agentCount} agents</span>
          <span>{totalFilesChanged} files changed</span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void handleAnalyze()}
            className="px-2 py-1 text-xs text-muted-foreground glass-hover rounded-md"
          >
            Analyze
          </button>
          <button
            onClick={handleDocs}
            className="px-2 py-1 text-xs text-muted-foreground glass-hover rounded-md"
          >
            Docs
          </button>
        </div>
      </div>
    </div>
  )
}

function ControlStateBadge({ state }: { state: ControlState }) {
  const styles: Record<ControlState, string> = {
    promoted: 'bg-indigo-500/15 text-indigo-300 border-indigo-500/20',
    active: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/20',
    paused: 'bg-amber-500/15 text-amber-300 border-amber-500/20',
    completed: 'bg-glass-20 text-muted-foreground border-glass-border',
  }
  return (
    <span
      className={`text-xs px-1.5 py-0.5 rounded-md border font-medium ${styles[state]}`}
    >
      {state}
    </span>
  )
}
