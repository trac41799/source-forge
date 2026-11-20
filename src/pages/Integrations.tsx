import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useIntegrationStore } from "@/stores/integrationStore";
import { useBackwardChannelStore } from "@/stores/backwardChannelStore";
import { useProjectStore } from "@/stores/projectStore";
import type { ChatPlatformConfig } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Database,
  Github,
  Search,
  Shield,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  Loader2,
  Settings2,
  Activity,
  FileCode,
  Key,
  Globe,
  Lock,
  RefreshCw,
  ExternalLink,
  Bug,
  ArrowRight,
  CircleDot,
  MessageCircle,
  GitPullRequest,
  Send,
  Flag,
  Check,
  Eye,
  FolderOpen,
} from "lucide-react";

type Tab = "supabase" | "github" | "lockdown" | "migration" | "chat";

const SUPABASE_FEATURES = [
  { key: "docs", label: "Documentation", icon: FileCode, description: "Browse and search Supabase docs", locked: false },
  { key: "database", label: "Database", icon: Database, description: "Query, schema inspection, SQL execution", locked: false },
  { key: "storage", label: "Storage", icon: Globe, description: "File storage bucket management", locked: false },
  { key: "debugging", label: "Debugging", icon: Bug, description: "Log inspection, error tracking, advisors", locked: false },
  { key: "functions", label: "Edge Functions", icon: Activity, description: "Deploy and manage Edge Functions", locked: false },
  { key: "branching", label: "Branching", icon: GitBranch, description: "Create/manage DB branches", locked: true },
  { key: "development", label: "Development", icon: Settings2, description: "Local dev, migration management", locked: true },
  { key: "account", label: "Account", icon: Key, description: "Project settings, billing, team", locked: true },
];

const GITHUB_FEATURES = [
  { key: "repos", label: "Repositories", icon: Github, description: "Browse repos, files, branches", locked: false },
  { key: "issues", label: "Issues", icon: Bug, description: "Create, search, classify issues", locked: false },
  { key: "prs", label: "Pull Requests", icon: ArrowRight, description: "Review, merge, monitor PRs", locked: false },
  { key: "actions", label: "Actions", icon: Activity, description: "CI/CD workflow monitoring", locked: false },
  { key: "code_security", label: "Code Security", icon: Shield, description: "Dependabot, secret scanning", locked: true },
  { key: "projects", label: "Projects", icon: Settings2, description: "Project boards and automation", locked: true },
  { key: "notifications", label: "Notifications", icon: CircleDot, description: "Watch and alert configuration", locked: true },
];

function GitBranch(props: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={props.className}
    >
      <line x1="6" x2="6" y1="3" y2="15" />
      <circle cx="6" cy="3" r="3" />
      <circle cx="6" cy="18" r="3" />
      <path d="M18 9a9 9 0 0 1-9 9" />
      <circle cx="18" cy="9" r="3" />
    </svg>
  );
}

function ToggleSwitch({
  enabled,
  onChange,
  disabled,
}: {
  enabled: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      disabled={disabled}
      onClick={() => !disabled && onChange(!enabled)}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
        enabled ? "bg-primary" : "bg-muted",
        disabled && "cursor-not-allowed opacity-50"
      )}
    >
      <span
        className={cn(
          "pointer-events-none block h-4 w-4 rounded-full bg-background shadow-sm transition-transform",
          enabled ? "translate-x-[18px]" : "translate-x-[2px]"
        )}
      />
    </button>
  );
}

function Badge({
  variant = "default",
  children,
}: {
  variant?: "default" | "success" | "warning" | "danger" | "info";
  children: React.ReactNode;
}) {
  const variants = {
    default: "bg-muted text-muted-foreground",
    success: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
    warning: "bg-amber-500/10 text-amber-400 border-amber-500/20",
    danger: "bg-red-500/10 text-red-400 border-red-500/20",
    info: "bg-blue-500/10 text-blue-400 border-blue-500/20",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs font-medium",
        variants[variant]
      )}
    >
      {children}
    </span>
  );
}

function Integrations() {
  const activeProjectPath = useProjectStore((s) => s.currentProject?.path) || "";
  const [activeTab, setActiveTab] = useState<Tab>("supabase");
  const [projectPath, setProjectPath] = useState(activeProjectPath);
  const [supabaseUrl, setSupabaseUrl] = useState("");
  const [anonKey, setAnonKey] = useState("");
  const [serviceKey, setServiceKey] = useState("");
  const [issueFilter, setIssueFilter] = useState<"open" | "closed" | "all">("open");
  const [lockdownEnabled, setLockdownEnabled] = useState(false);
  const [prTitle, setPrTitle] = useState("");
  const [prBody, setPrBody] = useState("");
  const [prHead, setPrHead] = useState("");
  const [prBase, setPrBase] = useState("main");
  const [githubToken, setGithubToken] = useState("");
  const [lastPrUrl, setLastPrUrl] = useState<string | null>(null);

  const store = useIntegrationStore();

  useEffect(() => {
    if (activeProjectPath) {
      setProjectPath((prev) => prev || activeProjectPath);
    }
  }, [activeProjectPath]);

  const handleBrowseProject = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string" && selected) {
        setProjectPath(selected);
      }
    } catch {
      // Ignore dialog cancellation / web-preview errors
    }
  };

  useEffect(() => {
    if (activeTab === "supabase") {
      store.detectSupabase(projectPath);
    } else if (activeTab === "github") {
      store.detectGitHub(projectPath);
    } else if (activeTab === "migration") {
      store.checkMigrationSafety(projectPath);
    }
  }, [activeTab, projectPath]);

  useEffect(() => {
    if (activeTab === "migration" && !store.migrationWatcherActive) {
      store.startMigrationWatcher(projectPath);
    }
  }, [activeTab, store.migrationWatcherActive]);

  const handleSaveSupabase = async () => {
    const config = {
      id: `supabase-${Date.now()}`,
      project_id: "default",
      supabase_project_ref: store.supabaseDetected || "local",
      supabase_url: supabaseUrl,
      anon_key: anonKey || null,
      service_role_key: serviceKey || null,
      feature_groups: {
        docs: true,
        database: true,
        storage: true,
        debugging: true,
        functions: true,
        branching: false,
        development: false,
        account: false,
      },
      read_only: true,
      created_at: new Date().toISOString(),
    };
    await store.saveSupabaseConfig(config);
    await store.getSupabaseConfigs("default");
  };

  const handleSaveGithub = async () => {
    const detected = store.githubDetected;
    const config = {
      id: `github-${Date.now()}`,
      project_id: "default",
      repo_owner: detected?.owner || "",
      repo_name: detected?.repo || "",
      repo_visibility: detected?.visibility || "unknown",
      lockdown_enabled: lockdownEnabled,
      token_present: false,
      features: {
        repos: true,
        issues: true,
        prs: true,
        actions: true,
        code_security: false,
        projects: false,
        notifications: false,
      },
      created_at: new Date().toISOString(),
    };
    await store.saveGitHubConfig(config);
    await store.getGitHubConfigs("default");
  };

  const handleFetchIssues = () => {
    const detected = store.githubDetected;
    if (detected) {
      store.listGitHubIssues(detected.owner, detected.repo, issueFilter);
    }
  };

  const handleCreatePr = async () => {
    const detected = store.githubDetected;
    if (!detected || !githubToken || !prTitle || !prHead) return;
    const url = await store.createPullRequest(
      githubToken,
      detected.owner,
      detected.repo,
      prTitle,
      prBody,
      prHead,
      prBase
    );
    if (url) setLastPrUrl(url);
  };

  const handleTriggerWave = (_issueNumber: number, _issueTitle: string) => {
  };

  const handleEnableLockdown = async () => {
    const detected = store.githubDetected;
    if (!detected) return;
    await store.enableLockdown(githubToken, detected.owner, detected.repo);
  };

  const handleCheckVisibility = async () => {
    const detected = store.githubDetected;
    if (!detected) return;
    await store.checkRepoVisibility(detected.owner, detected.repo);
  };

  const handleToggleSupabaseFeature = async (configId: string, feature: string, enabled: boolean) => {
    await store.toggleSupabaseFeature(configId, feature, enabled);
    await store.getSupabaseConfigs("default");
  };

  const handleToggleGithubFeature = async (configId: string, feature: string, enabled: boolean) => {
    await store.toggleGitHubFeature(configId, feature, enabled);
    await store.getGitHubConfigs("default");
  };

  const handleCheckMigrations = () => {
    store.checkMigrationSafety(projectPath);
  };

  const handleCheckActions = () => {
    store.checkGitHubActions(projectPath);
  };

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-14 shrink-0 items-center gap-4 border-b px-6">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Integrations</h1>
        </div>
        <span className="text-xs text-muted-foreground">Supabase & GitHub first-class connectors</span>
      </header>

      <div className="flex border-b">
        <button
          onClick={() => setActiveTab("supabase")}
          className={cn(
            "flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors",
            "border-b-2 -mb-px",
            activeTab === "supabase"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <Database className="size-4" />
          Supabase
        </button>
        <button
          onClick={() => setActiveTab("github")}
          className={cn(
            "flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors",
            "border-b-2 -mb-px",
            activeTab === "github"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <Github className="size-4" />
          GitHub
        </button>
        <button
          onClick={() => setActiveTab("lockdown")}
          className={cn(
            "flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors",
            "border-b-2 -mb-px",
            activeTab === "lockdown"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <Lock className="size-4" />
          Lockdown
        </button>
        <button
          onClick={() => setActiveTab("migration")}
          className={cn(
            "flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors",
            "border-b-2 -mb-px",
            activeTab === "migration"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <Shield className="size-4" />
          Migrations
          {store.migrationsDetected.filter((m) => m.flagged === null).length > 0 && (
            <span className="ml-1 rounded-full bg-amber-500/20 px-1.5 text-[10px] font-semibold text-amber-400">
              {store.migrationsDetected.filter((m) => m.flagged === null).length}
            </span>
          )}
        </button>
        <button
          onClick={() => setActiveTab("chat")}
          className={cn(
            "flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors",
            "border-b-2 -mb-px",
            activeTab === "chat"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <MessageCircle className="size-4" />
          Chat
        </button>
      </div>

      <ScrollArea className="flex-1">
        <div className="p-6 space-y-6">
          {/* Project Path Input */}
          <div className="rounded-lg border bg-card p-4">
            <label className="mb-2 block text-sm font-medium text-foreground">
              Project Path
            </label>
            <div className="flex gap-2">
              <Input
                value={projectPath}
                onChange={(e) => setProjectPath(e.target.value)}
                placeholder="/path/to/project"
                className="flex-1 font-mono text-sm"
              />
              <Button
                variant="outline"
                size="sm"
                type="button"
                onClick={handleBrowseProject}
              >
                <FolderOpen className="size-4 mr-1" />
                Browse Folder
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  if (activeTab === "supabase") store.detectSupabase(projectPath);
                  else store.detectGitHub(projectPath);
                }}
              >
                <Search className="size-4" />
              </Button>
            </div>
          </div>

          {activeTab === "supabase" && <SupabaseTab
            supabaseUrl={supabaseUrl}
            setSupabaseUrl={setSupabaseUrl}
            anonKey={anonKey}
            setAnonKey={setAnonKey}
            serviceKey={serviceKey}
            setServiceKey={setServiceKey}
            handleSaveSupabase={handleSaveSupabase}
            handleToggleFeature={handleToggleSupabaseFeature}
          />}

          {activeTab === "github" && <GithubTab
            lockdownEnabled={lockdownEnabled}
            setLockdownEnabled={setLockdownEnabled}
            issueFilter={issueFilter}
            setIssueFilter={setIssueFilter}
            githubToken={githubToken}
            setGithubToken={setGithubToken}
            prTitle={prTitle}
            setPrTitle={setPrTitle}
            prBody={prBody}
            setPrBody={setPrBody}
            prHead={prHead}
            setPrHead={setPrHead}
            prBase={prBase}
            setPrBase={setPrBase}
            lastPrUrl={lastPrUrl}
            handleSaveGithub={handleSaveGithub}
            handleToggleFeature={handleToggleGithubFeature}
            handleFetchIssues={handleFetchIssues}
            handleCheckActions={handleCheckActions}
            handleCreatePr={handleCreatePr}
            handleTriggerWave={handleTriggerWave}
          />}

          {activeTab === "lockdown" && <LockdownTab
            githubToken={githubToken}
            setGithubToken={setGithubToken}
            handleCheckVisibility={handleCheckVisibility}
            handleEnableLockdown={handleEnableLockdown}
          />}

          {activeTab === "migration" && <MigrationSafetyTab
            handleCheckMigrations={handleCheckMigrations}
          />}

          {activeTab === "chat" && <ChatTab />}

          {store.error && (
            <div className="rounded-lg border border-red-500/20 bg-red-500/5 p-4">
              <div className="flex items-start gap-3">
                <XCircle className="mt-0.5 size-4 text-red-400 shrink-0" />
                <div>
                  <p className="text-sm font-medium text-red-400">Error</p>
                  <p className="mt-1 text-xs text-red-400/80 font-mono">{store.error}</p>
                </div>
                <Button variant="ghost" size="sm" className="ml-auto shrink-0" onClick={store.clearError}>
                  Dismiss
                </Button>
              </div>
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}

function SupabaseTab({
  supabaseUrl, setSupabaseUrl,
  anonKey, setAnonKey,
  serviceKey, setServiceKey,
  handleSaveSupabase,
  handleToggleFeature,
}: {
  supabaseUrl: string;
  setSupabaseUrl: (v: string) => void;
  anonKey: string;
  setAnonKey: (v: string) => void;
  serviceKey: string;
  setServiceKey: (v: string) => void;
  handleSaveSupabase: () => void;
  handleToggleFeature: (configId: string, feature: string, enabled: boolean) => void;
}) {
  const supabaseConfigs = useIntegrationStore((s) => s.supabaseConfigs);
  const detected = useIntegrationStore((s) => s.supabaseDetected);
  const loading = useIntegrationStore((s) => s.loading);

  return (
    <div className="space-y-6">
      {/* Detection Status */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Search className="size-4" />
          Auto-Detection
        </h3>
        {detected ? (
          <div className="flex items-center gap-2">
            <CheckCircle2 className="size-4 text-emerald-400" />
            <span className="text-sm text-foreground">
              Supabase project detected:{" "}
              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">{detected}</code>
            </span>
            <Badge variant="success">Connected</Badge>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <XCircle className="size-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">
              No Supabase project detected. Enter details manually or ensure{" "}
              <code className="rounded bg-muted px-1">supabase/config.toml</code> exists.
            </span>
          </div>
        )}
      </div>

      {/* Config Form */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
          <Settings2 className="size-4" />
          Connection Config
        </h3>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs text-muted-foreground">Supabase URL</label>
            <Input
              value={supabaseUrl}
              onChange={(e) => setSupabaseUrl(e.target.value)}
              placeholder="https://xxxxxxxxxxxx.supabase.co"
              className="font-mono text-sm"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Anon Key</label>
              <Input
                value={anonKey}
                onChange={(e) => setAnonKey(e.target.value)}
                placeholder="sb_publishable_..."
                className="font-mono text-sm"
                type="password"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Service Role Key</label>
              <Input
                value={serviceKey}
                onChange={(e) => setServiceKey(e.target.value)}
                placeholder="sb_secret_..."
                className="font-mono text-sm"
                type="password"
              />
            </div>
          </div>
          <div className="flex items-center gap-2 pt-1">
            <ToggleSwitch
              enabled={true}
              onChange={() => {}}
              disabled={true}
            />
            <span className="text-xs text-muted-foreground">Read-only mode (enforced for safety)</span>
          </div>
          <Button onClick={handleSaveSupabase} disabled={loading} size="sm">
            {loading ? <Loader2 className="size-4 animate-spin" /> : <SaveIcon />}
            Save Configuration
          </Button>
        </div>
      </div>

      {/* Feature Groups */}
      {supabaseConfigs.length > 0 && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
            <Database className="size-4" />
            Feature Groups
          </h3>
          <div className="space-y-1">
            {SUPABASE_FEATURES.map((feat) => {
              const config = supabaseConfigs[0];
              const enabled = config?.feature_groups?.[feat.key] ?? false;
              return (
                <div
                  key={feat.key}
                  className="flex items-center justify-between rounded-md px-3 py-2.5 transition-colors hover:bg-muted/50"
                >
                  <div className="flex items-center gap-3">
                    <feat.icon className="size-4 text-muted-foreground" />
                    <div>
                      <p className="text-sm font-medium text-foreground">{feat.label}</p>
                      <p className="text-xs text-muted-foreground">{feat.description}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {feat.locked && (
                      <Badge variant="warning">Locked</Badge>
                    )}
                    <ToggleSwitch
                      enabled={enabled}
                      onChange={(v) => handleToggleFeature(config.id, feat.key, v)}
                      disabled={feat.locked}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Saved Configs */}
      {supabaseConfigs.map((config) => (
        <div key={config.id} className="rounded-lg border bg-card p-4">
          <h3 className="mb-2 flex items-center gap-2 text-sm font-medium text-foreground">
            <Database className="size-4" />
            Saved Config
          </h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <span className="text-muted-foreground">Project Ref:</span>
            <code className="font-mono text-foreground">{config.supabase_project_ref}</code>
            <span className="text-muted-foreground">URL:</span>
            <code className="font-mono text-foreground truncate">{config.supabase_url || "—"}</code>
            <span className="text-muted-foreground">Created:</span>
            <span className="text-foreground">{new Date(config.created_at).toLocaleString()}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

function GithubTab({
  lockdownEnabled, setLockdownEnabled,
  issueFilter, setIssueFilter,
  githubToken, setGithubToken,
  prTitle, setPrTitle,
  prBody, setPrBody,
  prHead, setPrHead,
  prBase, setPrBase,
  lastPrUrl,
  handleSaveGithub,
  handleToggleFeature,
  handleFetchIssues,
  handleCheckActions,
  handleCreatePr,
  handleTriggerWave,
}: {
  lockdownEnabled: boolean;
  setLockdownEnabled: (v: boolean) => void;
  issueFilter: "open" | "closed" | "all";
  setIssueFilter: (v: "open" | "closed" | "all") => void;
  githubToken: string;
  setGithubToken: (v: string) => void;
  prTitle: string;
  setPrTitle: (v: string) => void;
  prBody: string;
  setPrBody: (v: string) => void;
  prHead: string;
  setPrHead: (v: string) => void;
  prBase: string;
  setPrBase: (v: string) => void;
  lastPrUrl: string | null;
  handleSaveGithub: () => void;
  handleToggleFeature: (configId: string, feature: string, enabled: boolean) => void;
  handleFetchIssues: () => void;
  handleCheckActions: () => void;
  handleCreatePr: () => void;
  handleTriggerWave: (issueNumber: number, issueTitle: string) => void;
}) {
  const githubConfigs = useIntegrationStore((s) => s.githubConfigs);
  const detected = useIntegrationStore((s) => s.githubDetected);
  const issues = useIntegrationStore((s) => s.githubIssues);
  const workflows = useIntegrationStore((s) => s.actionsWorkflows);
  const loading = useIntegrationStore((s) => s.loading);

  return (
    <div className="space-y-6">
      {/* GitHub Token */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Key className="size-4" />
          GitHub Personal Access Token
        </h3>
        <Input
          value={githubToken}
          onChange={(e) => setGithubToken(e.target.value)}
          placeholder="ghp_..."
          type="password"
          className="font-mono text-sm"
        />
        <p className="mt-2 text-xs text-muted-foreground">
          Required for PR creation, CI status, and lockdown commands. Token is not stored.
        </p>
      </div>

      {/* Detection Status */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Github className="size-4" />
          Repository Detection
        </h3>
        {detected ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="size-4 text-emerald-400" />
              <span className="text-sm font-medium text-foreground">
                {detected.owner}/{detected.repo}
              </span>
              <Badge variant={detected.visibility === "public" ? "info" : "warning"}>
                {detected.visibility}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground">
              Detected via <code className="rounded bg-muted px-1">.git/config</code>. Visibility may require API check.
            </p>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <XCircle className="size-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">
              No GitHub repository detected. Ensure <code className="rounded bg-muted px-1">.git/config</code> has a GitHub remote.
            </span>
          </div>
        )}
      </div>

      {/* Lockdown Toggle */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Lock className="size-4" />
          Repository Lockdown
        </h3>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-foreground">Lockdown Mode</p>
            <p className="text-xs text-muted-foreground">
              Restrict write access and enforce approval workflows
            </p>
          </div>
          <ToggleSwitch enabled={lockdownEnabled} onChange={setLockdownEnabled} />
        </div>
        {lockdownEnabled && (
          <div className="mt-3 rounded-md border border-amber-500/20 bg-amber-500/5 p-3">
            <div className="flex items-start gap-2">
              <AlertTriangle className="mt-0.5 size-4 text-amber-400 shrink-0" />
              <p className="text-xs text-amber-400">
                Lockdown is active. Pushes, merges, and direct edits are restricted. Use the PR workflow for all changes.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Save Config */}
      {detected && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
            <Settings2 className="size-4" />
            Save Repository Config
          </h3>
          <Button onClick={handleSaveGithub} disabled={loading} size="sm">
            {loading ? <Loader2 className="size-4 animate-spin" /> : <SaveIcon />}
            Save {detected.owner}/{detected.repo}
          </Button>
        </div>
      )}

      {/* Feature Groups */}
      {githubConfigs.length > 0 && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
            <Github className="size-4" />
            Feature Groups
          </h3>
          <div className="space-y-1">
            {GITHUB_FEATURES.map((feat) => {
              const config = githubConfigs[0];
              const enabled = config?.features?.[feat.key] ?? false;
              return (
                <div
                  key={feat.key}
                  className="flex items-center justify-between rounded-md px-3 py-2.5 transition-colors hover:bg-muted/50"
                >
                  <div className="flex items-center gap-3">
                    <feat.icon className="size-4 text-muted-foreground" />
                    <div>
                      <p className="text-sm font-medium text-foreground">{feat.label}</p>
                      <p className="text-xs text-muted-foreground">{feat.description}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {feat.locked && <Badge variant="warning">Locked</Badge>}
                    <ToggleSwitch
                      enabled={enabled}
                      onChange={(v) => handleToggleFeature(config.id, feat.key, v)}
                      disabled={feat.locked}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* PR Creation Form */}
      {detected && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
            <GitPullRequest className="size-4" />
            Create Pull Request
          </h3>
          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Title</label>
              <Input
                value={prTitle}
                onChange={(e) => setPrTitle(e.target.value)}
                placeholder="Wave: feature description"
                className="text-sm"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Body (Markdown)</label>
              <textarea
                value={prBody}
                onChange={(e) => setPrBody(e.target.value)}
                placeholder="## Summary&#10;- Change 1&#10;- Change 2"
                className="min-h-[80px] w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm font-mono"
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="mb-1 block text-xs text-muted-foreground">Head branch</label>
                <Input
                  value={prHead}
                  onChange={(e) => setPrHead(e.target.value)}
                  placeholder="feature/wave-5"
                  className="font-mono text-sm"
                />
              </div>
              <div>
                <label className="mb-1 block text-xs text-muted-foreground">Base branch</label>
                <Input
                  value={prBase}
                  onChange={(e) => setPrBase(e.target.value)}
                  placeholder="main"
                  className="font-mono text-sm"
                />
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button
                onClick={handleCreatePr}
                disabled={loading || !githubToken || !prTitle || !prHead}
                size="sm"
              >
                {loading ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4 mr-2" />}
                Open PR
              </Button>
              {!githubToken && (
                <span className="text-xs text-muted-foreground">Enter a GitHub token to enable.</span>
              )}
            </div>
            {lastPrUrl && (
              <a
                href={lastPrUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 rounded-md border border-emerald-500/20 bg-emerald-500/5 p-2 text-xs text-emerald-400 hover:underline"
              >
                <CheckCircle2 className="size-3" />
                <span className="truncate">PR created: {lastPrUrl}</span>
                <ExternalLink className="size-3 shrink-0" />
              </a>
            )}
          </div>
        </div>
      )}

      {/* GitHub Actions Check */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Activity className="size-4" />
          CI/CD Status
        </h3>
        <Button variant="outline" size="sm" onClick={handleCheckActions} className="mb-3">
          <RefreshCw className="size-4 mr-2" />
          Scan Workflows
        </Button>
        {workflows.length > 0 ? (
          <div className="space-y-1.5">
            {workflows.map((wf, i) => (
              <div key={i} className="flex items-center gap-2 rounded-md bg-muted/30 px-3 py-2">
                <CheckCircle2 className="size-3 text-emerald-400" />
                <code className="font-mono text-xs text-foreground">{wf}</code>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            No GitHub Actions workflows found. Click Scan to check.
          </p>
        )}
      </div>

      {/* Issue Browser */}
      {detected && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
            <Bug className="size-4" />
            Issue Browser
          </h3>
          <div className="mb-3 flex items-center gap-2">
            {(["open", "closed", "all"] as const).map((s) => (
              <button
                key={s}
                onClick={() => setIssueFilter(s)}
                className={cn(
                  "rounded-md px-3 py-1 text-xs font-medium transition-colors capitalize",
                  issueFilter === s
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground hover:text-foreground"
                )}
              >
                {s}
              </button>
            ))}
            <Button variant="outline" size="sm" onClick={handleFetchIssues} disabled={loading} className="ml-auto">
              {loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
              Fetch
            </Button>
          </div>
          {issues.length > 0 ? (
            <div className="space-y-2">
              {issues.map((issue) => (
                <div
                  key={issue.id}
                  className="rounded-md border border-border p-3 transition-colors hover:bg-muted/50"
                >
                  <div className="flex items-start gap-3">
                    <a
                      href={`https://github.com/${issue.repo_owner}/${issue.repo_name}/issues/${issue.issue_number}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="mt-0.5 shrink-0"
                    >
                      {issue.state === "open" ? (
                        <CircleDot className="size-4 text-emerald-400" />
                      ) : (
                        <CheckCircle2 className="size-4 text-purple-400" />
                      )}
                    </a>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <a
                          href={`https://github.com/${issue.repo_owner}/${issue.repo_name}/issues/${issue.issue_number}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-sm font-medium text-foreground hover:underline truncate"
                        >
                          {issue.title}
                        </a>
                        <Badge variant={issue.connector_status === "detected" ? "info" : "default"}>
                          {issue.connector_status}
                        </Badge>
                      </div>
                      <div className="mt-1 flex flex-wrap items-center gap-1.5">
                        <span className="text-xs text-muted-foreground">#{issue.issue_number}</span>
                        {issue.labels.map((label) => (
                          <Badge key={label} variant="default">{label}</Badge>
                        ))}
                        {issue.assignee && (
                          <span className="text-xs text-muted-foreground">
                            @{issue.assignee}
                          </span>
                        )}
                      </div>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => handleTriggerWave(issue.issue_number, issue.title)}
                      className="shrink-0"
                    >
                      <ArrowRight className="size-3 mr-1" />
                      Trigger Wave
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">
              No issues loaded. Click Fetch to retrieve issues from GitHub.
            </p>
          )}
        </div>
      )}

      {/* Saved Configs */}
      {githubConfigs.map((config) => (
        <div key={config.id} className="rounded-lg border bg-card p-4">
          <h3 className="mb-2 flex items-center gap-2 text-sm font-medium text-foreground">
            <Github className="size-4" />
            Saved Config
          </h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <span className="text-muted-foreground">Repository:</span>
            <code className="font-mono text-foreground">{config.repo_owner}/{config.repo_name}</code>
            <span className="text-muted-foreground">Visibility:</span>
            <span className="text-foreground capitalize">{config.repo_visibility}</span>
            <span className="text-muted-foreground">Lockdown:</span>
            <span className="text-foreground">{config.lockdown_enabled ? "Enabled" : "Disabled"}</span>
            <span className="text-muted-foreground">Created:</span>
            <span className="text-foreground">{new Date(config.created_at).toLocaleString()}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

function SaveIcon() {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="mr-2">
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
      <polyline points="17 21 17 13 7 13 7 21" />
      <polyline points="7 3 7 8 15 8" />
    </svg>
  );
}

const CHAT_PLATFORMS = [
  { key: "lark", label: "Lark / Feishu", description: "AES-encrypted webhook, group chat → agent" },
  { key: "slack", label: "Slack", description: "HMAC-SHA256 webhook, channel → agent" },
  { key: "discord", label: "Discord", description: "Ed25519 webhook, server → agent" },
  { key: "telegram", label: "Telegram", description: "Bot token auth, polling or webhook" },
];

function ChatTab() {
  const store = useBackwardChannelStore();
  const [activePlatform, setActivePlatform] = useState<string>("lark");
  const [webhookUrl, setWebhookUrl] = useState("");
  const [routingKey, setRoutingKey] = useState("");
  const [appSecret, setAppSecret] = useState("");
  const [signingSecret, setSigningSecret] = useState("");
  const [botToken, setBotToken] = useState("");
  const [queueProvider, setQueueProvider] = useState("upstash");
  const [replyMode, setReplyMode] = useState("mcp_tool");
  const [daemonLogs, setDaemonLogs] = useState<string[]>([]);
  const [configPath, setConfigPath] = useState("/Users/tracnguyendang/Projects");

  useEffect(() => {
    const interval = setInterval(() => {
      store.getDaemonStatus();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleSaveConfig = async () => {
    const config: ChatPlatformConfig = {
      id: `chat-${activePlatform}-${Date.now()}`,
      platform: activePlatform,
      routing_key: routingKey,
      enabled: true,
      webhook_url: webhookUrl,
      credentials: {
        app_secret: appSecret,
        signing_secret: signingSecret,
        bot_token: botToken,
      },
      queue_provider: queueProvider,
      queue_config: {},
      reply_mode: replyMode,
      status: "configured",
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    await store.savePlatformConfig(config);
  };

  const handleTestConnection = async () => {
    await store.testPlatformConnection(activePlatform, {
      webhook_url: webhookUrl,
      routing_key: routingKey,
      ...(appSecret ? { app_secret: appSecret } : {}),
      ...(signingSecret ? { signing_secret: signingSecret } : {}),
      ...(botToken ? { bot_token: botToken } : {}),
    });
  };

  const handleStartDaemon = () => {
    store.startDaemon(configPath);
  };

  const handleStopDaemon = () => {
    store.stopDaemon();
  };

  const handleGetLogs = async () => {
    const logs = await store.getDaemonLogs(50);
    setDaemonLogs(logs);
  };

  const handleCheckQueue = () => {
    store.checkQueueHealth();
  };

  const handleBrowseConfig = async () => {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        filters: [{ name: "YAML", extensions: ["yaml", "yml", "json"] }],
      });
      if (typeof selected === "string" && selected) {
        setConfigPath(selected);
      }
    } catch {
      // Ignore dialog cancellation / web-preview errors
    }
  };

  const getPlatformLabel = (key: string) => {
    const platform = CHAT_PLATFORMS.find((p) => p.key === key);
    return platform?.label || key;
  };

  return (
    <div className="space-y-6">
      {/* Daemon Status Card */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Activity className="size-4" />
          Daemon Status
        </h3>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {store.daemonStatus.running ? (
                <CheckCircle2 className="size-4 text-emerald-400" />
              ) : (
                <XCircle className="size-4 text-muted-foreground" />
              )}
              <span className="text-sm font-medium text-foreground">
                {store.daemonStatus.running ? "Running" : "Stopped"}
              </span>
              <Badge variant={store.daemonStatus.running ? "success" : "default"}>
                {store.daemonStatus.running ? "Online" : "Offline"}
              </Badge>
            </div>
            <div className="flex items-center gap-2">
              {store.daemonStatus.running ? (
                <Button variant="outline" size="sm" onClick={handleStopDaemon} disabled={store.loading}>
                  {store.loading ? <Loader2 className="size-3 animate-spin" /> : null}
                  Stop
                </Button>
              ) : (
                <Button variant="outline" size="sm" onClick={handleStartDaemon} disabled={store.loading}>
                  {store.loading ? <Loader2 className="size-3 animate-spin" /> : null}
                  Start
                </Button>
              )}
              <Button variant="ghost" size="sm" onClick={handleGetLogs} disabled={store.loading}>
                <FileCode className="size-3" />
                <span className="ml-1">Logs</span>
              </Button>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <span className="text-muted-foreground">PID:</span>
            <code className="font-mono text-foreground">{store.daemonStatus.pid ?? "—"}</code>
            <span className="text-muted-foreground">Uptime:</span>
            <code className="font-mono text-foreground">
              {store.daemonStatus.uptime_s != null ? `${store.daemonStatus.uptime_s}s` : "—"}
            </code>
            <span className="text-muted-foreground">Queue Depth:</span>
            <code className="font-mono text-foreground">{store.daemonStatus.queue_depth}</code>
            <span className="text-muted-foreground">Last Event:</span>
            <code className="font-mono text-foreground">{store.daemonStatus.last_event_at ?? "—"}</code>
            <span className="text-muted-foreground">Active Platforms:</span>
            <code className="font-mono text-foreground">
              {store.daemonStatus.active_platforms.length > 0
                ? store.daemonStatus.active_platforms.join(", ")
                : "—"}
            </code>
          </div>
          <div>
            <label className="mb-1 block text-xs text-muted-foreground">Config Path</label>
            <div className="flex gap-2">
              <Input
                value={configPath}
                onChange={(e) => setConfigPath(e.target.value)}
                placeholder="/path/to/daemon/config.yaml"
                className="flex-1 font-mono text-sm"
              />
              <Button
                variant="outline"
                size="sm"
                type="button"
                onClick={handleBrowseConfig}
              >
                <FolderOpen className="size-4 mr-1" />
                Browse Config
              </Button>
            </div>
          </div>
          {store.daemonStatus.error && (
            <div className="rounded-md border border-red-500/20 bg-red-500/5 p-2">
              <p className="text-xs text-red-400 font-mono">{store.daemonStatus.error}</p>
            </div>
          )}
          {daemonLogs.length > 0 && (
            <div className="rounded-md bg-muted/30 p-3 max-h-48 overflow-auto">
              <pre className="text-xs font-mono text-muted-foreground">
                {daemonLogs.join("\n")}
              </pre>
            </div>
          )}
        </div>
      </div>

      {/* Queue Health Card */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Activity className="size-4" />
          Queue Health
        </h3>
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            {store.queueInfo.connected ? (
              <CheckCircle2 className="size-4 text-emerald-400" />
            ) : (
              <XCircle className="size-4 text-muted-foreground" />
            )}
            <span className="text-sm text-foreground">
              {store.queueInfo.connected ? "Connected" : "Disconnected"}
            </span>
            <Badge variant={store.queueInfo.connected ? "success" : "warning"}>
              {store.queueInfo.provider}
            </Badge>
          </div>
          <Button variant="outline" size="sm" onClick={handleCheckQueue} disabled={store.loading}>
            {store.loading ? <Loader2 className="size-3 animate-spin" /> : <RefreshCw className="size-3" />}
            <span className="ml-1">Refresh</span>
          </Button>
        </div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
          <span className="text-muted-foreground">Depth:</span>
          <code className="font-mono text-foreground">{store.queueInfo.queue_depth}</code>
          <span className="text-muted-foreground">Latency:</span>
          <code className="font-mono text-foreground">
            {store.queueInfo.latency_ms != null ? `${store.queueInfo.latency_ms}ms` : "—"}
          </code>
        </div>
      </div>

      {/* Platform Config */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
          <MessageCircle className="size-4" />
          Platform Configuration
        </h3>
        <div className="mb-4 flex items-center gap-1">
          {CHAT_PLATFORMS.map((p) => (
            <button
              key={p.key}
              onClick={() => setActivePlatform(p.key)}
              className={cn(
                "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                activePlatform === p.key
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:text-foreground"
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
        <p className="mb-4 text-xs text-muted-foreground">
          {CHAT_PLATFORMS.find((p) => p.key === activePlatform)?.description}
        </p>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs text-muted-foreground">Webhook URL</label>
            <Input
              value={webhookUrl}
              onChange={(e) => setWebhookUrl(e.target.value)}
              placeholder="https://your-webhook-server.com/chat/events"
              className="font-mono text-sm"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs text-muted-foreground">
              Routing Key{" "}
              <span className="text-muted-foreground/60">
                ({activePlatform === "lark"
                  ? "app_id"
                  : activePlatform === "slack"
                    ? "team_id:channel_id"
                    : activePlatform === "discord"
                      ? "server_id:channel_id"
                      : "chat_id"})
              </span>
            </label>
            <Input
              value={routingKey}
              onChange={(e) => setRoutingKey(e.target.value)}
              placeholder={
                activePlatform === "lark"
                  ? "cli_..."
                  : activePlatform === "slack"
                    ? "T12345:C67890"
                    : activePlatform === "discord"
                      ? "server:channel"
                      : "123456789"
              }
              className="font-mono text-sm"
            />
          </div>
          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">App Secret</label>
              <Input
                value={appSecret}
                onChange={(e) => setAppSecret(e.target.value)}
                placeholder="••••••••"
                className="font-mono text-sm"
                type="password"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Signing Secret</label>
              <Input
                value={signingSecret}
                onChange={(e) => setSigningSecret(e.target.value)}
                placeholder="••••••••"
                className="font-mono text-sm"
                type="password"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Bot Token</label>
              <Input
                value={botToken}
                onChange={(e) => setBotToken(e.target.value)}
                placeholder="••••••••"
                className="font-mono text-sm"
                type="password"
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Queue Provider</label>
              <div className="flex items-center gap-1">
                {(["upstash", "postgres", "redis", "file"] as const).map((qp) => (
                  <button
                    key={qp}
                    onClick={() => setQueueProvider(qp)}
                    className={cn(
                      "rounded-md px-3 py-1 text-xs font-medium transition-colors capitalize",
                      queueProvider === qp
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted text-muted-foreground hover:text-foreground"
                    )}
                  >
                    {qp}
                  </button>
                ))}
              </div>
            </div>
            <div>
              <label className="mb-1 block text-xs text-muted-foreground">Reply Mode</label>
              <div className="flex items-center gap-1">
                {(["mcp_tool", "post_process", "inline"] as const).map((rm) => (
                  <button
                    key={rm}
                    onClick={() => setReplyMode(rm)}
                    className={cn(
                      "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                      replyMode === rm
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted text-muted-foreground hover:text-foreground"
                    )}
                  >
                    {rm.replace(/_/g, " ")}
                  </button>
                ))}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2 pt-1">
            <ToggleSwitch enabled={true} onChange={() => {}} />
            <span className="text-xs text-muted-foreground">Enable platform connector</span>
          </div>
          <div className="flex items-center gap-2">
            <Button onClick={handleSaveConfig} disabled={store.loading} size="sm">
              {store.loading ? <Loader2 className="size-4 animate-spin" /> : <SaveIcon />}
              Save Configuration
            </Button>
            <Button variant="outline" size="sm" onClick={handleTestConnection} disabled={store.loading}>
              {store.loading ? <Loader2 className="size-3 animate-spin" /> : <Activity className="size-3" />}
              <span className="ml-1">Test Connection</span>
            </Button>
          </div>
        </div>
      </div>

      {/* Saved Platform Configs */}
      {store.platformConfigs.length > 0 && (
        <div className="rounded-lg border bg-card p-4">
          <h3 className="mb-4 flex items-center gap-2 text-sm font-medium text-foreground">
            <MessageCircle className="size-4" />
            Saved Platform Configs
          </h3>
          <div className="space-y-1">
            {store.platformConfigs.map((config) => (
              <div
                key={config.id}
                className="flex items-center justify-between rounded-md px-3 py-2.5 transition-colors hover:bg-muted/50"
              >
                <div className="flex items-center gap-3">
                  <MessageCircle className="size-4 text-muted-foreground" />
                  <div>
                    <p className="text-sm font-medium text-foreground">
                      {getPlatformLabel(config.platform)}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {config.webhook_url || "No webhook configured"} &middot; {config.routing_key}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Badge
                    variant={
                      config.status === "connected"
                        ? "success"
                        : config.status === "error"
                          ? "danger"
                          : "default"
                    }
                  >
                    {config.status}
                  </Badge>
                  <ToggleSwitch
                    enabled={config.enabled}
                    onChange={(v) => store.togglePlatformConfig(config.id, v)}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Agent Workspaces */}
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Settings2 className="size-4" />
          Agent Workspaces
        </h3>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-foreground">Agent Registry</p>
            <p className="text-xs text-muted-foreground">
              Configure agent workspaces via registry YAML. Agents process incoming chat events.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="info">2 agents</Badge>
            <Button variant="outline" size="sm">
              <ExternalLink className="size-3" />
              <span className="ml-1">Open Registry</span>
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function LockdownTab({
  githubToken, setGithubToken,
  handleCheckVisibility,
  handleEnableLockdown,
}: {
  githubToken: string;
  setGithubToken: (v: string) => void;
  handleCheckVisibility: () => void;
  handleEnableLockdown: () => void;
}) {
  const detected = useIntegrationStore((s) => s.githubDetected);
  const configs = useIntegrationStore((s) => s.githubConfigs);
  const lockdownAutoEnable = useIntegrationStore((s) => s.lockdownAutoEnable);
  const setLockdownAutoEnable = useIntegrationStore((s) => s.setLockdownAutoEnable);
  const loading = useIntegrationStore((s) => s.loading);

  const lockedRepos = configs.filter((c) => c.lockdown_enabled);
  const publicRepos = configs.filter((c) => c.repo_visibility === "public");

  return (
    <div className="space-y-6">
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Key className="size-4" />
          GitHub Token (required for visibility / lockdown)
        </h3>
        <Input
          value={githubToken}
          onChange={(e) => setGithubToken(e.target.value)}
          placeholder="ghp_..."
          type="password"
          className="font-mono text-sm"
        />
      </div>

      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Eye className="size-4" />
          Repository Visibility Check
        </h3>
        <p className="mb-3 text-xs text-muted-foreground">
          Check the public/private status of the detected repository via the GitHub API.
        </p>
        {detected ? (
          <div className="mb-3 flex items-center gap-2">
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
              {detected.owner}/{detected.repo}
            </code>
            <Badge variant={detected.visibility === "public" ? "info" : "warning"}>
              {detected.visibility}
            </Badge>
          </div>
        ) : (
          <p className="mb-3 text-xs text-muted-foreground">
            Detect a repository on the GitHub tab first.
          </p>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={handleCheckVisibility}
          disabled={loading || !detected}
        >
          {loading ? <Loader2 className="size-4 animate-spin" /> : <Eye className="size-4 mr-2" />}
          Check Visibility
        </Button>
      </div>

      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Lock className="size-4" />
          Auto-Enable for Public Repos
        </h3>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-foreground">Auto-lockdown public repositories</p>
            <p className="text-xs text-muted-foreground">
              When enabled, any new public repo detected will have lockdown turned on automatically.
            </p>
          </div>
          <ToggleSwitch enabled={lockdownAutoEnable} onChange={setLockdownAutoEnable} />
        </div>
        {lockdownAutoEnable && (
          <div className="mt-3 rounded-md border border-amber-500/20 bg-amber-500/5 p-3">
            <div className="flex items-start gap-2">
              <AlertTriangle className="mt-0.5 size-4 text-amber-400 shrink-0" />
              <p className="text-xs text-amber-400">
                Auto-lockdown enabled. Public repos will be marked as read-only.
              </p>
            </div>
          </div>
        )}
        <div className="mt-3">
          <Button
            size="sm"
            onClick={handleEnableLockdown}
            disabled={!detected || !githubToken}
          >
            <Lock className="size-4 mr-2" />
            Enable Lockdown on {detected ? `${detected.owner}/${detected.repo}` : "Detected Repo"}
          </Button>
        </div>
      </div>

      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Shield className="size-4" />
          Currently Locked-Down Repos
        </h3>
        {lockedRepos.length > 0 ? (
          <div className="space-y-2">
            {lockedRepos.map((c) => (
              <div
                key={c.id}
                className="flex items-center justify-between rounded-md border border-amber-500/20 bg-amber-500/5 px-3 py-2"
              >
                <div className="flex items-center gap-2">
                  <Lock className="size-3 text-amber-400" />
                  <code className="font-mono text-xs text-foreground">
                    {c.repo_owner}/{c.repo_name}
                  </code>
                </div>
                <Badge variant="warning">Locked</Badge>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            No repositories currently locked down.
          </p>
        )}
      </div>

      {publicRepos.length > 0 && (
        <div className="rounded-lg border border-blue-500/20 bg-blue-500/5 p-4">
          <div className="flex items-start gap-2">
            <Globe className="mt-0.5 size-4 text-blue-400 shrink-0" />
            <div>
              <p className="text-sm font-medium text-blue-400">Public repositories detected</p>
              <p className="mt-1 text-xs text-blue-400/80">
                {publicRepos.length} public repo{publicRepos.length === 1 ? "" : "s"} on file.
                Consider enabling lockdown for safer defaults.
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function MigrationSafetyTab({
  handleCheckMigrations,
}: {
  handleCheckMigrations: () => void;
}) {
  const warnings = useIntegrationStore((s) => s.migrationWarnings);
  const detected = useIntegrationStore((s) => s.migrationsDetected);
  const watcherActive = useIntegrationStore((s) => s.migrationWatcherActive);
  const startMigrationWatcher = useIntegrationStore((s) => s.startMigrationWatcher);
  const clearMigrations = useIntegrationStore((s) => s.clearMigrations);
  const confirmMigrationSafe = useIntegrationStore((s) => s.confirmMigrationSafe);
  const flagMigration = useIntegrationStore((s) => s.flagMigration);
  const loading = useIntegrationStore((s) => s.loading);

  const handleStart = () => {
    startMigrationWatcher("");
  };

  return (
    <div className="space-y-6">
      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Shield className="size-4" />
          Migration Watcher
        </h3>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-foreground">
              Status:{" "}
              <span className={cn("font-medium", watcherActive ? "text-emerald-400" : "text-muted-foreground")}>
                {watcherActive ? "Active" : "Inactive"}
              </span>
            </p>
            <p className="text-xs text-muted-foreground">
              Watches the project's <code>supabase/migrations/</code> folder for new .sql files.
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={handleStart}
            disabled={watcherActive}
          >
            <Activity className="size-4 mr-2" />
            {watcherActive ? "Running" : "Start"}
          </Button>
        </div>
      </div>

      <div className="rounded-lg border bg-card p-4">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
          <Search className="size-4" />
          Static Migration Safety Check
        </h3>
        <Button variant="outline" size="sm" onClick={handleCheckMigrations} className="mb-3">
          <Search className="size-4 mr-2" />
          Scan Migrations
        </Button>
        {warnings.length > 0 ? (
          <div className="space-y-2">
            {warnings.map((w, i) => (
              <div
                key={i}
                className={cn(
                  "flex items-start gap-2 rounded-md p-2 text-xs",
                  w.includes("Destructive") || w.includes("DROP") || w.includes("TRUNCATE")
                    ? "border border-red-500/20 bg-red-500/5 text-red-400"
                    : "border border-border bg-muted/30 text-muted-foreground"
                )}
              >
                <AlertTriangle className="mt-0.5 size-3 shrink-0" />
                <span className="font-mono">{w}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            No migration warnings. Click Scan to check for destructive SQL patterns.
          </p>
        )}
      </div>

      <div className="rounded-lg border bg-card p-4">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="flex items-center gap-2 text-sm font-medium text-foreground">
            <Flag className="size-4" />
            Detected Migrations ({detected.length})
          </h3>
          {detected.length > 0 && (
            <Button variant="ghost" size="sm" onClick={clearMigrations}>
              Clear
            </Button>
          )}
        </div>
        {detected.length > 0 ? (
          <div className="space-y-2">
            {detected.map((m) => (
              <div
                key={m.path}
                className={cn(
                  "rounded-md border p-3",
                  m.flagged === true
                    ? "border-red-500/20 bg-red-500/5"
                    : m.flagged === false
                      ? "border-emerald-500/20 bg-emerald-500/5"
                      : "border-amber-500/20 bg-amber-500/5"
                )}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    <code className="block truncate font-mono text-xs text-foreground">
                      {m.path}
                    </code>
                    <span className="text-[10px] text-muted-foreground">
                      {new Date(m.timestamp).toLocaleString()}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    {m.flagged === null && (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => confirmMigrationSafe(m.path)}
                        >
                          <Check className="size-3 mr-1" />
                          Confirm Safe
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => flagMigration(m.path)}
                        >
                          <Flag className="size-3 mr-1" />
                          Flag Risk
                        </Button>
                      </>
                    )}
                    {m.flagged === false && (
                      <Badge variant="success">
                        <Check className="size-3 mr-1" />
                        Safe
                      </Badge>
                    )}
                    {m.flagged === true && (
                      <Badge variant="danger">
                        <Flag className="size-3 mr-1" />
                        Risk
                      </Badge>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">
            {loading
              ? "Loading..."
              : watcherActive
                ? "Watching. New migration files will appear here."
                : "Start the watcher to detect new migration files automatically."}
          </p>
        )}
      </div>
    </div>
  );
}

export default Integrations;
