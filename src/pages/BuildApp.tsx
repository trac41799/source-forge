import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { IPC } from "@/lib/ipc/commands";
import { useProjectStore } from "@/stores/projectStore";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Hammer,
  PlayCircle,
  CheckCircle2,
  XCircle,
  MinusCircle,
  ExternalLink,
  Loader2,
} from "lucide-react";
import { cn } from "@/lib/utils";

interface StageRecord {
  name: string;
  status: string;
  message: string;
}

interface VerificationCheck {
  name: string;
  status: unknown;
  detail: string;
}

interface VerificationReport {
  passed: boolean;
  checks: VerificationCheck[];
}

interface DeployOutcome {
  provider: string;
  url: string | null;
  detail: string;
}

interface BuildReport {
  run_id: string;
  status: string;
  stack_id: string | null;
  stages: StageRecord[];
  verification: VerificationReport | null;
  deploy: DeployOutcome | null;
  compounder_items: number;
  error: string | null;
}

const STACK_OPTIONS = [
  { id: "nextjs-supabase-vercel", name: "Next.js + Supabase + Vercel" },
  { id: "nextjs-supabase-fastapi", name: "Next.js + Supabase + FastAPI" },
  { id: "express-react-supabase", name: "Express + React + Supabase" },
  { id: "nextjs-prisma-vercel", name: "Next.js + Prisma + Vercel Postgres" },
];

function checkPassed(status: unknown): boolean {
  return status === "Pass";
}

function checkLabel(status: unknown): string {
  if (status === "Pass") return "Pass";
  if (typeof status === "object" && status !== null) {
    const entries = Object.entries(status as Record<string, string>);
    if (entries.length > 0) return `${entries[0][0]}: ${entries[0][1]}`;
  }
  return String(status);
}

export default function BuildApp() {
  const project = useProjectStore((s) => s.currentProject);
  const [specPath, setSpecPath] = useState("");
  const [projectPath, setProjectPath] = useState("");
  const [stackId, setStackId] = useState("nextjs-supabase-vercel");
  const [agentCommand, setAgentCommand] = useState("opencode");
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<string[]>([]);
  const [report, setReport] = useState<BuildReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (project?.path && !projectPath) {
      setProjectPath(project.path);
    }
  }, [project, projectPath]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{ stage: string; status: string; message: string }>(
      "build-app-progress",
      (event) => {
        const p = event.payload;
        setProgress((prev) => [
          ...prev,
          `${p.stage}: ${p.status}${p.message ? ` — ${p.message}` : ""}`,
        ]);
      }
    ).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const canStart = specPath.trim().length > 0 && projectPath.trim().length > 0 && !running;

  const start = async () => {
    setRunning(true);
    setError(null);
    setReport(null);
    setProgress([]);
    try {
      const result = await invoke<BuildReport>(IPC.buildApp, {
        options: {
          project_id: null,
          spec_path: specPath.trim(),
          project_path: projectPath.trim(),
          stack_id: stackId,
          agent_command: agentCommand.trim() || "opencode",
          base_branch: "main",
          allow_deploy_on_failed_verification: false,
          generate_dockerfile: true,
        },
      });
      setReport(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-auto p-6">
      <div className="flex items-center gap-2">
        <Hammer className="size-5" />
        <h1 className="text-lg font-semibold">Build App</h1>
        <span className="text-xs text-muted-foreground">
          spec → provision → agents → verify → deploy
        </span>
      </div>

      <Card className="p-4 space-y-3">
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <label className="space-y-1 text-sm">
            <span className="text-muted-foreground">Spec file (markdown)</span>
            <Input
              value={specPath}
              onChange={(e) => setSpecPath(e.target.value)}
              placeholder="docs/PLAN.md"
            />
          </label>
          <label className="space-y-1 text-sm">
            <span className="text-muted-foreground">Project path</span>
            <Input
              value={projectPath}
              onChange={(e) => setProjectPath(e.target.value)}
              placeholder="/path/to/project"
            />
          </label>
          <label className="space-y-1 text-sm">
            <span className="text-muted-foreground">Stack</span>
            <select
              className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm"
              value={stackId}
              onChange={(e) => setStackId(e.target.value)}
            >
              {STACK_OPTIONS.map((stack) => (
                <option key={stack.id} value={stack.id}>
                  {stack.name}
                </option>
              ))}
            </select>
          </label>
          <label className="space-y-1 text-sm">
            <span className="text-muted-foreground">Agent command</span>
            <Input
              value={agentCommand}
              onChange={(e) => setAgentCommand(e.target.value)}
              placeholder="opencode"
            />
          </label>
        </div>
        <div className="flex items-center gap-3">
          <Button onClick={start} disabled={!canStart} className="gap-1.5">
            {running ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <PlayCircle className="size-4" />
            )}
            {running ? "Building…" : "Build App"}
          </Button>
          <span className="text-xs text-muted-foreground">
            Deploy runs only when verification passes.
          </span>
        </div>
      </Card>

      {progress.length > 0 && (
        <Card className="p-4 space-y-1">
          <div className="text-sm font-medium">Progress</div>
          {progress.map((line, index) => (
            <div key={index} className="text-xs text-muted-foreground">
              {line}
            </div>
          ))}
        </Card>
      )}

      {error && (
        <Card className="p-4 text-sm text-red-400">Build failed to start: {error}</Card>
      )}

      {report && (
        <Card className="p-4 space-y-4">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">Build report</span>
            <Badge
              variant={report.status === "succeeded" ? "default" : "secondary"}
              className={cn(
                report.status === "succeeded" && "bg-emerald-600/80 text-white"
              )}
            >
              {report.status}
            </Badge>
            <span className="text-xs text-muted-foreground">
              {report.compounder_items} knowledge items compounded
            </span>
          </div>

          {report.error && (
            <div className="text-sm text-red-400">{report.error}</div>
          )}

          <div className="space-y-1">
            {report.stages.map((stage) => (
              <div key={stage.name} className="flex items-center gap-2 text-xs">
                {stage.status === "done" && (
                  <CheckCircle2 className="size-3.5 text-emerald-500" />
                )}
                {stage.status === "failed" && (
                  <XCircle className="size-3.5 text-red-500" />
                )}
                {(stage.status === "skipped" || stage.status === "awaiting_user") && (
                  <MinusCircle className="size-3.5 text-yellow-500" />
                )}
                <span className="font-mono">{stage.name}</span>
                <span className="text-muted-foreground">
                  {stage.status}
                  {stage.message ? ` — ${stage.message}` : ""}
                </span>
              </div>
            ))}
          </div>

          {report.verification && (
            <div className="space-y-1">
              <div className="text-sm font-medium">
                Verification:{" "}
                <span
                  className={
                    report.verification.passed
                      ? "text-emerald-500"
                      : "text-red-400"
                  }
                >
                  {report.verification.passed ? "passed" : "failed"}
                </span>
              </div>
              {report.verification.checks
                .filter((check) => !checkPassed(check.status))
                .map((check) => (
                  <div key={check.name} className="text-xs text-muted-foreground">
                    {check.name}: {checkLabel(check.status)}
                  </div>
                ))}
            </div>
          )}

          {report.deploy?.url && (
            <a
              href={report.deploy.url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 text-sm text-[#58a6ff]"
            >
              <ExternalLink className="size-3.5" />
              {report.deploy.url}
            </a>
          )}
        </Card>
      )}
    </div>
  );
}
