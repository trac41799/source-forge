import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useOrchestrationStore, type MemoryCandidate } from "@/stores/orchestrationStore";
import { useMemoryStore } from "@/stores/memoryStore";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Boxes, Download, Upload, Brain, FileText, FileJson, Copy, Check, Play } from "lucide-react";

export default function Playbooks() {
  const {
    memoryCandidates, getMemoryCandidates,
    playbookManifest, buildPlaybookManifest,
    buildFeatureDocPrompt,
  } = useOrchestrationStore();

  const [exportName, setExportName] = useState("");
  const [includeSkills, setIncludeSkills] = useState(true);
  const [includeMemory, setIncludeMemory] = useState(true);
  const [includePresets, setIncludePresets] = useState(true);
  const [copied, setCopied] = useState(false);
  const [importedFilePath, setImportedFilePath] = useState<string | null>(null);
  const [candidateFeedback, setCandidateFeedback] = useState<Record<string, string>>({});

  useEffect(() => {
    getMemoryCandidates();
  }, []);

  const handleSelectAccFile = async () => {
    try {
      const selected = await open({
        directory: false,
        filters: [{ name: "ACC Playbook", extensions: ["acc", "json", "zip"] }],
      });
      if (typeof selected === "string" && selected) {
        setImportedFilePath(selected);
      }
    } catch (err) {
      console.error("Failed to select playbook file:", err);
    }
  };

  const updateCandidateStatus = (candidateId: string, status: string, feedback: string) => {
    useOrchestrationStore.setState((state) => ({
      memoryCandidates: state.memoryCandidates.map((c) =>
        c.id === candidateId ? { ...c, status } : c
      ),
    }));
    setCandidateFeedback((prev) => ({ ...prev, [candidateId]: feedback }));
  };

  const handleAddToMemory = async (candidate: MemoryCandidate) => {
    try {
      await useMemoryStore.getState().createFact({
        agent_id: "playbook",
        session_id: candidate.session_id || "playbook",
        user_id: "local",
        org_id: candidate.project_id || "default",
        fact_type: "learning",
        content: candidate.content,
        metadata: candidate.source_pattern ?? undefined,
        confidence: 0.9,
      });
    } catch (err) {
      console.error("Failed to persist memory fact:", err);
    }
    updateCandidateStatus(candidate.id, "approved", "Added to memory");
  };

  const handleSkipCandidate = (candidate: MemoryCandidate) => {
    updateCandidateStatus(candidate.id, "skipped", "Skipped");
  };

  const handleExport = async () => {
    await buildPlaybookManifest(
      exportName || "my-playbook",
      "current-project",
      ["typescript", "react", "tauri"],
      includeSkills,
      includeMemory,
      includePresets,
    );
  };

  const handleCopyManifest = () => {
    if (playbookManifest) {
      navigator.clipboard.writeText(JSON.stringify(playbookManifest, null, 2));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="flex h-full flex-col p-6 gap-6">
      <div>
        <div className="page-header">
          <div className="gradient-accent-bar" />
          <h1>Playbooks</h1>
        </div>
        <p className="text-sm text-muted-foreground mt-1">Export and import .acc bundles for team onboarding</p>
      </div>

      <div className="grid grid-cols-2 gap-6 flex-1 min-h-0">
        <div className="flex flex-col gap-4">
          <Card className="p-6 space-y-4">
            <div className="flex items-center gap-2">
              <Boxes className="size-5" />
              <h2 className="text-lg font-semibold">Export Playbook</h2>
            </div>
            <Separator />
            <div className="space-y-3">
              <div>
                <label className="text-sm font-medium">Playbook Name</label>
                <Input
                  placeholder="my-client-playbook"
                  value={exportName}
                  onChange={(e) => setExportName(e.target.value)}
                  className="mt-1"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Include</label>
                {[
                  { key: includeSkills, label: "Skills Library", setter: setIncludeSkills },
                  { key: includeMemory, label: "Memory Files", setter: setIncludeMemory },
                  { key: includePresets, label: "Preset Commands", setter: setIncludePresets },
                ].map(({ key, label, setter }) => (
                  <label key={label} className="flex items-center gap-3 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={key}
                      onChange={(e) => setter(e.target.checked)}
                      className="rounded border-border"
                    />
                    <span className="text-sm">{label}</span>
                  </label>
                ))}
              </div>
              <Button className="w-full" onClick={handleExport}>
                <Download className="size-4 mr-2" /> Generate Manifest
              </Button>
            </div>

            {playbookManifest && (
              <Card className="p-4 bg-secondary/50 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">Manifest Ready</span>
                  <Button variant="ghost" size="sm" onClick={handleCopyManifest}>
                    {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
                  </Button>
                </div>
                <pre className="text-xs bg-secondary rounded p-2 overflow-auto max-h-48">
                  {JSON.stringify(playbookManifest, null, 2)}
                </pre>
              </Card>
            )}
          </Card>

          <Card className="p-6 space-y-4">
            <div className="flex items-center gap-2">
              <Upload className="size-5" />
              <h2 className="text-lg font-semibold">Import Playbook</h2>
            </div>
            <Separator />
            <div className="border-2 border-dashed border-border rounded-lg p-8 text-center space-y-3">
              <Boxes className="size-8 mx-auto opacity-30" />
              <p className="text-sm text-muted-foreground">Drop a .acc playbook file here</p>
              <Button type="button" variant="outline" size="sm" onClick={handleSelectAccFile}>
                <FileJson className="size-4 mr-2" /> Select .acc File
              </Button>
              {importedFilePath && (
                <div className="mt-2 rounded-md border border-green-500/30 bg-green-500/10 px-3 py-2 text-xs text-green-400 flex items-center justify-center gap-2">
                  <Check className="size-3.5 shrink-0" />
                  <span className="truncate">Imported: {importedFilePath}</span>
                </div>
              )}
            </div>
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card className="p-6 space-y-4">
            <div className="flex items-center gap-2">
              <Brain className="size-5" />
              <h2 className="text-lg font-semibold">Reactive Memory</h2>
            </div>
            <Separator />
            <p className="text-sm text-muted-foreground">
              Learnings detected from PTY output. Review and approve to add to project memory.
            </p>
            <ScrollArea className="h-[300px]">
              <div className="space-y-2 pr-2">
                {memoryCandidates.length === 0 && (
                  <p className="text-sm text-muted-foreground text-center py-8">
                    No memory candidates yet. They appear when agents surface learnings during sessions.
                  </p>
                )}
                {memoryCandidates.map((candidate) => (
                  <Card key={candidate.id} className="p-3 space-y-2">
                    <div className="flex items-start justify-between gap-2">
                      <p className="text-sm flex-1">{candidate.content}</p>
                      <Badge variant={candidate.status === "approved" ? "default" : "secondary"}>
                        {candidate.status}
                      </Badge>
                    </div>
                    {candidate.source_pattern && (
                      <p className="text-xs text-muted-foreground">
                        Pattern: <code className="text-xs">{candidate.source_pattern}</code>
                      </p>
                    )}
                    <div className="flex items-center gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={candidate.status === "approved" || candidate.status === "skipped"}
                        onClick={() => handleAddToMemory(candidate)}
                      >
                        Add to Memory
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={candidate.status === "approved" || candidate.status === "skipped"}
                        onClick={() => handleSkipCandidate(candidate)}
                      >
                        Skip
                      </Button>
                      {candidateFeedback[candidate.id] && (
                        <span className="text-xs text-muted-foreground ml-auto">
                          {candidateFeedback[candidate.id]}
                        </span>
                      )}
                    </div>
                  </Card>
                ))}
              </div>
            </ScrollArea>
          </Card>

          <Card className="p-6 space-y-4">
            <div className="flex items-center gap-2">
              <FileText className="size-5" />
              <h2 className="text-lg font-semibold">Feature Docs</h2>
            </div>
            <Separator />
            <p className="text-sm text-muted-foreground">
              Generate 4 canonical docs from session context after feature completion.
            </p>
            <div className="grid grid-cols-2 gap-2">
              {["EXECUTIVE_PLAN", "CHANGELOG", "QA_REPORT", "TECHNICAL_PLAN"].map((docType) => (
                <Button
                  key={docType}
                  variant="outline"
                  size="sm"
                  className="justify-start"
                  onClick={async () => {
                    const prompt = await buildFeatureDocPrompt(docType, "latest", "feature-name");
                    navigator.clipboard.writeText(prompt);
                  }}
                >
                  <Play className="size-3 mr-2" />
                  {docType.replace("_", " ")}
                </Button>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
