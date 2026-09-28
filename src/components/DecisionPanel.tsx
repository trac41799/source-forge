import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IPC } from "@/lib/ipc/commands";

export interface DecisionConfig {
  backend: string;
  base_url: string;
  model: string;
  accept_threshold: number;
  review_threshold: number;
  context_limit: number;
  timeout_ms: number;
}

interface DecisionReview {
  id: string;
  consumer: string;
  decided_value: string | null;
  confidence: number | null;
  resolved: boolean;
}

/**
 * M5 (spec R52/R8): decision-layer backend selection, health, and review queue.
 * Reads configuration and health via the Rust decision commands.
 */
export function DecisionPanel() {
  const [config, setConfig] = useState<DecisionConfig | null>(null);
  const [health, setHealth] = useState("…");
  const [reviews, setReviews] = useState<DecisionReview[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        setConfig(await invoke<DecisionConfig>(IPC.getDecisionConfig));
        setHealth(await invoke<string>(IPC.decisionHealth));
        setReviews(await invoke<DecisionReview[]>(IPC.listDecisionReviews));
      } catch (e) {
        setError(String(e));
      }
    })();
  }, []);

  const save = async () => {
    if (!config) return;
    try {
      await invoke(IPC.setDecisionConfig, { config });
      setHealth(await invoke<string>(IPC.decisionHealth));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const resolve = async (id: string) => {
    try {
      await invoke(IPC.resolveDecisionReview, { id, resolution: "reviewed" });
      setReviews(await invoke<DecisionReview[]>(IPC.listDecisionReviews));
    } catch (e) {
      setError(String(e));
    }
  };

  if (error) {
    return (
      <div role="alert" className="font-mono text-xs text-red-400">
        {error}
      </div>
    );
  }
  if (!config) {
    return <div className="text-xs text-muted-foreground">Loading decision layer…</div>;
  }

  return (
    <div className="space-y-3" data-testid="decision-panel">
      <h3 className="text-sm font-medium text-foreground">Decision Layer</h3>
      <div className="text-xs text-muted-foreground">
        Backend health:{" "}
        <span data-testid="decision-health" className="font-mono text-foreground">
          {health}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <label className="text-xs text-muted-foreground">
          Backend
          <select
            aria-label="decision-backend"
            value={config.backend}
            onChange={(e) => setConfig({ ...config, backend: e.target.value })}
            className="mt-1 w-full rounded bg-muted px-2 py-1 text-xs text-foreground"
          >
            <option value="hosted">hosted</option>
            <option value="local">local</option>
          </select>
        </label>
        <label className="text-xs text-muted-foreground">
          Model
          <input
            aria-label="decision-model"
            value={config.model}
            onChange={(e) => setConfig({ ...config, model: e.target.value })}
            className="mt-1 w-full rounded bg-muted px-2 py-1 font-mono text-xs text-foreground"
          />
        </label>
      </div>

      <button
        onClick={save}
        className="rounded bg-primary px-3 py-1 text-xs text-primary-foreground"
      >
        Save
      </button>

      <div>
        <p className="text-xs text-muted-foreground">Review queue ({reviews.length})</p>
        <ul data-testid="decision-reviews" className="font-mono text-xs text-foreground">
          {reviews.map((r) => (
            <li key={r.id} className="flex items-center justify-between gap-2">
              <span>
                {r.consumer}: {r.decided_value ?? "â€”"} (
                {r.confidence != null ? r.confidence.toFixed(2) : "â€”"})
              </span>
              {!r.resolved && (
                <button onClick={() => resolve(r.id)} className="text-[10px] underline">
                  resolve
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
