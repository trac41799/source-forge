import { useEffect, useRef, useState } from "react";
import { Route, Routes, Navigate } from "react-router-dom";
import { ThemeProvider } from "@/components/ThemeProvider";
import { Sidebar } from "@/components/layout/Sidebar";
import { RecoveryBanner } from "@/components/RecoveryBanner";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { PlaceholderPage } from "@/pages/placeholder";
import Runner from "@/pages/Runner";
import Integrations from "@/pages/Integrations";
import Assets from "@/pages/Assets";
import Outcomes from "@/pages/Outcomes";
import Replay from "@/pages/Replay";
import RoutePage from "@/pages/Route";
import Orchestrate from "@/pages/Orchestrate";
import Playbooks from "@/pages/Playbooks";
import CostAggregation from "@/pages/CostAggregation";
import Knowledge from "@/pages/Knowledge";
import Scheduler from "@/pages/Scheduler";
import Settings from "@/pages/Settings";
import BuildApp from "@/pages/BuildApp";
import { invoke } from "@tauri-apps/api/core";
import type { AppStateSnapshot } from "@/lib/types";

function App() {
  const hasCheckedRef = useRef(false);
  const [recoverySnapshot, setRecoverySnapshot] = useState<AppStateSnapshot | null>(null);

  useEffect(() => {
    if (hasCheckedRef.current) return;
    hasCheckedRef.current = true;

    (async () => {
      try {
        const snapshot: AppStateSnapshot = await invoke("load_app_state");
        if (snapshot.activeAgents.length > 0) {
          const savedAt = new Date(snapshot.savedAt).getTime();
          const tenMinAgo = Date.now() - 10 * 60 * 1000;
          if (savedAt > tenMinAgo) {
            setRecoverySnapshot(snapshot);
          } else {
            await invoke("clear_app_state");
          }
        }
      } catch {
        // First launch — no snapshot exists
      }
    })();

    const interval = setInterval(async () => {
      try {
        await invoke("save_app_state");
      } catch {
        // Silently ignore save errors
      }
    }, 30_000);

    return () => clearInterval(interval);
  }, []);

  return (
    <ThemeProvider>
      <div className="flex h-screen w-screen overflow-hidden bg-background">
        <Sidebar />
        <div className="flex-1 flex flex-col overflow-hidden">
          {recoverySnapshot && (
            <RecoveryBanner
              snapshot={recoverySnapshot}
              onDismiss={() => setRecoverySnapshot(null)}
            />
          )}
          <main className="flex-1 overflow-auto">
            <ErrorBoundary>
              <Routes>
              <Route path="/" element={<Navigate to="/runner" replace />} />
              <Route path="/runner" element={<Runner />} />
              <Route path="/route" element={<RoutePage />} />
              <Route path="/orchestrate" element={<Orchestrate />} />
              <Route path="/build" element={<BuildApp />} />
              <Route path="/orchestrate/handoffs" element={<Orchestrate />} />
              <Route path="/orchestrate/messages" element={<Orchestrate />} />
              <Route path="/handoffs" element={<Navigate to="/orchestrate/handoffs" replace />} />
              <Route path="/messages" element={<Navigate to="/orchestrate/messages" replace />} />
              <Route path="/assets" element={<Assets />} />
              <Route path="/outcomes" element={<Outcomes />} />
              <Route path="/replay" element={<Replay />} />
              <Route path="/playbooks" element={<Playbooks />} />
              <Route path="/integrations" element={<Integrations />} />
              <Route path="/connectors" element={<Navigate to="/integrations" replace />} />
              <Route path="/knowledge" element={<Knowledge />} />
              <Route path="/scheduler" element={<Scheduler />} />
              <Route path="/costs" element={<CostAggregation />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="*" element={<PlaceholderPage />} />
            </Routes>
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </ThemeProvider>
  );
}

export default App;
