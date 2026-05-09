import { useEffect, useState } from "react";
import { AutoSwitchConfig } from "./components/AutoSwitchConfig";
import { ProfileManager } from "./components/ProfileManager";
import { SessionGrid } from "./components/SessionGrid";
import { StatsPanel } from "./components/StatsPanel";
import { useDaemonStore } from "./store/daemon";
import { useProfileStore } from "./store/profiles";
import "./styles/neubrutalism.css";

type Tab = "profiles" | "sessions" | "auto-switch" | "stats";

const TABS: { id: Tab; label: string }[] = [
  { id: "profiles", label: "Profiles" },
  { id: "sessions", label: "Sessions" },
  { id: "auto-switch", label: "Auto-Switch" },
  { id: "stats", label: "Stats" },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("profiles");
  const { active, loading, fetch: fetchProfiles } = useProfileStore();
  const { status: daemonStatus, fetch: fetchDaemon } = useDaemonStore();

  useEffect(() => {
    fetchProfiles();
    fetchDaemon();
    // Refresh every 30s
    const interval = setInterval(() => { fetchProfiles(); fetchDaemon(); }, 30_000);
    return () => clearInterval(interval);
  }, [fetchProfiles, fetchDaemon]);

  return (
    <div className="app-shell">
      {/* Tab bar */}
      <div className="tab-bar">
        <div className="tab-bar-title">🛡 SENTINEL</div>
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`tab${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
        <div style={{ flex: 1 }} />
        {loading && (
          <div
            style={{
              padding: "0 12px",
              color: "#888",
              fontSize: 11,
              alignSelf: "center",
            }}
          >
            Loading…
          </div>
        )}
      </div>

      {/* Content */}
      <div className="tab-content">
        {tab === "profiles" && <ProfileManager />}
        {tab === "sessions" && <SessionGrid />}
        {tab === "auto-switch" && <AutoSwitchConfig />}
        {tab === "stats" && <StatsPanel />}
      </div>

      {/* Status bar */}
      <div className="status-bar">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="label">Active:</span>
          <span>
            {active.profile
              ? `${active.profile}:${active.session}`
              : "No profile"}
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span>
            <span
              className={`status-dot${daemonStatus.running ? "" : " off"}`}
            />
            <span className="label">
              Daemon: {daemonStatus.running ? "On" : "Off"}
            </span>
          </span>
          {daemonStatus.active_timers > 0 && (
            <span className="badge">
              {daemonStatus.active_timers} timer
              {daemonStatus.active_timers !== 1 ? "s" : ""}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
