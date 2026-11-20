import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useOrchestrationStore, type TaskSuggestion } from "@/stores/orchestrationStore";
import { useProjectStore } from "@/stores/projectStore";
import { useAgentStore } from "@/stores/agentStore";
import { getAgentConfig } from "@/lib/agents/configs";
import type { AgentConfig } from "@/lib/types";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { Send, TrendingUp, BarChart3, CheckCircle2 } from "lucide-react";

export default function RoutePage() {
  const navigate = useNavigate();
  const { suggestions, routeTask } = useOrchestrationStore();
  const { currentProject, recentProjects } = useProjectStore();
  const spawnAgent = useAgentStore((s) => s.spawnAgent);
  const [taskDesc, setTaskDesc] = useState("");
  const [taskType, setTaskType] = useState("implement");
  const [projectId, setProjectId] = useState("");
  const [loading, setLoading] = useState(false);
  const [sendingAgentId, setSendingAgentId] = useState<string | null>(null);
  const [sendError, setSendError] = useState<string | null>(null);

  const handleRoute = async () => {
    if (!taskDesc.trim()) return;
    setLoading(true);
    await routeTask(taskDesc, taskType, projectId || undefined);
    setLoading(false);
  };

  const handleSendToAgent = async (agentId: string) => {
    setSendingAgentId(agentId);
    setSendError(null);
    try {
      const existingAgents = useAgentStore.getState().agents;
      if (!existingAgents.has(agentId)) {
        const config: AgentConfig = getAgentConfig(agentId) ?? {
          id: agentId,
          label: agentId === "pi" ? "Pi Agent (ACP)" : agentId,
          spawnCmd: agentId,
          defaultArgs: [],
          memoryFile: "AGENTS.md",
          globalConfigPath: `~/.${agentId}/`,
          mcpConfigFile: "config.json",
          mcpConfigKey: "mcpServers",
          tier: 1,
          waveEligible: true,
          supportsSubagents: false,
        };
        await spawnAgent(config, currentProject?.path || ".");
      }
      navigate("/runner");
    } catch (err) {
      setSendError(err instanceof Error ? err.message : String(err));
    } finally {
      setSendingAgentId(null);
    }
  };

  return (
    <div className="flex h-full flex-col p-6 gap-6">
      <div className="flex items-center justify-between">
          <div className="page-header">
            <div className="gradient-accent-bar" />
            <h1>Task Router</h1>
          </div>
      </div>

      <Card className="p-5 space-y-4">
        <div className="flex gap-3">
          <div className="flex-1">
            <label className="block text-xs text-gray-400 mb-1.5">Task Description</label>
            <Input
              value={taskDesc}
              onChange={(e) => setTaskDesc(e.target.value)}
              placeholder="Describe the task (e.g., 'Implement user authentication with JWT')"
              className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
            />
          </div>
        </div>
        <div className="flex gap-3">
          <div className="w-48">
            <label className="block text-xs text-gray-400 mb-1.5">Task Type</label>
            <select
              value={taskType}
              onChange={(e) => setTaskType(e.target.value)}
              className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
            >
              <option value="implement">Implement</option>
              <option value="refactor">Refactor</option>
              <option value="review">Review</option>
              <option value="test">Test</option>
              <option value="debug">Debug</option>
              <option value="document">Document</option>
            </select>
          </div>
          <div className="flex-1">
            <label htmlFor="route-project-scope" className="block text-xs text-gray-400 mb-1.5">Project Scope</label>
            <select
              id="route-project-scope"
              aria-label="Project Scope"
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
              className="w-full h-9 px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
            >
              <option value="">All Projects (Cross-project routing)</option>
              {currentProject && (
                <option value={currentProject.id || currentProject.path}>
                  Active: {currentProject.name || currentProject.id} ({currentProject.path})
                </option>
              )}
              {recentProjects
                .filter((p) => p !== currentProject?.path && p !== currentProject?.id)
                .map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
            </select>
          </div>
          <div className="flex items-end">
            <Button onClick={handleRoute} disabled={loading || !taskDesc.trim()} className="gap-2">
              {loading ? (
                <span className="animate-spin size-4 border-2 border-white border-t-transparent rounded-full" />
              ) : (
                <Send className="size-4" />
              )}
              Route Task
            </Button>
          </div>
        </div>
      </Card>

      {sendError && (
        <div className="rounded border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
          {sendError}
        </div>
      )}

      {suggestions.length > 0 && (
        <div>
          <div className="flex items-center gap-2 mb-3">
            <TrendingUp className="size-4 text-[#58a6ff]" />
            <span className="text-sm font-medium text-gray-300">
              Agent Suggestions ({suggestions.length})
            </span>
          </div>
          <ScrollArea className="flex-1">
            <div className="grid grid-cols-1 gap-3">
              {suggestions.map((s, i) => (
                <SuggestionCard
                  key={`${s.agent_id}-${i}`}
                  suggestion={s}
                  rank={i + 1}
                  sending={sendingAgentId === s.agent_id}
                  onSendToAgent={handleSendToAgent}
                />
              ))}
            </div>
          </ScrollArea>
        </div>
      )}

      {suggestions.length === 0 && (
        <div className="flex flex-col items-center justify-center flex-1 text-gray-500 gap-3">
          <BarChart3 className="size-12 opacity-30" />
          <p className="text-sm">Enter a task description and click Route Task to get agent suggestions.</p>
          <p className="text-xs text-gray-600">The router analyzes outcome history to recommend the best agents.</p>
        </div>
      )}
    </div>
  );
}

function SuggestionCard({
  suggestion,
  rank,
  sending,
  onSendToAgent,
}: {
  suggestion: TaskSuggestion;
  rank: number;
  sending: boolean;
  onSendToAgent: (agentId: string) => void;
}) {
  const pct = (suggestion.success_rate * 100).toFixed(0);
  const color = suggestion.success_rate >= 0.7 ? "green" : suggestion.success_rate >= 0.4 ? "yellow" : "red";

  return (
    <Card className="p-4 border-[#30363d] hover:border-[#1f6feb]/50 transition-colors">
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-2">
            <Badge
              variant="outline"
              className={cn(
                "text-xs",
                rank === 1 ? "border-[#d29922] text-[#d29922]" : "border-[#30363d] text-gray-400"
              )}
            >
              #{rank}
            </Badge>
            <span className="font-medium text-gray-200">{suggestion.agent_id}</span>
            <Badge
              variant="outline"
              className={cn(
                "text-xs",
                color === "green"
                  ? "border-green-500/30 text-green-400"
                  : color === "yellow"
                    ? "border-yellow-500/30 text-yellow-400"
                    : "border-red-500/30 text-red-400"
              )}
            >
              {pct}% success
            </Badge>
          </div>
          <p className="text-sm text-gray-400 mb-3">{suggestion.reasoning}</p>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <CheckCircle2 className="size-3.5 text-green-500" />
              <span className="text-xs text-gray-400">
                Confidence: {(suggestion.confidence * 100).toFixed(0)}%
              </span>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-2 ml-4 shrink-0">
          <div className="w-20 h-3 bg-[#21262d] rounded-full overflow-hidden">
            <div
              className={cn(
                "h-full rounded-full transition-all",
                color === "green" ? "bg-green-500" : color === "yellow" ? "bg-yellow-500" : "bg-red-500"
              )}
              style={{ width: `${Math.max(4, suggestion.success_rate * 100)}%` }}
            />
          </div>
          <Button
            size="sm"
            className="gap-1.5 text-xs"
            disabled={sending}
            onClick={() => onSendToAgent(suggestion.agent_id)}
          >
            <Send className="size-3" />
            {sending ? "Sending..." : "Send to Agent"}
          </Button>
        </div>
      </div>
    </Card>
  );
}
