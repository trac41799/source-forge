import { useState, useEffect, useMemo, useCallback } from "react";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import {
  Clock4,
  Plus,
  Play,
  Pause,
  Trash2,
  Edit,
  RotateCcw,
  X,
  Check,
  AlertTriangle,
  Zap,
  CalendarClock,
  History,
  ListChecks,
} from "lucide-react";
import {
  useSchedulerStore,
  type CronJob,
  type CronJobInput,
} from "@/stores/schedulerStore";

type TabId = "jobs" | "history" | "escalations";

const STATUS_COLORS: Record<string, string> = {
  running: "text-[#58a6ff] bg-[#58a6ff]/20 border-[#58a6ff]/30",
  success: "text-green-400 bg-green-500/20 border-green-500/30",
  completed: "text-green-400 bg-green-500/20 border-green-500/30",
  failed: "text-red-400 bg-red-500/20 border-red-500/30",
  escalated: "text-yellow-400 bg-yellow-500/20 border-yellow-500/30",
  scheduled: "text-gray-400 bg-gray-500/20 border-gray-500/30",
};

const CRON_PRESETS: Array<{ label: string; expr: string }> = [
  { label: "Every minute", expr: "* * * * *" },
  { label: "Every 5 min", expr: "*/5 * * * *" },
  { label: "Every 15 min", expr: "*/15 * * * *" },
  { label: "Every 30 min", expr: "*/30 * * * *" },
  { label: "Every hour", expr: "0 * * * *" },
  { label: "Every 6 hours", expr: "0 */6 * * *" },
  { label: "Daily 9am", expr: "0 9 * * *" },
  { label: "Daily midnight", expr: "0 0 * * *" },
  { label: "Weekdays 9am", expr: "0 9 * * 1-5" },
  { label: "Weekly Mon 9am", expr: "0 9 * * 1" },
];

const WAVE_PRESETS = ["default-4-wave", "acp-execution-spine", "nightly-audit"] as const;
const RETRY_DELAY_PRESETS = [15, 30, 60] as const;

function describeCron(expr: string): string {
  const trimmed = expr.trim();
  if (!trimmed) return "No schedule set";
  const parts = trimmed.split(/\s+/);
  if (parts.length !== 5) return `Custom: ${trimmed}`;
  const [minute, hour, dom, month, dow] = parts;
  const fmtField = (val: string, max: number): string => {
    if (val === "*") return "any";
    if (val.startsWith("*/")) return `every ${val.slice(2)}`;
    if (val.includes(",")) return `at ${val.split(",").join(", ")}`;
    if (val.includes("-")) {
      const [s, e] = val.split("-");
      return `from ${s} to ${e}`;
    }
    const n = parseInt(val, 10);
    if (!isNaN(n) && n >= 0 && n <= max) return `at ${val}`;
    return val;
  };
  const minuteDesc = fmtField(minute, 59);
  const hourDesc = hour === "*" ? "every hour" : fmtField(hour, 23);
  const domDesc = dom === "*" ? "" : `on day ${dom}`;
  const monthDesc = month === "*" ? "" : `in ${month}`;
  const dowDesc = dow === "*" ? "" : `on ${dow}`;
  return [minuteDesc, hourDesc, domDesc, monthDesc, dowDesc]
    .filter(Boolean)
    .join(" ");
}

const EMPTY_JOB: Partial<CronJobInput> = {
  name: "",
  description: "",
  schedule: "0 */6 * * *",
  task_template: "",
  wave_preset: "",
  auto_approve: false,
  escalation_policy: "",
  max_correction_retries: 3,
  enabled: true,
};

export default function Scheduler() {
  const store = useSchedulerStore();
  const {
    jobs,
    executions,
    escalations,
    lastFired,
    lastEscalation,
    loading,
    error,
    loadJobs,
    createJob,
    updateJob,
    deleteJob,
    toggleJob,
    runNow,
    getExecutions,
    evaluateSchedule,
    loadEscalations,
    clearError,
    attachEventListeners,
    detachEventListeners,
  } = store;

  const [tab, setTab] = useState<TabId>("jobs");
  const [showDialog, setShowDialog] = useState(false);
  const [editingJob, setEditingJob] = useState<CronJob | null>(null);
  const [form, setForm] = useState<Partial<CronJobInput>>({ ...EMPTY_JOB });
  const [historyFilter, setHistoryFilter] = useState("");
  const [historyJobId, setHistoryJobId] = useState<string | null>(null);
  const [notification, setNotification] = useState<{
    kind: "ok" | "err" | "warn";
    text: string;
  } | null>(null);
  const [saving, setSaving] = useState(false);

  const retryDelayMinutes = useMemo(() => {
    try {
      const parsed = JSON.parse(form.escalation_policy || "{}");
      if (
        typeof parsed === "object" &&
        parsed !== null &&
        !Array.isArray(parsed) &&
        typeof parsed.retry_delay_minutes === "number"
      ) {
        return parsed.retry_delay_minutes;
      }
    } catch {
      // Ignore invalid JSON while typing
    }
    return 30;
  }, [form.escalation_policy]);

  const updateRetryDelay = useCallback((minutes: number) => {
    const safeMinutes = Math.max(1, Number.isFinite(minutes) ? minutes : 30);
    setForm((f) => {
      let base: Record<string, unknown> = {
        max_retries: f.max_correction_retries ?? 3,
        notify_on_escalation: true,
      };
      try {
        const existing = JSON.parse(f.escalation_policy || "{}");
        if (typeof existing === "object" && existing !== null && !Array.isArray(existing)) {
          base = { ...existing };
        }
      } catch {
        // Fall back to default object when textarea text is not yet valid JSON
      }
      base.retry_delay_minutes = safeMinutes;
      return {
        ...f,
        escalation_policy: JSON.stringify(base, null, 2),
      };
    });
  }, []);

  useEffect(() => {
    loadJobs();
    loadEscalations();
  }, [loadJobs, loadEscalations]);

  useEffect(() => {
    let unlistens: Array<() => void> = [];
    let cancelled = false;
    (async () => {
      const handles = await attachEventListeners();
      if (cancelled) {
        detachEventListeners(handles);
      } else {
        unlistens = handles;
      }
    })();
    return () => {
      cancelled = true;
      if (unlistens.length) detachEventListeners(unlistens);
    };
  }, [attachEventListeners, detachEventListeners]);

  useEffect(() => {
    if (form.schedule) {
      const t = setTimeout(() => {
        evaluateSchedule(form.schedule!, 3).catch(() => {});
      }, 300);
      return () => clearTimeout(t);
    }
    return undefined;
  }, [form.schedule, evaluateSchedule]);

  useEffect(() => {
    if (tab === "history") {
      getExecutions(historyJobId ?? undefined);
    }
  }, [tab, historyJobId, getExecutions]);

  useEffect(() => {
    if (lastFired) {
      setNotification({
        kind: "ok",
        text: `Cron fired: job ${lastFired.job_id.slice(0, 8)} → execution ${lastFired.execution_id.slice(0, 8)}`,
      });
    }
  }, [lastFired]);

  useEffect(() => {
    if (lastEscalation) {
      setNotification({
        kind: "warn",
        text: `Escalation: job ${lastEscalation.job_id.slice(0, 8)} failed ${lastEscalation.failure_count}× in last hour`,
      });
    }
  }, [lastEscalation]);

  useEffect(() => {
    if (!notification) return;
    const t = setTimeout(() => setNotification(null), 4500);
    return () => clearTimeout(t);
  }, [notification]);

  useEffect(() => {
    if (error) {
      setNotification({ kind: "err", text: error });
      clearError();
    }
  }, [error, clearError]);

  const openCreate = useCallback(() => {
    setEditingJob(null);
    setForm({ ...EMPTY_JOB });
    setShowDialog(true);
  }, []);

  const openEdit = useCallback((job: CronJob) => {
    setEditingJob(job);
    setForm({
      name: job.name,
      description: job.description ?? "",
      project_id: job.project_id,
      schedule: job.schedule,
      task_template: job.task_template,
      wave_preset: job.wave_preset ?? "",
      auto_approve: job.auto_approve,
      escalation_policy: job.escalation_policy,
      notification_channels: job.notification_channels,
      max_correction_retries: job.max_correction_retries,
      enabled: job.enabled,
    });
    setShowDialog(true);
  }, []);

  const closeDialog = useCallback(() => {
    setShowDialog(false);
    setEditingJob(null);
    setForm({ ...EMPTY_JOB });
  }, []);

  const handleSave = useCallback(async () => {
    if (!form.name?.trim() || !form.schedule?.trim()) {
      setNotification({ kind: "err", text: "Name and schedule are required" });
      return;
    }
    setSaving(true);
    try {
      const payload: CronJobInput = {
        name: form.name.trim(),
        description: form.description?.trim() || undefined,
        project_id: form.project_id,
        schedule: form.schedule.trim(),
        task_template: form.task_template ?? "",
        wave_preset: form.wave_preset?.trim() || undefined,
        auto_approve: form.auto_approve,
        escalation_policy:
          form.escalation_policy?.trim() ||
          JSON.stringify({
            max_retries: 3,
            retry_delay_minutes: 30,
            notify_on_escalation: true,
            escalation_channels: ["system"],
          }),
        notification_channels: form.notification_channels,
        max_correction_retries: form.max_correction_retries,
        enabled: form.enabled,
      };
      if (editingJob) {
        await updateJob(editingJob.id, payload);
        setNotification({ kind: "ok", text: `Updated "${payload.name}"` });
      } else {
        await createJob(payload);
        setNotification({ kind: "ok", text: `Created "${payload.name}"` });
      }
      closeDialog();
      await loadJobs();
    } catch (e) {
      setNotification({
        kind: "err",
        text: `Save failed: ${e instanceof Error ? e.message : String(e)}`,
      });
    } finally {
      setSaving(false);
    }
  }, [form, editingJob, createJob, updateJob, closeDialog, loadJobs]);

  const handleToggle = useCallback(
    async (id: string, currentEnabled: boolean) => {
      try {
        await toggleJob(id, !currentEnabled);
        await loadJobs();
      } catch (e) {
        setNotification({
          kind: "err",
          text: `Toggle failed: ${e instanceof Error ? e.message : String(e)}`,
        });
      }
    },
    [toggleJob, loadJobs],
  );

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      if (!window.confirm(`Delete cron job "${name}"? This cannot be undone.`)) return;
      try {
        await deleteJob(id);
        setNotification({ kind: "ok", text: `Deleted "${name}"` });
        await loadJobs();
      } catch (e) {
        setNotification({
          kind: "err",
          text: `Delete failed: ${e instanceof Error ? e.message : String(e)}`,
        });
      }
    },
    [deleteJob, loadJobs],
  );

  const handleRunNow = useCallback(
    async (id: string, name: string) => {
      try {
        const execution = await runNow(id);
        if (execution) {
          setNotification({
            kind: "ok",
            text: `Recorded execution for "${name}" (${execution.id.slice(0, 8)})`,
          });
        }
      } catch (e) {
        setNotification({
          kind: "err",
          text: `Run-now failed: ${e instanceof Error ? e.message : String(e)}`,
        });
      }
    },
    [runNow],
  );

  const filteredExecutions = useMemo(() => {
    return executions.filter((e) => {
      if (historyJobId && e.cron_job_id !== historyJobId) return false;
      if (!historyFilter) return true;
      return e.cron_job_id.includes(historyFilter);
    });
  }, [executions, historyFilter, historyJobId]);

  const getJobName = useCallback(
    (jobId: string) => {
      const job = jobs.find((j) => j.id === jobId);
      return job?.name || jobId.slice(0, 8);
    },
    [jobs],
  );

  const tabCounts = useMemo(() => {
    return {
      jobs: jobs.length,
      history: executions.length,
      escalations: escalations.length,
    };
  }, [jobs.length, executions.length, escalations.length]);

  return (
    <div className="flex h-full flex-col p-6 gap-4">
      {notification && (
        <div
          className={cn(
            "fixed top-4 right-4 z-50 px-4 py-2 text-white text-sm rounded-lg shadow-lg animate-in fade-in",
            notification.kind === "ok" && "bg-emerald-600/90",
            notification.kind === "warn" && "bg-yellow-600/90",
            notification.kind === "err" && "bg-red-600/90",
          )}
        >
          {notification.text}
        </div>
      )}

      <div className="flex items-center justify-between">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Scheduler</h1>
        </div>
        <div className="flex items-center gap-2">
          {lastFired && (
            <Badge
              variant="outline"
              className="border-[#58a6ff]/30 text-[#58a6ff] bg-[#58a6ff]/10 text-xs gap-1"
            >
              <Zap className="size-3" /> Last fired {new Date(lastFired.timestamp).toLocaleTimeString()}
            </Badge>
          )}
          <Button variant="ghost" size="sm" onClick={() => { loadJobs(); loadEscalations(); }} className="gap-1.5">
            <RotateCcw className="size-3.5" /> Refresh
          </Button>
        </div>
      </div>

      {escalations.length > 0 && (
        <Card className="p-3 border-yellow-500/30 bg-yellow-500/10">
          <div className="flex items-start gap-3">
            <AlertTriangle className="size-5 text-yellow-400 flex-shrink-0 mt-0.5" />
            <div className="flex-1">
              <div className="text-sm font-medium text-yellow-200">
                {escalations.length} job{escalations.length === 1 ? "" : "s"} flagged for escalation
              </div>
              <div className="text-xs text-yellow-300/70 mt-1 space-y-0.5">
                {escalations.slice(0, 3).map((j) => (
                  <div key={j.id}>
                    • <span className="font-mono">{j.name}</span> — 2+ failures in the last hour
                  </div>
                ))}
                {escalations.length > 3 && (
                  <div className="text-yellow-400/60">
                    +{escalations.length - 3} more…
                  </div>
                )}
              </div>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setTab("escalations")}
              className="border-yellow-500/30 text-yellow-300 hover:bg-yellow-500/20"
            >
              View
            </Button>
          </div>
        </Card>
      )}

      <div className="flex gap-1 p-1 rounded-lg bg-secondary/50 w-fit">
        {(["jobs", "history", "escalations"] as TabId[]).map((t) => {
          const icons: Record<TabId, React.ReactNode> = {
            jobs: <ListChecks className="size-3.5" />,
            history: <History className="size-3.5" />,
            escalations: <AlertTriangle className="size-3.5" />,
          };
          const labels: Record<TabId, string> = {
            jobs: "Jobs",
            history: "History",
            escalations: "Escalations",
          };
          return (
            <Button
              key={t}
              variant={tab === t ? "default" : "ghost"}
              size="sm"
              onClick={() => setTab(t)}
              className="gap-1.5"
            >
              {icons[t]} {labels[t]}
              <span className="ml-1 text-[10px] text-muted-foreground">
                {tabCounts[t]}
              </span>
            </Button>
          );
        })}
      </div>

      {tab === "jobs" && (
        <div className="flex flex-col gap-4 flex-1 min-h-0">
          <div className="flex items-center gap-2">
            <Button onClick={openCreate} className="gap-1.5">
              <Plus className="size-4" /> New Job
            </Button>
            {loading && <span className="text-xs text-muted-foreground">Loading…</span>}
          </div>

          <ScrollArea className="flex-1">
            <div className="space-y-2 pr-2">
              {jobs.length === 0 && !loading && (
                <div className="py-12 text-center text-muted-foreground">
                  <Clock4 className="size-10 mx-auto mb-2 opacity-30" />
                  <p className="text-sm">No scheduled jobs yet.</p>
                  <p className="text-xs text-muted-foreground/60 mt-1">
                    Create your first cron job to automate agent tasks.
                  </p>
                </div>
              )}
              {jobs.map((job) => (
                <Card key={job.id} className="p-3 space-y-2">
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-medium text-sm truncate">{job.name}</span>
                        <Badge
                          variant="outline"
                          className={cn(
                            "text-xs",
                            job.enabled
                              ? "border-green-500/30 text-green-400"
                              : "border-red-500/30 text-red-400",
                          )}
                        >
                          {job.enabled ? "Enabled" : "Disabled"}
                        </Badge>
                        {job.auto_approve && (
                          <Badge variant="outline" className="text-xs border-blue-500/30 text-blue-400">
                            Auto-approve
                          </Badge>
                        )}
                      </div>
                      {job.description && (
                        <p className="text-xs text-muted-foreground mt-0.5">{job.description}</p>
                      )}
                    </div>
                    <div className="flex gap-1 flex-shrink-0">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 w-7 p-0"
                        onClick={() => handleToggle(job.id, job.enabled)}
                        title={job.enabled ? "Disable" : "Enable"}
                      >
                        {job.enabled ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 w-7 p-0"
                        onClick={() => handleRunNow(job.id, job.name)}
                        title="Record run now"
                      >
                        <RotateCcw className="size-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 w-7 p-0"
                        onClick={() => openEdit(job)}
                        title="Edit"
                      >
                        <Edit className="size-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 w-7 p-0 hover:text-red-400"
                        onClick={() => handleDelete(job.id, job.name)}
                        title="Delete"
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    </div>
                  </div>
                  <Separator />
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-xs">
                    <div>
                      <div className="text-muted-foreground text-[10px] uppercase tracking-wide mb-0.5">
                        Schedule
                      </div>
                      <code className="bg-secondary px-1.5 py-0.5 rounded text-[11px]">
                        {job.schedule}
                      </code>
                      <div className="text-muted-foreground/70 mt-1 text-[11px]">
                        {describeCron(job.schedule)}
                      </div>
                    </div>
                    <div>
                      <div className="text-muted-foreground text-[10px] uppercase tracking-wide mb-0.5">
                        <CalendarClock className="size-3 inline mr-1" />Last run
                      </div>
                      <div className="text-foreground">
                        {job.last_run_at
                          ? new Date(job.last_run_at).toLocaleString()
                          : "Never"}
                      </div>
                      {job.next_run_at && job.enabled && (
                        <div className="text-muted-foreground/70 text-[11px] mt-0.5">
                          Next: {new Date(job.next_run_at).toLocaleString()}
                        </div>
                      )}
                    </div>
                    <div>
                      <div className="text-muted-foreground text-[10px] uppercase tracking-wide mb-0.5">
                        Wave preset
                      </div>
                      <div className="text-foreground">
                        {job.wave_preset || "—"}
                      </div>
                      <div className="text-muted-foreground/70 text-[11px] mt-0.5">
                        Max retries: {job.max_correction_retries}
                      </div>
                    </div>
                  </div>
                  {job.task_template && (
                    <div className="text-xs">
                      <div className="text-muted-foreground text-[10px] uppercase tracking-wide mb-0.5">
                        Task template
                      </div>
                      <div className="text-foreground/80 font-mono text-[11px] bg-secondary/50 rounded p-1.5">
                        {job.task_template}
                      </div>
                    </div>
                  )}
                </Card>
              ))}
            </div>
          </ScrollArea>
        </div>
      )}

      {tab === "history" && (
        <div className="flex flex-col gap-4 flex-1 min-h-0">
          <div className="flex flex-wrap gap-2 items-center">
            <Input
              placeholder="Filter by job id…"
              value={historyFilter}
              onChange={(e) => setHistoryFilter(e.target.value)}
              className="w-64"
            />
            <select
              value={historyJobId ?? ""}
              onChange={(e) => setHistoryJobId(e.target.value || null)}
              className="h-9 rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="">All jobs</option>
              {jobs.map((j) => (
                <option key={j.id} value={j.id}>
                  {j.name}
                </option>
              ))}
            </select>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => getExecutions(historyJobId ?? undefined)}
              className="gap-1.5"
            >
              <RotateCcw className="size-3.5" /> Refresh
            </Button>
          </div>

          <ScrollArea className="flex-1">
            <div className="space-y-1 pr-2">
              {filteredExecutions.length === 0 && (
                <div className="py-12 text-center text-muted-foreground">
                  <History className="size-10 mx-auto mb-2 opacity-30" />
                  <p className="text-sm">No execution history yet.</p>
                  <p className="text-xs text-muted-foreground/60 mt-1">
                    Run a job or wait for the cron to fire.
                  </p>
                </div>
              )}
              {filteredExecutions.map((exec) => {
                const statusColor = STATUS_COLORS[exec.status] || STATUS_COLORS.scheduled;
                const duration =
                  exec.completed_at
                    ? Math.round(
                        (new Date(exec.completed_at).getTime() -
                          new Date(exec.started_at).getTime()) /
                          1000,
                      ) + "s"
                    : "—";
                return (
                  <Card key={exec.id} className="p-2.5 flex items-center gap-3 text-xs">
                    <Badge variant="outline" className={cn("text-xs capitalize", statusColor)}>
                      {exec.status}
                    </Badge>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium truncate">
                        {getJobName(exec.cron_job_id)}
                      </div>
                      <div className="text-muted-foreground text-[11px]">
                        Started {new Date(exec.started_at).toLocaleString()}
                        {exec.completed_at && (
                          <> · completed {new Date(exec.completed_at).toLocaleString()}</>
                        )}
                      </div>
                      {exec.escalation_reason && (
                        <div className="text-red-400/80 text-[11px] mt-0.5 truncate">
                          {exec.escalation_reason}
                        </div>
                      )}
                    </div>
                    <div className="text-muted-foreground text-[11px] font-mono flex-shrink-0">
                      {duration}
                    </div>
                  </Card>
                );
              })}
            </div>
          </ScrollArea>
        </div>
      )}

      {tab === "escalations" && (
        <div className="flex flex-col gap-4 flex-1 min-h-0">
          <div className="text-sm text-muted-foreground">
            Jobs with 2+ failed executions in the last hour. The last{" "}
            <code className="text-xs">cron-escalated</code> event is highlighted.
          </div>
          <ScrollArea className="flex-1">
            <div className="space-y-2 pr-2">
              {escalations.length === 0 && (
                <div className="py-12 text-center text-muted-foreground">
                  <AlertTriangle className="size-10 mx-auto mb-2 opacity-30" />
                  <p className="text-sm">No active escalations.</p>
                  <p className="text-xs text-muted-foreground/60 mt-1">
                    All cron jobs are within their failure threshold.
                  </p>
                </div>
              )}
              {escalations.map((job) => {
                const isLatest =
                  lastEscalation && lastEscalation.job_id === job.id;
                return (
                  <Card
                    key={job.id}
                    className={cn(
                      "p-3 space-y-2",
                      isLatest
                        ? "border-yellow-500/50 bg-yellow-500/5"
                        : "border-yellow-500/20",
                    )}
                  >
                    <div className="flex items-start gap-3">
                      <AlertTriangle className="size-4 text-yellow-400 flex-shrink-0 mt-0.5" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm">{job.name}</span>
                          {isLatest && (
                            <Badge
                              variant="outline"
                              className="text-xs border-yellow-500/40 text-yellow-300"
                            >
                              Latest
                            </Badge>
                          )}
                        </div>
                        <code className="text-[11px] bg-secondary px-1.5 py-0.5 rounded mt-1 inline-block">
                          {job.schedule}
                        </code>
                        <div className="text-xs text-muted-foreground mt-1">
                          Escalated — system notification dispatched.
                        </div>
                        {isLatest && lastEscalation && (
                          <div className="text-[11px] text-red-300/80 mt-1 font-mono bg-red-500/10 rounded p-1.5">
                            {lastEscalation.error}
                          </div>
                        )}
                      </div>
                      <div className="flex gap-1 flex-shrink-0">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0"
                          onClick={() => openEdit(job)}
                          title="Edit"
                        >
                          <Edit className="size-3.5" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0"
                          onClick={() => handleToggle(job.id, job.enabled)}
                          title={job.enabled ? "Disable" : "Enable"}
                        >
                          <Pause className="size-3.5" />
                        </Button>
                      </div>
                    </div>
                  </Card>
                );
              })}
            </div>
          </ScrollArea>
        </div>
      )}

      {showDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <Card className="w-full max-w-2xl p-6 space-y-4 max-h-[90vh] overflow-auto">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-semibold">
                {editingJob ? "Edit Job" : "Create Job"}
              </h2>
              <Button variant="ghost" size="sm" className="h-7 w-7 p-0" onClick={closeDialog}>
                <X className="size-4" />
              </Button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="block text-xs text-muted-foreground mb-1">Name *</label>
                <Input
                  value={form.name || ""}
                  onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                  placeholder="e.g., Daily Knowledge Sync"
                />
              </div>

              <div>
                <label className="block text-xs text-muted-foreground mb-1">Description</label>
                <Input
                  value={form.description || ""}
                  onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
                  placeholder="What does this job do?"
                />
              </div>

              <div>
                <label className="block text-xs text-muted-foreground mb-1">Schedule (Cron) *</label>
                <Input
                  value={form.schedule || ""}
                  onChange={(e) => setForm((f) => ({ ...f, schedule: e.target.value }))}
                  placeholder="0 */6 * * *"
                  className="font-mono"
                />
                <div className="text-[11px] text-muted-foreground mt-1">
                  {describeCron(form.schedule || "")}
                </div>
                <div className="flex flex-wrap gap-1 mt-2">
                  {CRON_PRESETS.map((p) => (
                    <Badge
                      key={p.expr}
                      variant="outline"
                      className="cursor-pointer text-[10px] border-[#30363d] text-muted-foreground hover:border-[#58a6ff]/50 hover:text-[#58a6ff]"
                      onClick={() => setForm((f) => ({ ...f, schedule: p.expr }))}
                    >
                      {p.label}
                    </Badge>
                  ))}
                </div>
                {store.nextRuns.length > 0 && (
                  <div className="text-[11px] text-muted-foreground mt-2">
                    <span className="text-muted-foreground/80">Next 3 fires:</span>{" "}
                    {store.nextRuns.map((r) => new Date(r).toLocaleString()).join(" · ")}
                  </div>
                )}
              </div>

              <div>
                <label className="block text-xs text-muted-foreground mb-1">Task Template</label>
                <textarea
                  className="w-full min-h-[80px] rounded-md border border-input bg-background px-3 py-2 text-sm font-mono resize-y"
                  value={form.task_template || ""}
                  onChange={(e) => setForm((f) => ({ ...f, task_template: e.target.value }))}
                  placeholder="Run knowledge extraction for the project and synthesize."
                />
              </div>

              <div>
                <label className="block text-xs text-muted-foreground mb-1">Wave Preset</label>
                <Input
                  value={form.wave_preset || ""}
                  onChange={(e) => setForm((f) => ({ ...f, wave_preset: e.target.value }))}
                  placeholder="e.g., default-4-wave"
                />
                <div className="flex flex-wrap gap-1 mt-2">
                  {WAVE_PRESETS.map((preset) => (
                    <button
                      key={preset}
                      type="button"
                      onClick={() => setForm((f) => ({ ...f, wave_preset: preset }))}
                      className={cn(
                        "inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-mono transition-colors",
                        form.wave_preset === preset
                          ? "border-[#58a6ff] bg-[#1f6feb]/20 text-[#58a6ff]"
                          : "border-[#30363d] text-muted-foreground hover:border-[#58a6ff]/50 hover:text-[#58a6ff]"
                      )}
                    >
                      {preset}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <label className="block text-xs text-muted-foreground mb-1">Escalation Policy (JSON)</label>
                <div className="flex items-center gap-3 mb-2 flex-wrap">
                  <div className="flex items-center gap-2">
                    <label htmlFor="scheduler-retry-delay" className="text-xs text-muted-foreground">
                      Retry Delay (min)
                    </label>
                    <Input
                      id="scheduler-retry-delay"
                      type="number"
                      min={1}
                      className="w-20 h-8 text-xs"
                      value={retryDelayMinutes}
                      onChange={(e) => updateRetryDelay(parseInt(e.target.value, 10) || 1)}
                    />
                  </div>
                  <div className="flex items-center gap-1">
                    {RETRY_DELAY_PRESETS.map((minutes) => (
                      <button
                        key={minutes}
                        type="button"
                        onClick={() => updateRetryDelay(minutes)}
                        className={cn(
                          "inline-flex items-center rounded-md border px-2 py-0.5 text-[11px] font-mono transition-colors",
                          retryDelayMinutes === minutes
                            ? "border-[#58a6ff] bg-[#1f6feb]/20 text-[#58a6ff]"
                            : "border-[#30363d] text-muted-foreground hover:border-[#58a6ff]/50 hover:text-[#58a6ff]"
                        )}
                      >
                        {minutes}m
                      </button>
                    ))}
                  </div>
                </div>
                <textarea
                  className="w-full min-h-[80px] rounded-md border border-input bg-background px-3 py-2 text-sm font-mono resize-y"
                  value={form.escalation_policy || ""}
                  onChange={(e) => setForm((f) => ({ ...f, escalation_policy: e.target.value }))}
                  placeholder='{"max_retries": 3, "retry_delay_minutes": 30}'
                />
              </div>

              <div className="flex items-center gap-4 flex-wrap">
                <div>
                  <label className="block text-xs text-muted-foreground mb-1">Max Retries</label>
                  <Input
                    type="number"
                    className="w-24"
                    value={form.max_correction_retries ?? 3}
                    onChange={(e) =>
                      setForm((f) => ({
                        ...f,
                        max_correction_retries: parseInt(e.target.value) || 0,
                      }))
                    }
                  />
                </div>

                <label className="flex items-center gap-2 pt-5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.auto_approve || false}
                    onChange={(e) => setForm((f) => ({ ...f, auto_approve: e.target.checked }))}
                    className="rounded border-border"
                  />
                  <span className="text-sm">Auto-approve</span>
                </label>

                <label className="flex items-center gap-2 pt-5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.enabled ?? true}
                    onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
                    className="rounded border-border"
                  />
                  <span className="text-sm">Enabled</span>
                </label>
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <Button variant="outline" onClick={closeDialog} disabled={saving}>
                Cancel
              </Button>
              <Button
                onClick={handleSave}
                disabled={!form.name?.trim() || !form.schedule?.trim() || saving}
                className="gap-1.5"
              >
                <Check className="size-4" />
                {editingJob ? "Save Changes" : "Create Job"}
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
