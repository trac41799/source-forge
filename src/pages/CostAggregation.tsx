import { useEffect, useMemo, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IPC } from "@/lib/ipc/commands";
import { useBudgetStore, type WipEntry } from "@/stores/budgetStore";
import {
  DollarSign, Zap, BarChart3, Layers, Clock, Cpu, AlertTriangle,
  Flame, Gauge, FileWarning, PlayCircle, ChevronRight,
  TrendingUp, Wallet, Eye, RefreshCw, BookOpen, ListChecks
} from "lucide-react";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type {
  CostSummary, ModelCostSummary, ProjectCostSummary, SessionCostSummary
} from "@/lib/types";

type Tab = "overview" | "budgets" | "wip" | "models" | "projects" | "sessions";

const THRESHOLD_LEVELS = [
  { pct: 60, label: "60%", severity: "info", tone: "border-sky-500/40 bg-sky-500/10 text-sky-300" },
  { pct: 80, label: "80%", severity: "warning", tone: "border-amber-500/40 bg-amber-500/10 text-amber-300" },
  { pct: 95, label: "95%", severity: "critical", tone: "border-orange-500/40 bg-orange-500/10 text-orange-300" },
  { pct: 100, label: "100%", severity: "halt", tone: "border-red-500/40 bg-red-500/10 text-red-300" },
] as const;

const STATE_TONE: Record<string, string> = {
  active: "border-emerald-500/30 bg-emerald-500/10 text-emerald-300",
  warning: "border-amber-500/30 bg-amber-500/10 text-amber-300",
  critical: "border-orange-500/30 bg-orange-500/10 text-orange-300",
  exceeded: "border-red-500/30 bg-red-500/10 text-red-300",
};

const NOTIFICATION_TIMEOUT_MS = 4000;

export default function CostAggregation() {
  const [summary, setSummary] = useState<CostSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");

  const budgets = useBudgetStore((s) => s.budgets);
  const resumptionPlan = useBudgetStore((s) => s.resumptionPlan);
  const thresholdBudgets = useBudgetStore((s) => s.thresholdBudgets);
  const lastThresholdFired = useBudgetStore((s) => s.lastThresholdFired);
  const loadBudgets = useBudgetStore((s) => s.loadBudgets);
  const loadResumptionPlans = useBudgetStore((s) => s.loadResumptionPlans);
  const loadWips = useBudgetStore((s) => s.loadWips);
  const resumeBudget = useBudgetStore((s) => s.resumeBudget);
  const captureWip = useBudgetStore((s) => s.captureWip);
  const subscribeThresholdEvents = useBudgetStore((s) => s.subscribeThresholdEvents);
  const [wipEntries, setWipEntries] = useState<WipEntry[]>([]);
  const [wipPreview, setWipPreview] = useState<{ path: string; content: string } | null>(null);
  const [resumptionPreview, setResumptionPreview] = useState<string | null>(null);
  const [recentlyFired, setRecentlyFired] = useState<Set<number>>(new Set());
  const [notification, setNotification] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  const loadSummary = useCallback(async () => {
    try {
      const data = await invoke<CostSummary>(IPC.getCostSummary, { projectId: null });
      setSummary(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSummary();
  }, [loadSummary]);

  useEffect(() => {
    loadBudgets();
    loadResumptionPlans();
    loadWips().then(setWipEntries).catch(() => undefined);
  }, [loadBudgets, loadResumptionPlans, loadWips]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    (async () => {
      const u = await subscribeThresholdEvents();
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [subscribeThresholdEvents]);

  useEffect(() => {
    if (!lastThresholdFired) return;
    setRecentlyFired((prev) => {
      const next = new Set(prev);
      next.add(lastThresholdFired.percentage);
      return next;
    });
    setNotification({ kind: "ok", text: `${lastThresholdFired.agent_ref} hit ${lastThresholdFired.percentage}%` });
    loadWips().then(setWipEntries).catch(() => undefined);
  }, [lastThresholdFired, loadWips]);

  useEffect(() => {
    if (!notification) return;
    const t = setTimeout(() => setNotification(null), NOTIFICATION_TIMEOUT_MS);
    return () => clearTimeout(t);
  }, [notification]);

  if (loading) {
    return (
      <div className="flex flex-col gap-4 p-6">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Cost Aggregation</h1>
        </div>
        <p className="text-sm text-muted-foreground">Loading cost data...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col gap-4 p-6">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Cost Aggregation</h1>
        </div>
        <Card className="p-8 text-center">
          <AlertTriangle className="size-8 text-muted-foreground mx-auto mb-2" />
          <p className="text-sm text-muted-foreground">{error}</p>
          <p className="text-xs text-muted-foreground mt-1">
            Connect to a Supabase project or run agents to populate cost data.
          </p>
        </Card>
      </div>
    );
  }

  if (!summary) return null;

  const fmtCost = (usd: number) => `$${usd.toFixed(4)}`;
  const fmtTokens = (n: number) =>
    n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1_000 ? `${(n / 1_000).toFixed(1)}K` : String(n);

  const tabs: { id: Tab; label: string; icon: typeof DollarSign }[] = [
    { id: "overview", label: "Overview", icon: BarChart3 },
    { id: "budgets", label: "Budgets", icon: Wallet },
    { id: "wip", label: "WIP / Resumption", icon: FileWarning },
    { id: "models", label: "Models", icon: Cpu },
    { id: "projects", label: "Projects", icon: Layers },
    { id: "sessions", label: "Sessions", icon: Clock },
  ];

  const activeBudgets = budgets.filter((b) => b.state === "active").length;
  const warningBudgets = budgets.filter((b) => b.state === "warning" || b.state === "critical").length;
  const totalBudgetTokens = budgets.reduce((acc, b) => acc + b.budget_total, 0);
  const totalUsedTokens = budgets.reduce((acc, b) => acc + b.budget_used, 0);
  const burnRatePct = totalBudgetTokens > 0 ? (totalUsedTokens / totalBudgetTokens) * 100 : 0;
  const projectedMonthEnd = (() => {
    if (totalUsedTokens === 0 || summary.total_tokens === 0) return 0;
    const ratio = summary.estimated_total_cost_usd / summary.total_tokens;
    return ratio * totalBudgetTokens;
  })();

  return (
    <div className="flex flex-col gap-4 p-6">
      {notification && (
        <div
          className={cn(
            "fixed top-4 right-4 z-50 px-4 py-2 text-white text-sm rounded-lg shadow-lg animate-in fade-in",
            notification.kind === "ok" ? "bg-emerald-600/90" : "bg-red-600/90"
          )}
        >
          {notification.text}
        </div>
      )}

      <div className="flex items-center justify-between">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Cost Aggregation</h1>
        </div>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            loadSummary();
            loadBudgets();
            loadResumptionPlans();
            loadWips().then(setWipEntries).catch(() => undefined);
          }}
          className="gap-1.5"
        >
          <RefreshCw className="size-3.5" /> Refresh
        </Button>
      </div>

      <div className="flex gap-1 flex-wrap">
        {tabs.map(({ id, label, icon: Icon }) => (
          <Button
            key={id}
            variant={tab === id ? "default" : "ghost"}
            size="sm"
            onClick={() => setTab(id)}
            className="gap-1.5"
          >
            <Icon className="size-3.5" />
            {label}
          </Button>
        ))}
      </div>

      {tab === "overview" && (
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-4 gap-3">
            <Card className="p-4">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <DollarSign className="size-3.5" /> Total Spend
              </div>
              <p className="text-2xl font-bold">{fmtCost(summary.estimated_total_cost_usd)}</p>
              <p className="text-xs text-muted-foreground mt-1">{fmtTokens(summary.total_tokens)} tokens</p>
            </Card>
            <Card className="p-4">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Flame className="size-3.5" /> Burn Rate
              </div>
              <p className="text-2xl font-bold">{burnRatePct.toFixed(1)}%</p>
              <p className="text-xs text-muted-foreground mt-1">
                {fmtTokens(totalUsedTokens)} of {fmtTokens(totalBudgetTokens)}
              </p>
            </Card>
            <Card className="p-4">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <TrendingUp className="size-3.5" /> Projected Month-End
              </div>
              <p className="text-2xl font-bold">{fmtCost(projectedMonthEnd)}</p>
              <p className="text-xs text-muted-foreground mt-1">USD @ current rate</p>
            </Card>
            <Card className="p-4">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Wallet className="size-3.5" /> Active Budgets
              </div>
              <p className="text-2xl font-bold">{activeBudgets}</p>
              <p className="text-xs text-muted-foreground mt-1">
                {warningBudgets} in warning / critical
              </p>
            </Card>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <ThresholdLadder
              budgets={budgets}
              recentlyFired={recentlyFired}
            />
            <CostBreakdownChart
              budgets={budgets}
              fmtTokens={fmtTokens}
              fmtCost={fmtCost}
            />
          </div>
        </div>
      )}

      {tab === "budgets" && (
        <BudgetList
          budgets={budgets}
          thresholdBudgets={thresholdBudgets}
          recentlyFired={recentlyFired}
          onResume={async (id) => {
            await resumeBudget(id, 100_000);
            await loadBudgets();
            setNotification({ kind: "ok", text: `Budget ${id.slice(0, 8)} +100K tokens` });
          }}
          onCapture={async (id) => {
            const wipPath = `${id.slice(0, 8)}-WIP.md`;
            await captureWip(id, wipPath);
            await loadBudgets();
            const next = await loadWips();
            setWipEntries(next);
            setNotification({ kind: "ok", text: `WIP captured for ${id.slice(0, 8)}` });
          }}
          fmtTokens={fmtTokens}
        />
      )}

      {tab === "wip" && (
        <WipResumptionPanel
          wips={wipEntries}
          resumptionPlan={resumptionPlan}
          onPreviewWip={(p) => {
            setWipPreview({ path: p, content: readMockWip(p) });
          }}
          onPreviewResumption={() => {
            if (resumptionPlan) {
              setResumptionPreview(formatResumptionPreview(resumptionPlan));
            } else {
              setResumptionPreview("# No Resumption Plan\n\nRun a wave that captures WIP to populate this view.");
            }
          }}
        />
      )}

      {tab === "models" && (
        <ModelTable data={summary.by_model} fmtCost={fmtCost} fmtTokens={fmtTokens} />
      )}

      {tab === "projects" && (
        <ProjectTable data={summary.by_project} fmtCost={fmtCost} fmtTokens={fmtTokens} />
      )}

      {tab === "sessions" && (
        <SessionTable data={summary.by_session} fmtCost={fmtCost} fmtTokens={fmtTokens} />
      )}

      {wipPreview && (
        <PreviewDialog
          title={wipPreview.path}
          body={wipPreview.content}
          onClose={() => setWipPreview(null)}
        />
      )}
      {resumptionPreview && (
        <PreviewDialog
          title="Wave Resumption Plan"
          body={resumptionPreview}
          onClose={() => setResumptionPreview(null)}
        />
      )}
    </div>
  );
}

function ThresholdLadder({
  budgets,
  recentlyFired,
}: {
  budgets: import("@/lib/types").AgentBudget[];
  recentlyFired: Set<number>;
}) {
  const totalPct = useMemo(() => {
    if (budgets.length === 0) return 0;
    const totals = budgets.reduce(
      (acc, b) => {
        acc.used += b.budget_used;
        acc.total += b.budget_total;
        return acc;
      },
      { used: 0, total: 0 },
    );
    if (totals.total === 0) return 0;
    return (totals.used / totals.total) * 100;
  }, [budgets]);

  return (
    <Card className="p-4 space-y-3">
      <div className="flex items-center gap-2">
        <Gauge className="size-4 text-indigo-400" />
        <h3 className="text-sm font-medium">Threshold Ladder</h3>
        <Badge variant="outline" className="ml-auto border-indigo-500/30 text-indigo-300 text-[10px]">
          {totalPct.toFixed(1)}% current
        </Badge>
      </div>
      <div className="relative pl-5">
        <div className="absolute left-1.5 top-2 bottom-2 w-px bg-gradient-to-b from-sky-500/40 via-amber-500/40 via-orange-500/40 to-red-500/40" />
        <div className="space-y-3">
          {THRESHOLD_LEVELS.map((level) => {
            const isFired = totalPct >= level.pct;
            const isActive = recentlyFired.has(level.pct);
            return (
              <div key={level.pct} className="relative flex items-center gap-3">
                <div
                  className={cn(
                    "absolute -left-[18px] size-3 rounded-full border-2 transition-all",
                    isFired
                      ? isActive
                        ? "border-white bg-indigo-400 shadow-glow"
                        : "border-indigo-400 bg-indigo-500/40"
                      : "border-glass-border bg-background",
                  )}
                />
                <Badge
                  variant="outline"
                  className={cn("text-[10px] px-1.5 py-0.5 min-w-[44px] justify-center", isFired ? level.tone : "")}
                >
                  {level.label}
                </Badge>
                <span className={cn("text-xs", isFired ? "text-foreground" : "text-muted-foreground")}>
                  {level.severity === "info" && "Awareness prompt"}
                  {level.severity === "warning" && "Wrap up task; plan handoff"}
                  {level.severity === "critical" && "Write WIP_CHECKPOINT.md"}
                  {level.severity === "halt" && "Stop immediately; capture WIP"}
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <p className="text-[11px] text-muted-foreground">
        Live injection — each rung fires exactly once per agent per cycle.
      </p>
    </Card>
  );
}

function CostBreakdownChart({
  budgets,
  fmtTokens,
  fmtCost,
}: {
  budgets: import("@/lib/types").AgentBudget[];
  fmtTokens: (n: number) => string;
  fmtCost: (n: number) => string;
}) {
  const sorted = useMemo(() => {
    return [...budgets].sort((a, b) => b.budget_used - a.budget_used).slice(0, 8);
  }, [budgets]);

  const max = sorted.reduce((m, b) => Math.max(m, b.budget_used), 0);

  if (sorted.length === 0) {
    return (
      <Card className="p-4">
        <div className="flex items-center gap-2 mb-3">
          <BarChart3 className="size-4 text-cyan-400" />
          <h3 className="text-sm font-medium">Cost Breakdown</h3>
        </div>
        <p className="text-xs text-muted-foreground text-center py-8">
          No budget data — allocate budgets to see per-agent breakdown.
        </p>
      </Card>
    );
  }

  return (
    <Card className="p-4 space-y-3">
      <div className="flex items-center gap-2">
        <BarChart3 className="size-4 text-cyan-400" />
        <h3 className="text-sm font-medium">Per-Agent Cost Breakdown</h3>
        <Badge variant="outline" className="ml-auto border-cyan-500/30 text-cyan-300 text-[10px]">
          top {sorted.length}
        </Badge>
      </div>
      <ScrollArea className="max-h-[260px] pr-2">
        <div className="space-y-2">
          {sorted.map((b) => {
            const pct = max > 0 ? (b.budget_used / max) * 100 : 0;
            const stateTone = STATE_TONE[b.state] || STATE_TONE.active;
            return (
              <div key={b.id} className="space-y-1">
                <div className="flex items-center gap-2 text-xs">
                  <span className="font-mono text-foreground truncate max-w-[160px]">{b.agent_id}</span>
                  <Badge variant="outline" className={cn("text-[10px] px-1.5 py-0", stateTone)}>
                    {b.state}
                  </Badge>
                  <span className="ml-auto text-muted-foreground tabular-nums">
                    {fmtTokens(b.budget_used)} / {fmtTokens(b.budget_total)}
                  </span>
                </div>
                <div className="h-1.5 rounded-full bg-glass-20 overflow-hidden">
                  <div
                    className={cn(
                      "h-full rounded-full transition-all",
                      b.state === "exceeded" ? "bg-red-500" :
                      b.state === "critical" ? "bg-orange-500" :
                      b.state === "warning" ? "bg-amber-500" :
                      "bg-gradient-to-r from-cyan-500 to-indigo-500"
                    )}
                    style={{ width: `${Math.max(2, pct)}%` }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      </ScrollArea>
      <p className="text-[11px] text-muted-foreground">
        Bars normalised to highest consumer — figures use the agent_budgets table.
      </p>
      <div className="text-[10px] text-muted-foreground">
        {fmtCost(0)} USD est. — wire get_cost_breakdown_cmd for per-session cost rows.
      </div>
    </Card>
  );
}

function BudgetList({
  budgets,
  thresholdBudgets,
  recentlyFired,
  onResume,
  onCapture,
  fmtTokens,
}: {
  budgets: import("@/lib/types").AgentBudget[];
  thresholdBudgets: import("@/lib/types").AgentBudget[];
  recentlyFired: Set<number>;
  onResume: (id: string) => void;
  onCapture: (id: string) => void;
  fmtTokens: (n: number) => string;
}) {
  const filtered = useMemo(() => {
    if (thresholdBudgets.length === 0) return budgets;
    const ids = new Set(thresholdBudgets.map((b) => b.id));
    const warned = budgets.filter((b) => ids.has(b.id));
    return warned.length > 0 ? warned : budgets;
  }, [budgets, thresholdBudgets]);

  if (filtered.length === 0) {
    return (
      <Card className="p-12 text-center">
        <Wallet className="size-10 mx-auto mb-2 opacity-30" />
        <p className="text-sm text-muted-foreground">No budgets allocated yet.</p>
        <p className="text-xs text-muted-foreground mt-1">
          Create a budget to start tracking token consumption.
        </p>
      </Card>
    );
  }

  return (
    <ScrollArea className="flex-1 max-h-[60vh]">
      <div className="space-y-2 pr-2">
        {filtered.map((b) => (
          <Card key={b.id} className="p-3 space-y-2">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="font-mono text-sm text-foreground">{b.agent_id}</span>
              {b.model && (
                <Badge variant="outline" className="text-[10px] border-glass-border">
                  {b.model}
                </Badge>
              )}
              {b.task_complexity && (
                <Badge variant="outline" className="text-[10px] border-glass-border">
                  {b.task_complexity}
                </Badge>
              )}
              <Badge
                variant="outline"
                className={cn("text-[10px] ml-auto", STATE_TONE[b.state] || STATE_TONE.active)}
              >
                {b.state}
              </Badge>
            </div>
            <div className="flex items-center gap-3 text-xs text-muted-foreground">
              <span>
                <span className="text-foreground font-medium">{fmtTokens(b.budget_used)}</span>
                {" "}of {fmtTokens(b.budget_total)} ({b.usage_percent.toFixed(1)}%)
              </span>
              {b.wip_path && (
                <span className="text-amber-300">
                  WIP: <code className="font-mono">{b.wip_path}</code>
                </span>
              )}
            </div>
            <div className="h-2 rounded-full bg-glass-20 overflow-hidden relative">
              <div
                className={cn(
                  "h-full rounded-full transition-all",
                  b.state === "exceeded" ? "bg-red-500" :
                  b.state === "critical" ? "bg-orange-500" :
                  b.state === "warning" ? "bg-amber-500" :
                  "bg-gradient-to-r from-cyan-500 to-indigo-500"
                )}
                style={{ width: `${Math.min(100, Math.max(2, b.usage_percent))}%` }}
              />
              {THRESHOLD_LEVELS.map((level) => (
                <div
                  key={level.pct}
                  className={cn(
                    "absolute top-0 bottom-0 w-px",
                    recentlyFired.has(level.pct) ? "bg-white shadow-glow" : "bg-glass-border",
                    level.pct === 100 && "w-[2px]"
                  )}
                  style={{ left: `${level.pct}%` }}
                  title={`${level.pct}% threshold`}
                />
              ))}
            </div>
            <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <span>created {new Date(b.created_at).toLocaleDateString()}</span>
              <span>·</span>
              <span>updated {new Date(b.updated_at).toLocaleString()}</span>
              <div className="ml-auto flex gap-1.5">
                {b.state !== "exceeded" && (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 text-[10px] gap-1"
                    onClick={() => onCapture(b.id)}
                  >
                    <FileWarning className="size-3" /> Capture WIP
                  </Button>
                )}
                {b.state === "exceeded" && (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 text-[10px] gap-1 text-cyan-400"
                    onClick={() => onResume(b.id)}
                  >
                    <PlayCircle className="size-3" /> Resume +100K
                  </Button>
                )}
              </div>
            </div>
          </Card>
        ))}
      </div>
    </ScrollArea>
  );
}

function WipResumptionPanel({
  wips,
  resumptionPlan,
  onPreviewWip,
  onPreviewResumption,
}: {
  wips: WipEntry[];
  resumptionPlan: import("@/lib/types").WaveResumptionPlan | null;
  onPreviewWip: (path: string) => void;
  onPreviewResumption: () => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-2">
          <FileWarning className="size-4 text-amber-400" />
          <h3 className="text-sm font-medium">WIP Checkpoints</h3>
          <Badge variant="outline" className="ml-auto border-amber-500/30 text-amber-300 text-[10px]">
            {wips.length}
          </Badge>
        </div>
        {wips.length === 0 ? (
          <p className="text-xs text-muted-foreground text-center py-8">
            No WIP_CHECKPOINT.md files captured. Thresholds at 95%/100% trigger auto-capture.
          </p>
        ) : (
          <ScrollArea className="max-h-[360px] pr-2">
            <div className="space-y-2">
              {wips.map((w) => (
                <div
                  key={w.budget_id}
                  className="border border-glass-border rounded-lg p-2 flex items-center gap-2 hover:border-indigo-500/30 transition-colors"
                >
                  <FileWarning className="size-3.5 text-amber-400 shrink-0" />
                  <div className="flex-1 min-w-0">
                    <p className="text-xs font-mono text-foreground truncate">{w.wip_path}</p>
                    <p className="text-[10px] text-muted-foreground">
                      {w.agent_id} · {new Date(w.updated_at).toLocaleString()}
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 text-[10px] gap-1"
                    onClick={() => onPreviewWip(w.wip_path)}
                  >
                    <Eye className="size-3" /> View
                  </Button>
                </div>
              ))}
            </div>
          </ScrollArea>
        )}
      </Card>

      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-2">
          <BookOpen className="size-4 text-indigo-400" />
          <h3 className="text-sm font-medium">Wave Resumption Plan</h3>
          {resumptionPlan && (
            <Badge variant="outline" className="ml-auto border-indigo-500/30 text-indigo-300 text-[10px]">
              {resumptionPlan.wave_id}
            </Badge>
          )}
        </div>
        {resumptionPlan ? (
          <div className="space-y-2">
            <div className="text-xs text-muted-foreground space-y-1">
              <p>
                <span className="text-foreground">Plan path:</span>{" "}
                <code className="font-mono">{resumptionPlan.plan_path}</code>
              </p>
              {resumptionPlan.pending_task_id && (
                <p>
                  <span className="text-foreground">Pending task:</span>{" "}
                  <code className="font-mono">{resumptionPlan.pending_task_id}</code>
                </p>
              )}
              {resumptionPlan.estimated_remaining_tokens != null && (
                <p>
                  <span className="text-foreground">Remaining:</span>{" "}
                  {resumptionPlan.estimated_remaining_tokens.toLocaleString()} tokens
                </p>
              )}
            </div>
            <Separator className="bg-glass-border" />
            <div className="grid grid-cols-3 gap-2 text-[11px]">
              <AgentList label="Completed" items={resumptionPlan.agents_completed} tone="text-emerald-300" />
              <AgentList label="WIP'd" items={resumptionPlan.agents_wipd} tone="text-amber-300" />
              <AgentList label="Pending" items={resumptionPlan.agents_pending} tone="text-cyan-300" />
            </div>
            <Button
              size="sm"
              variant="ghost"
              className="h-7 text-xs gap-1.5 w-full justify-center"
              onClick={onPreviewResumption}
            >
              <ListChecks className="size-3" /> View Full Plan
            </Button>
          </div>
        ) : (
          <p className="text-xs text-muted-foreground text-center py-8">
            No resumption plan yet. Create one from accumulated WIPs.
          </p>
        )}
      </Card>
    </div>
  );
}

function AgentList({
  label,
  items,
  tone,
}: {
  label: string;
  items?: string;
  tone: string;
}) {
  if (!items) return <div className="space-y-1"><p className="text-muted-foreground">{label}</p><p className="text-[10px] text-muted-foreground">—</p></div>;
  const list = items.split(",").filter(Boolean);
  return (
    <div className="space-y-1">
      <p className={cn("font-medium", tone)}>{label}</p>
      {list.length === 0 ? (
        <p className="text-[10px] text-muted-foreground">—</p>
      ) : (
        <ul className="space-y-0.5">
          {list.map((a) => (
            <li key={a} className="text-[10px] font-mono text-foreground flex items-center gap-1">
              <ChevronRight className="size-2.5 text-muted-foreground" /> {a}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function PreviewDialog({
  title,
  body,
  onClose,
}: {
  title: string;
  body: string;
  onClose: () => void;
}) {
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-6">
      <Card className="w-[720px] max-w-[90vw] max-h-[80vh] flex flex-col bg-[#0d1117] border-glass-border">
        <div className="flex items-center justify-between p-3 border-b border-glass-border">
          <span className="text-sm font-medium text-foreground truncate">{title}</span>
          <Button size="sm" variant="ghost" className="h-6 text-xs" onClick={onClose}>
            Close
          </Button>
        </div>
        <ScrollArea className="flex-1 p-3">
          <pre className="text-xs font-mono text-foreground whitespace-pre-wrap leading-relaxed">
            {body}
          </pre>
        </ScrollArea>
      </Card>
    </div>
  );
}

function readMockWip(path: string): string {
  return `# WIP_CHECKPOINT — ${path}

## Status
Auto-captured preview. The real WIP file lives at the path above.

## Last Known State
- Active file edits: see HANDOFF_*.md entries
- Remaining tasks: derive from latest HANDOFF
- Open signals: see ACB inbox

## How to Resume
1. Read HANDOFF_<last>.md for next-agent context
2. Open the resumption plan in the WIP / Resumption tab
3. Spawn a fresh session with the resumptive guideline
`;
}

function formatResumptionPreview(plan: import("@/lib/types").WaveResumptionPlan): string {
  const lines: string[] = [];
  lines.push(`# Wave Resumption Plan — ${plan.wave_id}`);
  lines.push("");
  lines.push(`Plan path: ${plan.plan_path}`);
  if (plan.pending_task_id) lines.push(`Pending task: ${plan.pending_task_id}`);
  if (plan.estimated_remaining_tokens != null) {
    lines.push(`Estimated remaining tokens: ${plan.estimated_remaining_tokens.toLocaleString()}`);
  }
  lines.push("");
  lines.push("## Agent Status");
  lines.push(`- Completed: ${plan.agents_completed || "—"}`);
  lines.push(`- WIP'd: ${plan.agents_wipd || "—"}`);
  lines.push(`- Pending: ${plan.agents_pending || "—"}`);
  lines.push("");
  lines.push(`Created: ${new Date(plan.created_at).toLocaleString()}`);
  return lines.join("\n");
}

function ModelTable({ data, fmtCost, fmtTokens }: { data: ModelCostSummary[]; fmtCost: (n: number) => string; fmtTokens: (n: number) => string }) {
  if (data.length === 0) return <EmptyState message="No model usage data yet." />;
  return (
    <Card className="overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b bg-muted/50">
            <th className="px-4 py-2 text-left font-medium">Model</th>
            <th className="px-4 py-2 text-right font-medium">Tokens In</th>
            <th className="px-4 py-2 text-right font-medium">Tokens Out</th>
            <th className="px-4 py-2 text-right font-medium">Sessions</th>
            <th className="px-4 py-2 text-right font-medium">Est. Cost</th>
          </tr>
        </thead>
        <tbody>
          {data.map((m) => (
            <tr key={m.model} className="border-b">
              <td className="px-4 py-2 font-medium">{m.model}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(m.tokens_in)}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(m.tokens_out)}</td>
              <td className="px-4 py-2 text-right">{m.sessions}</td>
              <td className="px-4 py-2 text-right">{fmtCost(m.estimated_cost_usd)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function ProjectTable({ data, fmtCost, fmtTokens }: { data: ProjectCostSummary[]; fmtCost: (n: number) => string; fmtTokens: (n: number) => string }) {
  if (data.length === 0) return <EmptyState message="No project usage data yet." />;
  return (
    <Card className="overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b bg-muted/50">
            <th className="px-4 py-2 text-left font-medium">Project</th>
            <th className="px-4 py-2 text-right font-medium">Tokens In</th>
            <th className="px-4 py-2 text-right font-medium">Tokens Out</th>
            <th className="px-4 py-2 text-right font-medium">Sessions</th>
            <th className="px-4 py-2 text-right font-medium">Est. Cost</th>
          </tr>
        </thead>
        <tbody>
          {data.map((p) => (
            <tr key={p.project_id} className="border-b">
              <td className="px-4 py-2 font-medium">{p.project_name || p.project_id}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(p.tokens_in)}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(p.tokens_out)}</td>
              <td className="px-4 py-2 text-right">{p.sessions}</td>
              <td className="px-4 py-2 text-right">{fmtCost(p.estimated_cost_usd)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function SessionTable({ data, fmtCost, fmtTokens }: { data: SessionCostSummary[]; fmtCost: (n: number) => string; fmtTokens: (n: number) => string }) {
  if (data.length === 0) return <EmptyState message="No session data yet." />;
  return (
    <Card className="overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b bg-muted/50">
            <th className="px-4 py-2 text-left font-medium">Session</th>
            <th className="px-4 py-2 text-left font-medium">Agent</th>
            <th className="px-4 py-2 text-right font-medium">Tokens In</th>
            <th className="px-4 py-2 text-right font-medium">Tokens Out</th>
            <th className="px-4 py-2 text-right font-medium">Est. Cost</th>
          </tr>
        </thead>
        <tbody>
          {data.map((s) => (
            <tr key={s.session_id} className="border-b">
              <td className="px-4 py-2 font-mono text-xs">{s.session_id.slice(0, 8)}...</td>
              <td className="px-4 py-2">{s.agent_id || "-"}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(s.tokens_in)}</td>
              <td className="px-4 py-2 text-right">{fmtTokens(s.tokens_out)}</td>
              <td className="px-4 py-2 text-right">{fmtCost(s.estimated_cost_usd)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <Card className="p-8 text-center">
      <Zap className="size-8 text-muted-foreground mx-auto mb-2" />
      <p className="text-sm text-muted-foreground">{message}</p>
    </Card>
  );
}
