import { useState, useEffect, useCallback } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useOrchestrationStore, type PlanAgent, type CorrectionDoc } from "@/stores/orchestrationStore";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import {
  Waves,
  Plus,
  Play,
  CheckCircle2,
  XCircle,
  RotateCcw,
  Bot,
  Layers,
  Bug,
  ChevronDown,
  ChevronUp,
  ClipboardList,
  MessageSquare,
} from "lucide-react";
import { HandoffPanel } from "@/components/orchestrate/HandoffPanel";
import { MessagePanel } from "@/components/orchestrate/MessagePanel";

const STATUS_COLORS: Record<string, string> = {
  queued: "text-gray-400 bg-gray-500/20 border-gray-500/30",
  running: "text-[#58a6ff] bg-[#58a6ff]/20 border-[#58a6ff]/30",
  done: "text-green-400 bg-green-500/20 border-green-500/30",
  failed: "text-red-400 bg-red-500/20 border-red-500/30",
  blocked: "text-yellow-400 bg-yellow-500/20 border-yellow-500/30",
};

const TABS = [
  { id: "plan", label: "Wave Plan", icon: Waves },
  { id: "handoffs", label: "Handoffs", icon: ClipboardList },
  { id: "messages", label: "Messages", icon: MessageSquare },
] as const;

type TabId = typeof TABS[number]["id"];

export default function Orchestrate() {
  const location = useLocation();
  const navigate = useNavigate();
  const store = useOrchestrationStore();
  const [selectedPlanId, setSelectedPlanId] = useState<string | null>(null);
  const [newSlug, setNewSlug] = useState("");
  const [newProjectId, setNewProjectId] = useState("acc-main");
  const [newAgent, setNewAgent] = useState({ agentRef: "", task: "", wave: "1", dependsOn: "", agentId: "" });
  const [hasLoaded, setHasLoaded] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [isFinalizing, setIsFinalizing] = useState(false);
  const [selectedHarness, setSelectedHarness] = useState("opencode");
  const [actionError, setActionError] = useState<string | null>(null);

  const activeTab: TabId = location.pathname.includes("/handoffs")
    ? "handoffs"
    : location.pathname.includes("/messages")
    ? "messages"
    : "plan";

  const handleTabChange = (tabId: TabId) => {
    const paths: Record<TabId, string> = {
      plan: "/orchestrate",
      handoffs: "/orchestrate/handoffs",
      messages: "/orchestrate/messages",
    };
    navigate(paths[tabId]);
  };

  useEffect(() => {
    if (!hasLoaded) {
      setHasLoaded(true);
      void store.getWavePlans().catch(() => {});
    }
  }, [hasLoaded, store]);

  const handleCreatePlan = async () => {
    if (!newSlug.trim()) return;
    setActionError(null);
    const plan = await store.createWavePlan(newProjectId, newSlug);
    setSelectedPlanId(plan.id);
    setNewSlug("");
    await store.getPlanAgents(plan.id);
  };

  const handleAddAgent = async () => {
    if (!selectedPlanId || !newAgent.agentRef.trim() || !newAgent.task.trim()) return;
    setActionError(null);
    try {
      await store.addPlanAgent(
        selectedPlanId,
        newAgent.agentRef,
        newAgent.task,
        parseInt(newAgent.wave) || 1,
        newAgent.dependsOn || undefined,
        newAgent.agentId || undefined
      );
      setNewAgent({ agentRef: "", task: "", wave: "1", dependsOn: "", agentId: "" });
      await store.getPlanAgents(selectedPlanId);
      await store.getCorrections(selectedPlanId);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExecuteWave = async () => {
    if (!selectedPlanId) return;
    setActionError(null);
    setIsExecuting(true);
    try {
      await store.executeWave(selectedPlanId, ".", "main", selectedHarness, []);
      await store.getPlanAgents(selectedPlanId);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsExecuting(false);
    }
  };

  const handleVerifyAndFinalize = async () => {
    if (!selectedPlanId || !store.lastWaveReport) return;
    setActionError(null);
    setIsFinalizing(true);
    try {
      await store.finalizeWave(store.lastWaveReport, ".");
      await store.getPlanAgents(selectedPlanId);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsFinalizing(false);
    }
  };

  const handleUpdateStatus = async (agentId: string, status: string) => {
    await store.updatePlanAgentStatus(agentId, status);
    if (selectedPlanId) await store.getPlanAgents(selectedPlanId);
  };

  const groupedByWave = useCallback(() => {
    const groups: Record<number, PlanAgent[]> = {};
    for (const a of store.planAgents) {
      if (!groups[a.wave]) groups[a.wave] = [];
      groups[a.wave].push(a);
    }
    return Object.entries(groups).sort(([a], [b]) => parseInt(a) - parseInt(b));
  }, [store.planAgents]);

  const waveSummary = (agents: PlanAgent[]) => {
    const done = agents.filter((a) => a.status === "done").length;
    const failed = agents.filter((a) => a.status === "failed").length;
    const total = agents.length;
    const running = agents.some((a) => a.status === "running");
    return { done, failed, total, running };
  };

  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
          <div className="page-header">
            <div className="gradient-accent-bar" />
            <h1>Wave Orchestrator</h1>
          </div>
      </div>

      {/* Tab Bar */}
      <div className="flex gap-1 border-b border-glass-border pb-1" role="tablist">
        {TABS.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={isActive}
              onClick={() => handleTabChange(tab.id)}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-t-md transition-colors",
                isActive
                  ? "text-indigo-300 border-b-2 border-indigo-500"
                  : "text-muted-foreground/60 hover:text-muted-foreground"
              )}
            >
              <Icon className="size-3.5" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {activeTab === "plan" && (
        <>
      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Layers className="size-4 text-gray-400" />
          <span className="text-sm font-medium text-gray-300">Create Wave Plan</span>
        </div>
        <div className="flex gap-3 items-end">
          <div className="flex-1">
            <label className="block text-xs text-gray-400 mb-1">Slug</label>
            <Input
              value={newSlug}
              onChange={(e) => setNewSlug(e.target.value)}
              placeholder="e.g., add-dark-mode"
              className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
            />
          </div>
          <div className="w-40">
            <label className="block text-xs text-gray-400 mb-1">Project ID</label>
            <Input
              value={newProjectId}
              onChange={(e) => setNewProjectId(e.target.value)}
              className="bg-[#0d1117] border-[#30363d] text-gray-300"
            />
          </div>
          <Button onClick={handleCreatePlan} disabled={!newSlug.trim()} className="gap-1.5">
            <Plus className="size-4" />
            Create Plan
          </Button>
        </div>
      </Card>

      {/* Plan List / Selection */}
      {store.wavePlans.length > 0 && (
        <div className="flex gap-2 flex-wrap">
          {store.wavePlans.map((plan) => (
            <Badge
              key={plan.id}
              variant="outline"
              className={cn(
                "cursor-pointer px-3 py-1.5 text-xs transition-colors",
                selectedPlanId === plan.id
                  ? "border-[#1f6feb] text-[#58a6ff] bg-[#1f6feb]/10"
                  : "border-[#30363d] text-gray-400 hover:border-[#58a6ff]/50"
              )}
              onClick={async () => {
                setSelectedPlanId(plan.id);
                await store.getPlanAgents(plan.id);
                await store.getCorrections(plan.id);
              }}
            >
              <Waves className="size-3 mr-1" />
              {plan.slug}
            </Badge>
          ))}
        </div>
      )}

      {/* Selected Plan Content */}
      {selectedPlanId && (
        <>
          {/* Execution & Verification Controls */}
          <Card className="p-4 space-y-3">
            <div className="flex items-center justify-between gap-2 flex-wrap">
              <div className="flex items-center gap-2">
                <Waves className="size-4 text-[#58a6ff]" />
                <span className="text-sm font-medium text-gray-200">Wave Execution</span>
              </div>
              <div className="flex items-center gap-2">
                <select
                  aria-label="Execution Harness"
                  value={selectedHarness}
                  onChange={(e) => setSelectedHarness(e.target.value)}
                  disabled={isExecuting || isFinalizing}
                  className="h-8 rounded-md border border-[#30363d] bg-[#0d1117] px-2 text-xs text-gray-200"
                >
                  <option value="pi">Pi Agent (ACP)</option>
                  <option value="opencode">OpenCode</option>
                  <option value="opencode-acp">OpenCode (ACP)</option>
                  <option value="claude">Claude Code (ACP)</option>
                  <option value="codex">Codex CLI (ACP)</option>
                  <option value="gemini">Gemini CLI (ACP)</option>
                  <option value="mock">Mock</option>
                </select>
                <Button
                  size="sm"
                  onClick={handleExecuteWave}
                  disabled={isExecuting || isFinalizing}
                  className="gap-1.5"
                >
                  <Play className="size-3.5" />
                  {isExecuting ? "Executing..." : "Execute Wave Plan"}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleVerifyAndFinalize}
                  disabled={!store.lastWaveReport || isExecuting || isFinalizing}
                  className="gap-1.5"
                >
                  <CheckCircle2 className="size-3.5" />
                  {isFinalizing ? "Verifying..." : "Verify & Finalize"}
                </Button>
              </div>
            </div>

            {actionError && (
              <div className="rounded border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
                {actionError}
              </div>
            )}

            {store.lastWaveReport && store.lastWaveReport.plan_id === selectedPlanId && (
              <div className="space-y-2 border-t border-[#30363d] pt-3 text-xs">
                <div className="flex items-center justify-between text-gray-400">
                  <span>Execution Report ({store.lastWaveReport.agents.length} agents)</span>
                  <span>Cost: ${store.lastWaveReport.total_cost_usd.toFixed(4)}</span>
                </div>
                <div className="space-y-1">
                  {store.lastWaveReport.agents.map((exec) => (
                    <div
                      key={`${exec.agent_ref}-${exec.session_id}`}
                      className="flex items-center justify-between rounded bg-[#0d1117] px-2.5 py-1.5"
                    >
                      <span className="font-mono text-gray-200">
                        {exec.agent_ref} ({exec.branch})
                      </span>
                      <Badge variant="outline" className={cn("text-xs", STATUS_COLORS[exec.status] || STATUS_COLORS.queued)}>
                        {exec.status}
                      </Badge>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {store.lastVerification && (
              <div className="space-y-2 border-t border-[#30363d] pt-3 text-xs">
                <div className="flex items-center gap-2">
                  {store.lastVerification.passed ? (
                    <CheckCircle2 className="size-4 text-green-400" />
                  ) : (
                    <XCircle className="size-4 text-red-400" />
                  )}
                  <span className={store.lastVerification.passed ? "text-green-400 font-medium" : "text-red-400 font-medium"}>
                    Verification {store.lastVerification.passed ? "Passed" : "Failed"}
                  </span>
                </div>
                <div className="space-y-1">
                  {store.lastVerification.checks.map((chk) => (
                    <div key={chk.name} className="flex items-center justify-between rounded bg-[#0d1117] px-2.5 py-1">
                      <span className="text-gray-300">{chk.name}</span>
                      <span className="text-gray-400">
                        {typeof chk.status === "string" ? chk.status : "Fail"}{chk.detail ? ` — ${chk.detail}` : ""}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </Card>

          {/* Add Agent */}
          <Card className="p-4 space-y-3">
            <div className="flex items-center gap-2">
              <Bot className="size-4 text-gray-400" />
              <span className="text-sm font-medium text-gray-300">Add Plan Agent</span>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-gray-400 mb-1">Agent Ref</label>
                <Input
                  value={newAgent.agentRef}
                  onChange={(e) => setNewAgent((p) => ({ ...p, agentRef: e.target.value }))}
                  placeholder="e.g., claude"
                  className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Agent ID (optional)</label>
                <Input
                  value={newAgent.agentId}
                  onChange={(e) => setNewAgent((p) => ({ ...p, agentId: e.target.value }))}
                  placeholder="e.g., claude-code"
                  className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Task</label>
                <Input
                  value={newAgent.task}
                  onChange={(e) => setNewAgent((p) => ({ ...p, task: e.target.value }))}
                  placeholder="What should this agent do?"
                  className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                />
              </div>
              <div className="flex gap-2">
                <div className="flex-1">
                  <label className="block text-xs text-gray-400 mb-1">Wave #</label>
                  <Input
                    value={newAgent.wave}
                    onChange={(e) => setNewAgent((p) => ({ ...p, wave: e.target.value }))}
                    className="bg-[#0d1117] border-[#30363d] text-gray-300"
                  />
                </div>
                <div className="flex-1">
                  <label className="block text-xs text-gray-400 mb-1">Depends On</label>
                  <Input
                    value={newAgent.dependsOn}
                    onChange={(e) => setNewAgent((p) => ({ ...p, dependsOn: e.target.value }))}
                    placeholder="agent ref"
                    className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                  />
                </div>
              </div>
            </div>
            <Button size="sm" onClick={handleAddAgent} disabled={!newAgent.agentRef.trim()} className="gap-1.5">
              <Plus className="size-3.5" />
              Add Agent
            </Button>
          </Card>

          {/* Wave Groups */}
          <ScrollArea className="flex-1">
            <div className="space-y-6">
              {groupedByWave().map(([waveNum, agents]) => {
                const summary = waveSummary(agents);
                return (
                  <WaveGroup
                    key={waveNum}
                    waveNum={parseInt(waveNum)}
                    agents={agents}
                    summary={summary}
                    onUpdateStatus={handleUpdateStatus}
                  />
                );
              })}
            </div>
          </ScrollArea>

          {/* Corrections */}
          {store.corrections.length > 0 && (
            <div>
              <div className="flex items-center gap-2 mb-3">
                <Bug className="size-4 text-red-400" />
                <span className="text-sm font-medium text-gray-300">
                  Corrections ({store.corrections.length})
                </span>
              </div>
              <div className="space-y-2">
                {store.corrections.map((c) => (
                  <CorrectionCard key={c.id} correction={c} />
                ))}
              </div>
            </div>
          )}
        </>
      )}

      {!selectedPlanId && store.wavePlans.length === 0 && (
        <div className="flex flex-col items-center justify-center flex-1 text-gray-500 gap-3">
          <Waves className="size-12 opacity-30" />
          <p className="text-sm">Create a wave plan to begin orchestrating agents.</p>
          <p className="text-xs text-gray-600">Wave plans coordinate multiple agents across sequential phases.</p>
        </div>
      )}
      </>)}
      {activeTab === "handoffs" && (
        <HandoffPanel />
      )}

      {activeTab === "messages" && (
        <MessagePanel />
      )}
    </div>
  );
}

function WaveGroup({
  waveNum,
  agents,
  summary,
  onUpdateStatus,
}: {
  waveNum: number;
  agents: PlanAgent[];
  summary: { done: number; failed: number; total: number; running: boolean };
  onUpdateStatus: (id: string, status: string) => void;
}) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="space-y-2">
      <div
        className="flex items-center gap-3 py-1.5 cursor-pointer select-none"
        onClick={() => setCollapsed(!collapsed)}
      >
        {collapsed ? <ChevronDown className="size-4 text-gray-500" /> : <ChevronUp className="size-4 text-gray-500" />}
        <span className="text-sm font-semibold text-gray-200">Wave {waveNum}</span>
        <Separator orientation="vertical" className="h-4" />
        <span className="text-xs text-gray-400">
          {summary.done}/{summary.total} done
        </span>
        {summary.failed > 0 && (
          <Badge variant="outline" className="text-xs border-red-500/30 text-red-400">
            {summary.failed} failed
          </Badge>
        )}
        {summary.running && (
          <Badge variant="outline" className="text-xs border-[#58a6ff]/30 text-[#58a6ff]">
            running
          </Badge>
        )}
        {summary.done === summary.total && summary.total > 0 && (
          <CheckCircle2 className="size-4 text-green-500" />
        )}
      </div>

      {!collapsed && (
        <div className="grid grid-cols-1 gap-2 pl-2 border-l-2 border-[#30363d]">
          {agents.map((agent) => (
            <AgentCard key={agent.id} agent={agent} onUpdateStatus={onUpdateStatus} />
          ))}
        </div>
      )}
    </div>
  );
}

function AgentCard({
  agent,
  onUpdateStatus,
}: {
  agent: PlanAgent;
  onUpdateStatus: (id: string, status: string) => void;
}) {
  const statusClass = STATUS_COLORS[agent.status] || STATUS_COLORS.queued;

  return (
    <Card className="p-3 border-[#30363d] bg-[#161b22]">
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <Bot className="size-4 text-gray-400 shrink-0" />
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium text-sm text-gray-200">{agent.agent_ref}</span>
              <Badge variant="outline" className={cn("text-xs", statusClass)}>
                {agent.status}
              </Badge>
              {agent.retry_count > 0 && (
                <Badge variant="outline" className="text-xs border-yellow-500/30 text-yellow-400">
                  <RotateCcw className="size-3 mr-0.5" />
                  {agent.retry_count}
                </Badge>
              )}
            </div>
            <p className="text-xs text-gray-400 mt-0.5 truncate">{agent.task}</p>
            {agent.depends_on && (
              <p className="text-xs text-gray-600 mt-0.5">
                Depends on: {agent.depends_on}
              </p>
            )}
          </div>
        </div>
        <div className="flex items-center gap-1 shrink-0 ml-2">
          {agent.status === "queued" && (
            <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => onUpdateStatus(agent.id, "running")}>
              <Play className="size-3" /> Start
            </Button>
          )}
          {agent.status === "running" && (
            <>
              <Button size="sm" variant="outline" className="h-7 text-xs gap-1 text-green-400" onClick={() => onUpdateStatus(agent.id, "done")}>
                <CheckCircle2 className="size-3" /> Done
              </Button>
              <Button size="sm" variant="outline" className="h-7 text-xs gap-1 text-red-400" onClick={() => onUpdateStatus(agent.id, "failed")}>
                <XCircle className="size-3" /> Fail
              </Button>
            </>
          )}
          {agent.status === "done" && (
            <div className="flex items-center gap-1 text-xs text-green-400">
              <CheckCircle2 className="size-3.5" />
              Complete
            </div>
          )}
          {agent.status === "failed" && (
            <Button size="sm" variant="outline" className="h-7 text-xs gap-1 text-yellow-400" onClick={() => onUpdateStatus(agent.id, "queued")}>
              <RotateCcw className="size-3" /> Retry
            </Button>
          )}
        </div>
      </div>
    </Card>
  );
}

function CorrectionCard({ correction }: { correction: CorrectionDoc }) {
  return (
    <Card className={cn("p-3 border", correction.resolved ? "border-[#30363d] opacity-60" : "border-red-500/30 bg-[#161b22]")}>
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <Bug className="size-3.5 text-red-400" />
            <span className="text-sm font-medium text-gray-200">{correction.agent_ref}</span>
            <Badge variant="outline" className="text-xs border-yellow-500/30 text-yellow-400">
              <RotateCcw className="size-3 mr-0.5" />
              Retry #{correction.retry_number}
            </Badge>
            {correction.resolved && (
              <Badge variant="outline" className="text-xs border-green-500/30 text-green-400">
                Resolved
              </Badge>
            )}
          </div>
          {correction.bug_desc && (
            <p className="text-xs text-gray-400 mb-1">
              <span className="text-gray-500">Bug:</span> {correction.bug_desc}
            </p>
          )}
          {correction.root_cause && (
            <p className="text-xs text-gray-400 mb-1">
              <span className="text-gray-500">Root Cause:</span> {correction.root_cause}
            </p>
          )}
          {correction.fix_required && (
            <p className="text-xs text-gray-400">
              <span className="text-gray-500">Fix:</span> {correction.fix_required}
            </p>
          )}
        </div>
      </div>
    </Card>
  );
}
