import { useState, useEffect, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAssetStore, type SkillEntry, type MemoryFileEntry, type MCPEntry, type VaultEntry } from "@/stores/assetStore";
import { useProjectStore } from "@/stores/projectStore";
import { AGENT_CONFIGS } from "@/lib/agents/configs";
import { cn } from "@/lib/utils";
import {
  FolderOpen,
  Brain,
  Server,
  KeyRound,
  Puzzle,
  RefreshCw,
  Search,
  Eye,
  EyeOff,
  Save,
  X,
  FileText,
  Wrench,
  Loader2,
  AlertTriangle,
  CheckCircle,
  Circle,
  Plus,
  Copy,
  Terminal,
  Download,
  Plug,
  Zap,
} from "lucide-react";

type TabId = "skills" | "memory" | "mcps" | "vault" | "plugins";

const TABS: { id: TabId; label: string; icon: typeof FolderOpen }[] = [
  { id: "skills", label: "Skills Library", icon: Brain },
  { id: "memory", label: "Memory Browser", icon: FileText },
  { id: "mcps", label: "MCP Registry", icon: Server },
  { id: "vault", label: "Connector Vault", icon: KeyRound },
  { id: "plugins", label: "Plugin Manager", icon: Puzzle },
];

const MCP_CONFIG_PRESETS = [
  "~/.claude.json",
  ".opencode/config.json",
  "~/.gemini/settings.json",
] as const;

const VAULT_KEY_PRESETS = [
  "OPENROUTER_API_KEY",
  "ANTHROPIC_API_KEY",
  "OPENAI_API_KEY",
  "GEMINI_API_KEY",
] as const;

export default function Assets() {
  const activeProjectPath = useProjectStore((s) => s.currentProject?.path) || "";
  const [activeTab, setActiveTab] = useState<TabId>("skills");
  const [search, setSearch] = useState("");
  const [scanPath, setScanPath] = useState(activeProjectPath);
  const [mcpPath, setMcpPath] = useState("");
  const [profilePath, setProfilePath] = useState(activeProjectPath);

  const store = useAssetStore();

  useEffect(() => {
    if (activeProjectPath) {
      setScanPath((prev) => prev || activeProjectPath);
      setProfilePath((prev) => prev || activeProjectPath);
    }
  }, [activeProjectPath]);

  const handleBrowseFolder = async (onSelect: (path: string) => void) => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string" && selected) {
        onSelect(selected);
      }
    } catch {
      // Ignore dialog cancellation / web-preview errors
    }
  };

  const handleBrowseMcpFile = async () => {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        filters: [{ name: "Config", extensions: ["json", "yaml", "yml", "toml"] }],
      });
      if (typeof selected === "string" && selected) {
        setMcpPath(selected);
      }
    } catch {
      // Ignore dialog cancellation / web-preview errors
    }
  };

  useEffect(() => {
    if (activeTab === "skills") store.scanSkills(scanPath);
    if (activeTab === "memory") store.scanMemory(scanPath);
    if (activeTab === "mcps") store.detectBaguaMcp();
    if (activeTab === "vault") store.listSecrets();
    if (activeTab === "plugins") store.listPlugins();
  }, [activeTab]);

  const filteredSkills = useCallback(() => {
    if (!search) return store.skills;
    const s = search.toLowerCase();
    return store.skills.filter(
      (sk) =>
        sk.name.toLowerCase().includes(s) ||
        sk.source.toLowerCase().includes(s)
    );
  }, [store.skills, search]);

  const filteredMemory = useCallback(() => {
    if (!search) return store.memoryFiles;
    const s = search.toLowerCase();
    return store.memoryFiles.filter(
      (f) =>
        f.name.toLowerCase().includes(s) || f.agent.toLowerCase().includes(s)
    );
  }, [store.memoryFiles, search]);

  const filteredMcps = useCallback(() => {
    if (!search) return store.mcps;
    const s = search.toLowerCase();
    return store.mcps.filter(
      (m) =>
        m.name.toLowerCase().includes(s) ||
        m.server_command.toLowerCase().includes(s)
    );
  }, [store.mcps, search]);

  const filteredSecrets = useCallback(() => {
    if (!search) return store.secrets;
    const s = search.toLowerCase();
    return store.secrets.filter(
      (sec) =>
        sec.key_name.toLowerCase().includes(s) ||
        sec.scope.toLowerCase().includes(s)
    );
  }, [store.secrets, search]);

  const filteredPlugins = useCallback(() => {
    if (!search) return store.plugins;
    const s = search.toLowerCase();
    return store.plugins.filter((p) => p.toLowerCase().includes(s));
  }, [store.plugins, search]);

  return (
    <div className="flex flex-col h-full min-h-screen bg-[#0d1117] text-gray-300">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[#30363d] bg-[#161b22]">
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Asset Manager</h1>
        </div>
        <div className="flex items-center gap-2">
          {store.loading && (
            <span className="flex items-center gap-1 text-xs text-gray-400">
              <Loader2 className="size-3 animate-spin" />
              Loading...
            </span>
          )}
          {store.error && (
            <span className="flex items-center gap-1 text-xs text-red-400">
              <AlertTriangle className="size-3" />
              {store.error}
            </span>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-0 border-b border-[#30363d] bg-[#161b22] px-2">
        {TABS.map((tab) => {
          const Icon = tab.icon;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "flex items-center gap-1.5 px-4 py-2.5 text-sm transition-colors border-b-2 -mb-px",
                activeTab === tab.id
                  ? "border-[#1f6feb] text-[#58a6ff]"
                  : "border-transparent text-gray-400 hover:text-gray-300 hover:border-[#30363d]"
              )}
            >
              <Icon className="size-3.5" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Toolbar */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-[#30363d] bg-[#0d1117]">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-gray-500" />
          <input
            type="text"
            placeholder="Search..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-7 pr-2 py-1.5 text-sm bg-[#161b22] border border-[#30363d] rounded text-gray-300 placeholder-gray-500 focus:outline-none focus:border-[#1f6feb]"
          />
        </div>
        <div className="flex-1" />
        {activeTab === "skills" && (
          <>
            <input
              type="text"
              value={scanPath}
              onChange={(e) => setScanPath(e.target.value)}
              className="px-2 py-1.5 text-sm bg-[#161b22] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb] min-w-[240px]"
              placeholder="Scan path..."
            />
            <button
              type="button"
              onClick={() => handleBrowseFolder(setScanPath)}
              className="flex items-center gap-1 px-2.5 py-1.5 text-xs bg-[#21262d] hover:bg-[#30363d] border border-[#30363d] text-gray-300 rounded transition-colors"
            >
              <FolderOpen className="size-3" /> Browse Folder
            </button>
            <button
              onClick={() => store.scanSkills(scanPath)}
              className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Scan
            </button>
          </>
        )}
        {activeTab === "memory" && (
          <>
            <input
              type="text"
              value={scanPath}
              onChange={(e) => setScanPath(e.target.value)}
              className="px-2 py-1.5 text-sm bg-[#161b22] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb] min-w-[240px]"
              placeholder="Project path..."
            />
            <button
              type="button"
              onClick={() => handleBrowseFolder(setScanPath)}
              className="flex items-center gap-1 px-2.5 py-1.5 text-xs bg-[#21262d] hover:bg-[#30363d] border border-[#30363d] text-gray-300 rounded transition-colors"
            >
              <FolderOpen className="size-3" /> Browse Folder
            </button>
            <button
              onClick={() => store.scanMemory(scanPath)}
              className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Scan
            </button>
          </>
        )}
        {activeTab === "mcps" && (
          <>
            <input
              type="text"
              value={mcpPath}
              onChange={(e) => setMcpPath(e.target.value)}
              className="px-2 py-1.5 text-sm bg-[#161b22] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb] min-w-[260px]"
              placeholder="MCP config path (e.g. ~/.claude.json)"
            />
            <select
              aria-label="MCP config preset"
              value={MCP_CONFIG_PRESETS.includes(mcpPath as (typeof MCP_CONFIG_PRESETS)[number]) ? mcpPath : ""}
              onChange={(e) => {
                if (e.target.value) setMcpPath(e.target.value);
              }}
              className="px-2 py-1.5 text-xs bg-[#161b22] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
            >
              <option value="">Presets...</option>
              {MCP_CONFIG_PRESETS.map((preset) => (
                <option key={preset} value={preset}>
                  {preset}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={handleBrowseMcpFile}
              className="flex items-center gap-1 px-2.5 py-1.5 text-xs bg-[#21262d] hover:bg-[#30363d] border border-[#30363d] text-gray-300 rounded transition-colors"
            >
              <FileText className="size-3" /> Browse File
            </button>
            <button
              onClick={() => store.listMcps(mcpPath)}
              className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Load MCPs
            </button>
          </>
        )}
        {activeTab === "vault" && (
          <button
            onClick={() => store.listSecrets()}
            className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
          >
            <RefreshCw className="size-3" /> Refresh
          </button>
        )}
        {activeTab === "plugins" && (
          <>
            <input
              type="text"
              value={profilePath}
              onChange={(e) => setProfilePath(e.target.value)}
              className="px-2 py-1.5 text-sm bg-[#161b22] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb] min-w-[240px]"
              placeholder="Project path..."
            />
            <button
              type="button"
              onClick={() => handleBrowseFolder(setProfilePath)}
              className="flex items-center gap-1 px-2.5 py-1.5 text-xs bg-[#21262d] hover:bg-[#30363d] border border-[#30363d] text-gray-300 rounded transition-colors"
            >
              <FolderOpen className="size-3" /> Browse Folder
            </button>
            <button
              onClick={() => store.generateProfile(profilePath)}
              className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Generate Profile
            </button>
            <button
              onClick={() => store.listPlugins()}
              className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#238636] hover:bg-[#2ea043] text-white rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Scan Plugins
            </button>
          </>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4">
        {activeTab === "skills" && (
          <SkillsTab
            skills={filteredSkills()}
            selectedSkill={store.selectedSkill}
            onSelect={store.setSelectedSkill}
          />
        )}
        {activeTab === "memory" && (
          <MemoryTab
            files={filteredMemory()}
            selectedFile={store.selectedMemoryFile}
            onSelect={store.setSelectedMemoryFile}
            onSave={store.writeMemoryFile}
          />
        )}
        {activeTab === "mcps" && (
          <MCPsTab
            mcps={filteredMcps()}
            onToggle={store.toggleMcp}
            mcpPath={mcpPath}
            baguaMcpStatus={store.baguaMcpStatus}
            baguaMcpConnection={store.baguaMcpConnection}
            onDetectBagua={store.detectBaguaMcp}
            onTestBagua={store.testBaguaMcpConnection}
            baguaMcpConfig={store.getBaguaMcpConfig()}
          />
        )}
        {activeTab === "vault" && (
          <VaultTab
            secrets={filteredSecrets()}
            onAdd={store.storeSecret}
            onReveal={store.getSecretValue}
          />
        )}
        {activeTab === "plugins" && (
          <PluginsTab
            plugins={filteredPlugins()}
            profile={store.projectProfile}
          />
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between px-4 py-2 border-t border-[#30363d] bg-[#161b22] text-xs text-gray-400">
        <span>
          {activeTab === "skills" && `${store.skills.length} skills`}
          {activeTab === "memory" && `${store.memoryFiles.length} memory files`}
          {activeTab === "mcps" && `${store.mcps.length} MCP servers`}
          {activeTab === "vault" && `${store.secrets.length} secrets`}
          {activeTab === "plugins" && `${store.plugins.length} plugins`}
        </span>
        <span>Assets</span>
      </div>
    </div>
  );
}

// ─── Skills Library ──────────────────────────────────────────────

function SkillsTab({
  skills,
  selectedSkill,
  onSelect,
}: {
  skills: SkillEntry[];
  selectedSkill: SkillEntry | null;
  onSelect: (s: SkillEntry | null) => void;
}) {
  const [viewingContent, setViewingContent] = useState<string | null>(null);
  const store = useAssetStore();

  const handleView = async (path: string) => {
    const content = await store.readSkill(path);
    setViewingContent(content);
  };

  return (
    <div className="grid grid-cols-1 gap-4">
      {skills.length === 0 && (
        <EmptyState icon={Brain} message="No skills found. Enter a path and click Scan." />
      )}
      {skills.map((skill) => (
        <div
          key={skill.id}
          className="bg-[#161b22] border border-[#30363d] rounded-lg p-4 hover:border-[#1f6feb]/50 transition-colors"
        >
          <div className="flex items-start justify-between">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-1">
                <FileText className="size-4 text-[#58a6ff]" />
                <span className="font-medium text-gray-200 truncate">
                  {skill.name}
                </span>
                <SourceBadge source={skill.source} />
              </div>
              <p className="text-xs text-gray-500 truncate mb-2">
                {skill.path}
              </p>
              <p className="text-sm text-gray-400 line-clamp-2">
                {skill.content.substring(0, 200)}...
              </p>
            </div>
            <div className="flex items-center gap-1 ml-2">
              <button
                onClick={() =>
                  onSelect(selectedSkill?.id === skill.id ? null : skill)
                }
                className={cn(
                  "px-2 py-1 text-xs rounded transition-colors",
                  selectedSkill?.id === skill.id
                    ? "bg-[#1f6feb] text-white"
                    : "bg-[#21262d] text-gray-300 hover:bg-[#30363d]"
                )}
              >
                {selectedSkill?.id === skill.id ? "Selected" : "Select"}
              </button>
              <button
                onClick={() => handleView(skill.path)}
                className="px-2 py-1 text-xs bg-[#21262d] text-gray-300 hover:bg-[#30363d] rounded transition-colors"
              >
                <Eye className="size-3" />
              </button>
            </div>
          </div>
        </div>
      ))}

      {viewingContent && (
        <SkillContentModal content={viewingContent} onClose={() => setViewingContent(null)} />
      )}
    </div>
  );
}

// ─── Memory Browser ──────────────────────────────────────────────

function MemoryTab({
  files,
  selectedFile,
  onSelect,
  onSave,
}: {
  files: MemoryFileEntry[];
  selectedFile: MemoryFileEntry | null;
  onSelect: (f: MemoryFileEntry | null) => void;
  onSave: (path: string, content: string) => Promise<void>;
}) {
  const [editing, setEditing] = useState<{
    path: string;
    content: string;
  } | null>(null);

  const handleEdit = (file: (typeof files)[0]) => {
    setEditing({ path: file.path, content: file.content });
  };

  return (
    <div className="grid grid-cols-1 gap-4">
      {files.length === 0 && (
        <EmptyState
          icon={FileText}
          message="No memory files found. Enter a project path and click Scan."
        />
      )}
      {files.map((file) => (
        <div
          key={file.id}
          className="bg-[#161b22] border border-[#30363d] rounded-lg p-4 hover:border-[#1f6feb]/50 transition-colors"
        >
          <div className="flex items-start justify-between">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-1">
                <FileText className="size-4 text-[#58a6ff]" />
                <span className="font-medium text-gray-200">
                  {file.name}
                </span>
                <AgentBadge agent={file.agent} />
                <span className="text-xs text-gray-500">
                  {file.last_modified.substring(0, 19)}
                </span>
              </div>
              <p className="text-xs text-gray-500 truncate mb-2">
                {file.path}
              </p>
              <pre className="text-xs text-gray-400 bg-[#0d1117] rounded p-2 overflow-auto max-h-32 whitespace-pre-wrap">
                {file.content.substring(0, 500)}
                {file.content.length > 500 ? "..." : ""}
              </pre>
            </div>
            <div className="flex items-center gap-1 ml-2 shrink-0">
              <button
                onClick={() => handleEdit(file)}
                className="flex items-center gap-1 px-2 py-1 text-xs bg-[#21262d] text-gray-300 hover:bg-[#30363d] rounded transition-colors"
              >
                <Wrench className="size-3" /> Edit
              </button>
              <button
                onClick={() =>
                  onSelect(
                    selectedFile?.id === file.id ? null : file
                  )
                }
                className={cn(
                  "px-2 py-1 text-xs rounded transition-colors",
                  selectedFile?.id === file.id
                    ? "bg-[#1f6feb] text-white"
                    : "bg-[#21262d] text-gray-300 hover:bg-[#30363d]"
                )}
              >
                View
              </button>
            </div>
          </div>
        </div>
      ))}

      {/* Editor Modal */}
      {editing && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setEditing(null)}
        >
          <div
            className="bg-[#161b22] border border-[#30363d] rounded-lg w-[800px] max-h-[80vh] flex flex-col shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between px-4 py-3 border-b border-[#30363d]">
              <span className="text-sm font-medium text-gray-200">
                Editing: {editing.path}
              </span>
              <button
                onClick={() => setEditing(null)}
                className="text-gray-400 hover:text-gray-300"
              >
                <X className="size-4" />
              </button>
            </div>
            <textarea
              value={editing.content}
              onChange={(e) =>
                setEditing({ ...editing, content: e.target.value })
              }
              className="flex-1 p-4 bg-[#0d1117] text-gray-300 text-sm font-mono resize-none focus:outline-none"
              rows={20}
            />
            <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[#30363d]">
              <button
                onClick={() => setEditing(null)}
                className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-300 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={async () => {
                  await onSave(editing.path, editing.content);
                  setEditing(null);
                }}
                className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#238636] hover:bg-[#2ea043] text-white rounded transition-colors"
              >
                <Save className="size-3" /> Save
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ─── MCP Registry ────────────────────────────────────────────────

function MCPsTab({
  mcps,
  onToggle,
  mcpPath,
  baguaMcpStatus,
  baguaMcpConnection,
  onDetectBagua,
  onTestBagua,
  baguaMcpConfig,
}: {
  mcps: MCPEntry[];
  onToggle: (
    configPath: string,
    name: string,
    enabled: boolean
  ) => Promise<void>;
  mcpPath: string;
  baguaMcpStatus: import("@/lib/types").McpInstallStatus | null;
  baguaMcpConnection: import("@/lib/types").McpConnectionTest | null;
  onDetectBagua: () => Promise<void>;
  onTestBagua: () => Promise<void>;
  baguaMcpConfig: MCPEntry;
}) {
  return (
    <div className="grid grid-cols-1 gap-4">
      {/* Built-in: GA-Bagua Semantic KG MCP */}
      <div className="bg-[#161b22] border border-[#1f6feb]/30 rounded-lg p-4">
        <div className="flex items-start justify-between">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <Zap className="size-4 text-[#d29922]" />
              <span className="font-medium text-gray-200">{baguaMcpConfig.name}</span>
              <span className="px-1.5 py-0.5 text-xs rounded border bg-[#d29922]/20 text-[#d29922] border-[#d29922]/30">
                builtin
              </span>
              {baguaMcpStatus?.installed ? (
                <span className="flex items-center gap-1 text-xs text-[#238636]">
                  <CheckCircle className="size-3" /> Installed
                </span>
              ) : (
                <span className="flex items-center gap-1 text-xs text-gray-500">
                  <Circle className="size-3" /> Not installed
                </span>
              )}
            </div>
            <p className="text-xs text-gray-500 mb-1 font-mono">
              {baguaMcpConfig.server_command}
            </p>
            <p className="text-xs text-gray-400 line-clamp-2 mb-2">
              29 semantic tools: encode concepts into Bagua/Geometric Algebra vectors, classify relationships via WuXing cycles, solve analogies, manage semantic knowledge store
            </p>
            {baguaMcpStatus?.path && (
              <p className="text-xs text-gray-500 font-mono">Path: {baguaMcpStatus.path}</p>
            )}
            {baguaMcpStatus?.version && (
              <p className="text-xs text-gray-500 font-mono">Version: {baguaMcpStatus.version}</p>
            )}
            {baguaMcpConnection && (
              <div className="mt-2 flex items-center gap-3 text-xs">
                {baguaMcpConnection.connected ? (
                  <span className="flex items-center gap-1 text-[#238636]">
                    <CheckCircle className="size-3" /> Connected — {baguaMcpConnection.toolCount} tools
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-[#da3633]">
                    <AlertTriangle className="size-3" /> {baguaMcpConnection.error || "Connection failed"}
                  </span>
                )}
              </div>
            )}
          </div>
          <div className="flex items-center gap-2 ml-2 shrink-0">
            <button
              onClick={onDetectBagua}
              className="flex items-center gap-1 px-2 py-1 text-xs bg-[#21262d] text-gray-300 hover:bg-[#30363d] rounded transition-colors"
            >
              <RefreshCw className="size-3" /> Detect
            </button>
            {!baguaMcpStatus?.installed && (
              <button
                onClick={() => navigator.clipboard.writeText("npm install -g ga-semantics-mcp")}
                className="flex items-center gap-1 px-2 py-1 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
              >
                <Download className="size-3" /> Install
              </button>
            )}
            {baguaMcpStatus?.installed && (
              <button
                onClick={onTestBagua}
                className="flex items-center gap-1 px-2 py-1 text-xs bg-[#238636] hover:bg-[#2ea043] text-white rounded transition-colors"
              >
                <Plug className="size-3" /> Test
              </button>
            )}
          </div>
        </div>
      </div>

      {mcps.length === 0 && (
        <EmptyState
          icon={Server}
          message="No MCPs loaded. Enter a config file path and click Load MCPs."
        />
      )}
      {mcps.map((mcp) => (
        <div
          key={mcp.id}
          className={cn(
            "bg-[#161b22] border rounded-lg p-4 transition-colors",
            mcp.enabled
              ? "border-[#30363d] hover:border-[#1f6feb]/50"
              : "border-[#30363d]/50 opacity-60"
          )}
        >
          <div className="flex items-start justify-between">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-1">
                <Server className="size-4 text-[#58a6ff]" />
                <span className="font-medium text-gray-200">{mcp.name}</span>
                <HealthBadge health={mcp.health} />
                <span className="text-xs text-gray-500">{mcp.source}</span>
              </div>
              <p className="text-xs text-gray-500 mb-1 font-mono">
                {mcp.server_command}{" "}
                {mcp.args.length > 0 && mcp.args.join(" ")}
              </p>
              {Object.keys(mcp.env).length > 0 && (
                <div className="flex flex-wrap gap-1 mt-1">
                  {Object.entries(mcp.env).map(([k]) => (
                    <span
                      key={k}
                      className="px-1.5 py-0.5 text-xs bg-[#21262d] text-gray-400 rounded"
                    >
                      {k}
                    </span>
                  ))}
                </div>
              )}
            </div>
            <div className="flex items-center gap-2 ml-2 shrink-0">
              <button
                onClick={() =>
                  onToggle(mcpPath, mcp.name, !mcp.enabled)
                }
                className={cn(
                  "relative inline-flex h-5 w-9 items-center rounded-full transition-colors",
                  mcp.enabled ? "bg-[#238636]" : "bg-[#30363d]"
                )}
              >
                <span
                  className={cn(
                    "inline-block h-4 w-4 transform rounded-full bg-white transition-transform",
                    mcp.enabled ? "translate-x-[18px]" : "translate-x-[2px]"
                  )}
                />
              </button>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Connector Vault ─────────────────────────────────────────────

function VaultTab({
  secrets,
  onAdd,
  onReveal,
}: {
  secrets: VaultEntry[];
  onAdd: (
    key: string,
    value: string,
    scope: string,
    agentId?: string,
    projectId?: string
  ) => Promise<void>;
  onReveal: (id: string) => Promise<string>;
}) {
  const [showAdd, setShowAdd] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [newScope, setNewScope] = useState("global");
  const [newAgentId, setNewAgentId] = useState("");
  const [revealedValues, setRevealedValues] = useState<
    Record<string, string>
  >({});

  const handleAdd = async () => {
    if (!newKey || !newValue) return;
    await onAdd(
      newKey,
      newValue,
      newScope,
      newAgentId || undefined,
      undefined
    );
    setNewKey("");
    setNewValue("");
    setShowAdd(false);
  };

  const handleReveal = async (id: string) => {
    if (revealedValues[id]) {
      setRevealedValues((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      return;
    }
    const val = await onReveal(id);
    setRevealedValues((prev) => ({ ...prev, [id]: val }));
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <span className="text-sm text-gray-400">
          Stored API keys, tokens, and credentials
        </span>
        <button
          onClick={() => setShowAdd(!showAdd)}
          className="flex items-center gap-1 px-3 py-1.5 text-xs bg-[#238636] hover:bg-[#2ea043] text-white rounded transition-colors"
        >
          <Plus className="size-3" /> Add Secret
        </button>
      </div>

      {showAdd && (
        <div className="bg-[#161b22] border border-[#30363d] rounded-lg p-4">
          <div className="grid grid-cols-2 gap-3 mb-3">
            <div>
              <label className="block text-xs text-gray-400 mb-1">
                Key Name
              </label>
              <div className="flex flex-wrap gap-1.5 mb-2">
                {VAULT_KEY_PRESETS.map((preset) => (
                  <button
                    key={preset}
                    type="button"
                    onClick={() => setNewKey(preset)}
                    className={cn(
                      "px-2 py-0.5 text-[11px] font-mono rounded border transition-colors",
                      newKey === preset
                        ? "bg-[#1f6feb]/20 border-[#1f6feb] text-[#58a6ff]"
                        : "bg-[#0d1117] border-[#30363d] text-gray-400 hover:border-gray-500 hover:text-gray-200"
                    )}
                  >
                    {preset}
                  </button>
                ))}
              </div>
              <input
                type="text"
                value={newKey}
                onChange={(e) => setNewKey(e.target.value)}
                placeholder="e.g. OPENAI_API_KEY"
                className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">
                Scope
              </label>
              <select
                value={newScope}
                onChange={(e) => setNewScope(e.target.value)}
                className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
              >
                <option value="global">Global</option>
                <option value="agent">Agent</option>
                <option value="project">Project</option>
              </select>
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">
                Value
              </label>
              <input
                type="password"
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                placeholder="Enter secret value"
                className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
              />
            </div>
            <div>
              <label htmlFor="vault-agent-scope" className="block text-xs text-gray-400 mb-1">
                Agent Scope
              </label>
              <select
                id="vault-agent-scope"
                value={newAgentId}
                onChange={(e) => setNewAgentId(e.target.value)}
                className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 focus:outline-none focus:border-[#1f6feb]"
              >
                <option value="">All Agents (Global)</option>
                <option value="pi">Pi</option>
                {AGENT_CONFIGS.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.label}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowAdd(false)}
              className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-300 transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              className="px-3 py-1.5 text-xs bg-[#1f6feb] hover:bg-[#388bfd] text-white rounded transition-colors"
            >
              Store Secret
            </button>
          </div>
        </div>
      )}

      {secrets.length === 0 && !showAdd && (
        <EmptyState
          icon={KeyRound}
          message="No secrets stored. Click Add Secret to store credentials."
        />
      )}

      <div className="grid grid-cols-1 gap-3">
        {secrets.map((secret) => (
          <div
            key={secret.id}
            className="bg-[#161b22] border border-[#30363d] rounded-lg p-4 hover:border-[#1f6feb]/50 transition-colors"
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <KeyRound className="size-4 text-[#d29922]" />
                <span className="font-medium text-gray-200 font-mono text-sm">
                  {secret.key_name}
                </span>
                <ScopeBadge scope={secret.scope} />
                {secret.agent_id && (
                  <span className="text-xs text-gray-500">
                    agent: {secret.agent_id}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-500 font-mono">
                  {revealedValues[secret.id] || secret.masked_value}
                </span>
                <button
                  onClick={() => handleReveal(secret.id)}
                  className="ml-1 text-gray-400 hover:text-gray-300 transition-colors"
                >
                  {revealedValues[secret.id] ? (
                    <EyeOff className="size-3.5" />
                  ) : (
                    <Eye className="size-3.5" />
                  )}
                </button>
                <button
                  onClick={() => {
                    if (revealedValues[secret.id]) {
                      navigator.clipboard.writeText(
                        revealedValues[secret.id]
                      );
                    }
                  }}
                  className="text-gray-400 hover:text-gray-300 transition-colors"
                >
                  <Copy className="size-3.5" />
                </button>
              </div>
            </div>
            <p className="text-xs text-gray-500 mt-1">
              Created: {secret.created_at.substring(0, 19)}
            </p>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Plugin Manager ──────────────────────────────────────────────

function PluginsTab({
  plugins,
  profile,
}: {
  plugins: string[];
  profile: Record<string, unknown> | null;
}) {
  return (
    <div className="grid grid-cols-1 gap-4">
      {profile && (
        <div className="bg-[#161b22] border border-[#1f6feb]/30 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Terminal className="size-4 text-[#58a6ff]" />
            <span className="font-medium text-gray-200">Project Profile</span>
          </div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm">
            <div>
              <span className="text-gray-500">Name:</span>{" "}
              <span className="text-gray-300">{profile.name as string}</span>
            </div>
            <div>
              <span className="text-gray-500">Path:</span>{" "}
              <span className="text-gray-300 font-mono text-xs">
                {profile.path as string}
              </span>
            </div>
            <div>
              <span className="text-gray-500">Stack:</span>{" "}
              <span className="text-gray-300">
                {(profile.stack as string[])?.join(", ") || "unknown"}
              </span>
            </div>
            <div>
              <span className="text-gray-500">Test FW:</span>{" "}
              <span className="text-gray-300">
                {(profile.test_framework as string) || "none"}
              </span>
            </div>
            <div>
              <span className="text-gray-500">Package Mgr:</span>{" "}
              <span className="text-gray-300">
                {(profile.package_manager as string) || "unknown"}
              </span>
            </div>
            <div>
              <span className="text-gray-500">Active Agents:</span>{" "}
              <span className="text-gray-300">
                {(profile.active_agents as string[])?.join(", ") || "-"}
              </span>
            </div>
          </div>
        </div>
      )}

      <div>
        <div className="flex items-center gap-2 mb-3">
          <Puzzle className="size-4 text-[#58a6ff]" />
          <span className="font-medium text-gray-200">
            VS Code Extensions ({plugins.length})
          </span>
        </div>
        {plugins.length === 0 && (
          <EmptyState icon={Puzzle} message="No plugins found. Click Scan Plugins." />
        )}
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2">
          {plugins.map((plugin) => (
            <div
              key={plugin}
              className="bg-[#161b22] border border-[#30363d] rounded px-3 py-2 text-sm text-gray-300 hover:border-[#1f6feb]/50 transition-colors"
            >
              <div className="flex items-center gap-2">
                <Puzzle className="size-3.5 text-gray-500" />
                <span className="truncate">{plugin}</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ─── Shared Components ───────────────────────────────────────────

function EmptyState({ icon: Icon, message }: { icon: typeof FolderOpen; message: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-gray-500">
      <Icon className="size-10 mb-3 opacity-40" />
      <p className="text-sm">{message}</p>
    </div>
  );
}

function SourceBadge({ source }: { source: string }) {
  const colors: Record<string, string> = {
    claude: "bg-[#d29922]/20 text-[#d29922] border-[#d29922]/30",
    opencode: "bg-[#58a6ff]/20 text-[#58a6ff] border-[#58a6ff]/30",
    gemini: "bg-[#a371f7]/20 text-[#a371f7] border-[#a371f7]/30",
    bundled: "bg-[#7ee787]/20 text-[#7ee787] border-[#7ee787]/30",
    custom: "bg-[#7ee787]/20 text-[#7ee787] border-[#7ee787]/30",
  };
  return (
    <span
      className={cn(
        "px-1.5 py-0.5 text-xs rounded border",
        colors[source] || colors.custom
      )}
    >
      {source}
    </span>
  );
}

function AgentBadge({ agent }: { agent: string }) {
  const colors: Record<string, string> = {
    claude: "bg-[#d29922]/20 text-[#d29922] border-[#d29922]/30",
    opencode: "bg-[#58a6ff]/20 text-[#58a6ff] border-[#58a6ff]/30",
    gemini: "bg-[#a371f7]/20 text-[#a371f7] border-[#a371f7]/30",
  };
  return (
    <span
      className={cn(
        "px-1.5 py-0.5 text-xs rounded border",
        colors[agent] || "bg-[#30363d]/50 text-gray-400 border-[#30363d]"
      )}
    >
      {agent}
    </span>
  );
}

function HealthBadge({ health }: { health: string }) {
  const icons: Record<string, typeof CheckCircle> = {
    green: CheckCircle,
    grey: Circle,
    red: AlertTriangle,
  };
  const Icon = icons[health] || Circle;
  const colors: Record<string, string> = {
    green: "text-[#238636]",
    grey: "text-gray-500",
    red: "text-[#da3633]",
  };
  return <Icon className={cn("size-3.5", colors[health] || colors.grey)} />;
}

function ScopeBadge({ scope }: { scope: string }) {
  const colors: Record<string, string> = {
    global: "bg-[#a371f7]/20 text-[#a371f7] border-[#a371f7]/30",
    agent: "bg-[#58a6ff]/20 text-[#58a6ff] border-[#58a6ff]/30",
    project: "bg-[#7ee787]/20 text-[#7ee787] border-[#7ee787]/30",
  };
  return (
    <span
      className={cn(
        "px-1.5 py-0.5 text-xs rounded border",
        colors[scope] || colors.global
      )}
    >
      {scope}
    </span>
  );
}

function SkillContentModal({
  content,
  onClose,
}: {
  content: string;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="bg-[#161b22] border border-[#30363d] rounded-lg w-[700px] max-h-[80vh] flex flex-col shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#30363d]">
          <span className="text-sm font-medium text-gray-200">
            Skill Content
          </span>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-300"
          >
            <X className="size-4" />
          </button>
        </div>
        <div className="flex-1 overflow-auto p-4">
          <pre className="text-sm text-gray-300 whitespace-pre-wrap font-mono">
            {content}
          </pre>
        </div>
      </div>
    </div>
  );
}
