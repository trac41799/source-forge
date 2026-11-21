import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useOrchestrationStore, type HandoffEnvelope } from "@/stores/orchestrationStore";
import { AGENT_CONFIGS } from "@/lib/agents/configs";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  FileCheck,
  CheckCircle2,
  XCircle,
  Send,
  Eye,
  Copy,
  FolderOpen,
} from "lucide-react";

const HANDOFF_AGENT_OPTIONS = [
  { id: "pi", label: "Pi Agent (ACP)" },
  ...AGENT_CONFIGS.map((a) => ({ id: a.id, label: a.label })),
];

interface AgentSelectFieldProps {
  id: string;
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
}

function AgentSelectField({ id, label, placeholder, value, onChange }: AgentSelectFieldProps) {
  const hasKnownOption = !value || HANDOFF_AGENT_OPTIONS.some((a) => a.id === value);
  return (
    <div>
      <label htmlFor={id} className="block text-xs text-gray-400 mb-1">
        {label}
      </label>
      <select
        id={id}
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full h-9 px-2 text-sm bg-[#0d1117] border border-[#30363d] rounded-md text-gray-300 focus:outline-none focus:border-[#1f6feb]"
      >
        <option value="">{placeholder}</option>
        {HANDOFF_AGENT_OPTIONS.map((agent) => (
          <option key={agent.id} value={agent.id}>
            {agent.label}
          </option>
        ))}
        {!hasKnownOption && <option value={value}>{value}</option>}
      </select>
    </div>
  );
}

interface ValidatedSection {
  name: string;
  present: boolean;
}

export function HandoffPanel() {
  const store = useOrchestrationStore();
  const [content, setContent] = useState("");
  const [envelope, setEnvelope] = useState<HandoffEnvelope>({
    original_task: "",
    completed_by: "",
    model_used: "",
    output_summary: "",
    changed_files: [],
    diff_preview: "",
    handoff_instruction: "",
    next_agent: "",
    next_model: "",
  });
  const [validatedSections, setValidatedSections] = useState<ValidatedSection[] | null>(null);
  const [handoffOutput, setHandoffOutput] = useState("");
  const [loadedFilePath, setLoadedFilePath] = useState<string | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);

  const handleBrowseHandoff = async () => {
    setParseError(null);
    try {
      const selected = await open({
        directory: false,
        filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
      });
      if (typeof selected !== "string" || !selected) return;

      const parsed = await store.parseHandoffFile(selected);
      const changedFiles = (parsed.changed_files ?? []).map((f) => `- ${f}`).join("\n");
      const formatted = [
        `## Completed Work\n${parsed.output_summary || parsed.original_task}`,
        `## Test Results\n- Verified by ${parsed.completed_by || "agent"} (${parsed.model_used || "default"})`,
        `## Interface Contracts Exposed\n${changedFiles || "- None"}`,
        `## Files NOT Modified\n- Unscoped workspace files untouched`,
        `## Design Decisions\n- Task: ${parsed.original_task}`,
        `## Handoff Instructions\n${parsed.handoff_instruction}`,
      ].join("\n\n");

      setContent(formatted);
      setEnvelope((prev) => ({
        ...prev,
        original_task: parsed.original_task || prev.original_task,
        completed_by: parsed.completed_by || prev.completed_by,
        model_used: parsed.model_used || prev.model_used,
        output_summary: parsed.output_summary || prev.output_summary,
        changed_files: parsed.changed_files?.length ? parsed.changed_files : prev.changed_files,
        handoff_instruction: parsed.handoff_instruction || prev.handoff_instruction,
      }));
      setLoadedFilePath(parsed.raw_path || selected);
    } catch (err) {
      setParseError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleValidate = async () => {
    if (!content.trim()) return;
    const result = await store.validateHandoff(content);
    setValidatedSections(
      result.missing.map((name) => ({ name, present: false })).concat(
        ["Completed Work", "Test Results", "Interface Contracts Exposed", "Files NOT Modified", "Design Decisions", "Handoff Instructions"]
          .filter((s) => !result.missing.includes(s))
          .map((name) => ({ name, present: true }))
      )
    );
  };

  const handleBuild = async () => {
    const result = await store.buildHandoff(envelope);
    setHandoffOutput(result);
  };

  const handleFileChange = (value: string) => {
    setEnvelope((prev) => ({
      ...prev,
      changed_files: value.split("\n").filter(Boolean),
    }));
  };

  return (
    <div className="grid grid-cols-2 gap-4 flex-1 min-h-0">
      {/* Left: Build Handoff Envelope */}
      <div className="flex flex-col gap-4 min-h-0">
          <Card className="p-4 space-y-3">
            <div className="flex items-center gap-2">
              <Send className="size-4 text-[#58a6ff]" />
              <span className="text-sm font-medium text-gray-300">Build Handoff Envelope</span>
            </div>
            <div className="grid grid-cols-1 gap-2">
              <div>
                <label className="block text-xs text-gray-400 mb-1">Original Task</label>
                <Input
                  value={envelope.original_task}
                  onChange={(e) => setEnvelope((p) => ({ ...p, original_task: e.target.value }))}
                  placeholder="Describe the original task"
                  className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <AgentSelectField
                  id="handoff-completed-by"
                  label="Completed By"
                  placeholder="Select agent..."
                  value={envelope.completed_by}
                  onChange={(value) => setEnvelope((p) => ({ ...p, completed_by: value }))}
                />
                <div>
                  <label className="block text-xs text-gray-400 mb-1">Model Used</label>
                  <Input
                    value={envelope.model_used}
                    onChange={(e) => setEnvelope((p) => ({ ...p, model_used: e.target.value }))}
                    placeholder="e.g., sonnet-4"
                    className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <AgentSelectField
                  id="handoff-next-agent"
                  label="Next Agent"
                  placeholder="Select next agent..."
                  value={envelope.next_agent}
                  onChange={(value) => setEnvelope((p) => ({ ...p, next_agent: value }))}
                />
                <div>
                  <label className="block text-xs text-gray-400 mb-1">Next Model</label>
                  <Input
                    value={envelope.next_model}
                    onChange={(e) => setEnvelope((p) => ({ ...p, next_model: e.target.value }))}
                    placeholder="e.g., gemini-2.5-pro"
                    className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
                  />
                </div>
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Output Summary</label>
                <textarea
                  value={envelope.output_summary}
                  onChange={(e) => setEnvelope((p) => ({ ...p, output_summary: e.target.value }))}
                  rows={2}
                  className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 placeholder:text-gray-600 focus:outline-none focus:border-[#1f6feb] resize-none"
                  placeholder="Brief summary of what was done"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Changed Files (one per line)</label>
                <textarea
                  value={envelope.changed_files.join("\n")}
                  onChange={(e) => handleFileChange(e.target.value)}
                  rows={3}
                  className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 placeholder:text-gray-600 focus:outline-none focus:border-[#1f6feb] resize-none"
                  placeholder="file1.ts&#10;file2.ts"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Diff Preview</label>
                <textarea
                  value={envelope.diff_preview}
                  onChange={(e) => setEnvelope((p) => ({ ...p, diff_preview: e.target.value }))}
                  rows={3}
                  className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 placeholder:text-gray-600 focus:outline-none focus:border-[#1f6feb] resize-none font-mono"
                  placeholder="+ added line&#10;- removed line"
                />
              </div>
              <div>
                <label className="block text-xs text-gray-400 mb-1">Handoff Instructions</label>
                <textarea
                  value={envelope.handoff_instruction}
                  onChange={(e) => setEnvelope((p) => ({ ...p, handoff_instruction: e.target.value }))}
                  rows={2}
                  className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 placeholder:text-gray-600 focus:outline-none focus:border-[#1f6feb] resize-none"
                  placeholder="Instructions for the next agent"
                />
              </div>
            </div>
            <Button onClick={handleBuild} className="gap-1.5 w-full">
              <FileCheck className="size-4" />
              Generate Handoff
            </Button>
          </Card>
        </div>

        {/* Right: Validate & Preview */}
        <div className="flex flex-col gap-4 min-h-0">
          {/* Validation */}
          <Card className="p-4 space-y-3">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <Eye className="size-4 text-[#d29922]" />
                <span className="text-sm font-medium text-gray-300">Validate Handoff Schema</span>
              </div>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={handleBrowseHandoff}
                className="gap-1.5 text-xs"
              >
                <FolderOpen className="size-3.5" />
                Browse Handoff File...
              </Button>
            </div>
            {loadedFilePath && (
              <p className="text-xs text-green-400 truncate">Loaded: {loadedFilePath}</p>
            )}
            {parseError && (
              <p className="text-xs text-red-400">{parseError}</p>
            )}
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              rows={4}
              className="w-full px-2 py-1.5 text-sm bg-[#0d1117] border border-[#30363d] rounded text-gray-300 placeholder:text-gray-600 focus:outline-none focus:border-[#1f6feb] resize-none"
              placeholder="Paste handoff content here to validate schema..."
            />
            <Button onClick={handleValidate} variant="outline" className="gap-1.5 w-full">
              <Eye className="size-4" />
              Validate Schema
            </Button>
            {validatedSections && (
              <div className="space-y-1.5 mt-2">
                {validatedSections.map((section) => (
                  <div key={section.name} className="flex items-center gap-2 text-xs">
                    {section.present ? (
                      <CheckCircle2 className="size-3.5 text-green-500" />
                    ) : (
                      <XCircle className="size-3.5 text-red-500" />
                    )}
                    <span className={section.present ? "text-green-400" : "text-red-400"}>
                      {section.name}
                    </span>
                    {!section.present && (
                      <Badge variant="outline" className="text-xs border-red-500/30 text-red-400">
                        missing
                      </Badge>
                    )}
                  </div>
                ))}
              </div>
            )}
          </Card>

          {/* Handoff Output */}
          {handoffOutput && (
            <Card className="p-4 flex-1 min-h-0 flex flex-col">
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <FileCheck className="size-4 text-green-400" />
                  <span className="text-sm font-medium text-gray-300">Generated Handoff</span>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 text-xs gap-1"
                  onClick={() => navigator.clipboard.writeText(handoffOutput)}
                >
                  <Copy className="size-3" /> Copy
                </Button>
              </div>
              <ScrollArea className="flex-1">
                <pre className="text-xs text-gray-300 font-mono whitespace-pre-wrap">
                  {handoffOutput}
                </pre>
              </ScrollArea>
            </Card>
          )}
        </div>
      </div>
  );
}

export default function Handoffs() {
  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="page-header">
        <div className="gradient-accent-bar" />
        <h1>Handoff Monitor</h1>
      </div>
      <HandoffPanel />
    </div>
  );
}
