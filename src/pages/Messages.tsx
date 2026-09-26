import { useState, useEffect } from "react";
import { useOrchestrationStore, type ACBSignal } from "@/stores/orchestrationStore";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import {
  MessageSquare,
  CheckCircle2,
  Filter,
  Send,
  ArrowRight,
  Clock,
} from "lucide-react";

const PRIORITY_COLORS: Record<string, string> = {
  HIGH: "text-red-400 bg-red-500/20 border-red-500/30",
  MEDIUM: "text-yellow-400 bg-yellow-500/20 border-yellow-500/30",
  INFO: "text-[#58a6ff] bg-[#58a6ff]/20 border-[#58a6ff]/30",
  LOW: "text-gray-400 bg-gray-500/20 border-gray-500/30",
};

export function MessagePanel() {
  const store = useOrchestrationStore();
  const [sessionFilter, setSessionFilter] = useState("");
  const [lineInput, setLineInput] = useState("");
  const [parsedSignal, setParsedSignal] = useState<ACBSignal | null>(null);
  const [priorityFilter, setPriorityFilter] = useState<string | null>(null);
  const [hasLoaded, setHasLoaded] = useState(false);

  useEffect(() => {
    if (!hasLoaded) {
      store.getOpenSignals();
      setHasLoaded(true);
    }
  }, [hasLoaded, store]);

  const handleParse = async () => {
    if (!lineInput.trim()) return;
    const signal = await store.parseAcbSignal(lineInput);
    setParsedSignal(signal);
  };

  const handleRecord = async () => {
    if (!parsedSignal) return;
    await store.recordAcbSignal(parsedSignal);
    setLineInput("");
    setParsedSignal(null);
  };

  const handleResolve = async (signalId: string) => {
    await store.resolveSignal(signalId);
  };

  const signals = store.acbSignals ?? [];
  const filtered = signals.filter((s) => {
    if (priorityFilter && s.priority !== priorityFilter) return false;
    return true;
  });

  return (
    <>
      {/* Signal Parser */}
      <Card className="p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Send className="size-4 text-gray-400" />
          <span className="text-sm font-medium text-gray-300">Parse ACB Signal</span>
        </div>
        <div className="flex gap-3">
          <div className="flex-1">
            <Input
              value={lineInput}
              onChange={(e) => setLineInput(e.target.value)}
              placeholder='e.g., [ACC:STATUS from=claude to=ORCHESTRATOR priority=INFO] Tests passing, continuing to refactor'
              className="bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600"
            />
          </div>
          <Button onClick={handleParse} disabled={!lineInput.trim()} className="gap-1.5">
            <EyeIcon />
            Parse
          </Button>
        </div>
        {parsedSignal && (
          <div className="bg-[#161b22] border border-[#30363d] rounded p-3 space-y-2">
            <div className="flex items-center gap-3 text-xs">
              <span className="text-gray-400">
                From: <span className="text-[#58a6ff]">{parsedSignal.from_agent || "(none)"}</span>
              </span>
              <span className="text-gray-400">
                To: <span className="text-[#a371f7]">{parsedSignal.to_agent}</span>
              </span>
              <Badge variant="outline" className={cn("text-xs", PRIORITY_COLORS[parsedSignal.priority] || PRIORITY_COLORS.INFO)}>
                {parsedSignal.priority}
              </Badge>
              <span className="text-gray-500">Type: {parsedSignal.signal_type}</span>
            </div>
            <p className="text-sm text-gray-300 bg-[#0d1117] rounded p-2 font-mono">
              {parsedSignal.body}
            </p>
            <Button size="sm" onClick={handleRecord} className="gap-1.5">
              <CheckCircle2 className="size-3.5" />
              Record Signal
            </Button>
          </div>
        )}
      </Card>

      {/* Filters */}
      <div className="flex items-center gap-2">
        <Filter className="size-4 text-gray-500" />
        <Input
          value={sessionFilter}
          onChange={(e) => setSessionFilter(e.target.value)}
          placeholder="Filter by session ID..."
          className="w-72 bg-[#0d1117] border-[#30363d] text-gray-300 placeholder:text-gray-600 text-sm h-8"
        />
        <div className="flex gap-1">
          {["HIGH", "MEDIUM", "INFO", "LOW"].map((p) => (
            <Badge
              key={p}
              variant="outline"
              className={cn(
                "cursor-pointer text-xs transition-colors",
                priorityFilter === p
                  ? cn("bg-opacity-30", PRIORITY_COLORS[p])
                  : "border-[#30363d] text-gray-500 hover:border-[#58a6ff]/50"
              )}
              onClick={() => setPriorityFilter(priorityFilter === p ? null : p)}
            >
              {p}
            </Badge>
          ))}
        </div>
        <div className="flex-1" />
        <span className="text-xs text-gray-500">{filtered.length} open signals</span>
      </div>

      {/* Signal List */}
      <ScrollArea className="flex-1">
        <div className="space-y-2">
          {filtered.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-gray-500 gap-3">
              <MessageSquare className="size-10 opacity-30" />
              <p className="text-sm">No open ACB signals.</p>
              <p className="text-xs text-gray-600">Parse an ACB signal line or wait for agents to communicate.</p>
            </div>
          )}
          {filtered.map((signal) => (
            <SignalCard key={signal.id} signal={signal} onResolve={handleResolve} />
          ))}
        </div>
      </ScrollArea>
    </>
  );
}

export default function Messages() {
  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="page-header">
        <div className="gradient-accent-bar" />
        <h1>ACB Message Bus</h1>
      </div>
      <MessagePanel />
    </div>
  );
}

function SignalCard({ signal, onResolve }: { signal: ACBSignal; onResolve: (id: string) => void }) {
  return (
    <Card className={cn(
      "p-3 border transition-colors",
      signal.priority === "HIGH"
        ? "border-red-500/30 bg-[#161b22]"
        : signal.priority === "MEDIUM"
          ? "border-yellow-500/30 bg-[#161b22]"
          : "border-[#30363d] hover:border-[#1f6feb]/30"
    )}>
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1.5 flex-wrap">
            <span className="text-xs text-[#58a6ff] font-mono">{signal.from_agent}</span>
            <ArrowRight className="size-3 text-gray-600" />
            <span className="text-xs text-[#a371f7] font-mono">{signal.to_agent}</span>
            <Badge variant="outline" className={cn("text-xs", PRIORITY_COLORS[signal.priority] || PRIORITY_COLORS.INFO)}>
              {signal.priority}
            </Badge>
            <Badge variant="outline" className="text-xs border-[#30363d] text-gray-400">
              {signal.signal_type}
            </Badge>
            {signal.wave !== null && (
              <Badge variant="outline" className="text-xs border-[#30363d] text-gray-500">
                Wave {signal.wave}
              </Badge>
            )}
            <span className="text-xs text-gray-600 flex items-center gap-1">
              <Clock className="size-3" />
              {signal.created_at.substring(0, 19).replace("T", " ")}
            </span>
          </div>
          <pre className="text-sm text-gray-300 bg-[#0d1117] rounded p-2 whitespace-pre-wrap max-h-32 overflow-y-auto font-mono">
            {signal.body}
          </pre>
          {signal.session_id && (
            <p className="text-xs text-gray-600 mt-1">Session: {signal.session_id}</p>
          )}
        </div>
        <Button
          size="sm"
          variant="outline"
          className="h-7 text-xs gap-1 shrink-0"
          onClick={() => onResolve(signal.id)}
        >
          <CheckCircle2 className="size-3" />
          Resolve
        </Button>
      </div>
    </Card>
  );
}

function EyeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}
