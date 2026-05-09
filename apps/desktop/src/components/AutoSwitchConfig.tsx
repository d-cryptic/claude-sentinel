import { useEffect } from "react";
import { useDaemonStore } from "../store/daemon";

export function AutoSwitchConfig() {
  const { status, switchLog, schedulerEntries, error, fetch, start, stop } =
    useDaemonStore();

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 10_000);
    return () => clearInterval(interval);
  }, [fetch]);

  const active = schedulerEntries.filter((e) => !e.switched_back);

  return (
    <div className="pane">
      {error && (
        <div
          role="alert"
          style={{ color: "#c00", background: "#fff0f0", padding: "8px 12px", marginBottom: 12, border: "2px solid #c00", fontSize: 12 }}
        >
          Error: {error}
        </div>
      )}
      {/* Daemon status */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">Daemon</div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span
              className={`status-dot${status.running ? "" : " off"}`}
              style={{ background: status.running ? "var(--black)" : "transparent" }}
            />
            <span className="label">
              {status.running ? "Running" : "Stopped"}
            </span>
            {status.running && status.active_timers > 0 && (
              <span className="badge">
                {status.active_timers} timer{status.active_timers !== 1 ? "s" : ""}
              </span>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            {!status.running ? (
              <button className="btn btn-primary btn-sm" onClick={start}>
                Start Daemon
              </button>
            ) : (
              <button className="btn btn-sm" onClick={stop}>
                Stop
              </button>
            )}
            <button className="btn btn-sm" onClick={fetch}>
              Refresh
            </button>
          </div>
        </div>
      </div>

      {/* Active rate-limit timers */}
      {active.length > 0 && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-title">Active Rate-Limit Timers</div>
          <table>
            <thead>
              <tr>
                <th>Profile</th>
                <th>Detected At</th>
                <th>Refills In</th>
                <th>Auto Switch Back</th>
              </tr>
            </thead>
            <tbody>
              {active.map((e) => (
                <tr key={e.profile}>
                  <td>{e.profile}</td>
                  <td>{e.detected_at}</td>
                  <td>{e.time_until_refill}</td>
                  <td>{e.auto_switch_back ? "YES" : "NO"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Switch log */}
      <div className="card">
        <div
          className="card-title"
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          Switch Log
          <span className="label" style={{ fontWeight: 400 }}>
            Last {switchLog.length} events
          </span>
        </div>
        {switchLog.length === 0 ? (
          <div
            style={{
              color: "#999",
              fontSize: 12,
              padding: "8px 0",
              textAlign: "center",
            }}
          >
            No switch events recorded yet.
            <br />
            Start the daemon and use cst to begin tracking.
          </div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Time</th>
                <th>From</th>
                <th>To</th>
                <th>Reason</th>
              </tr>
            </thead>
            <tbody>
              {[...switchLog].reverse().map((ev, i) => (
                <tr key={i}>
                  <td style={{ whiteSpace: "nowrap" }}>{ev.timestamp}</td>
                  <td>
                    {ev.from_profile}:{ev.from_session}
                  </td>
                  <td>
                    {ev.to_profile}:{ev.to_session}
                  </td>
                  <td>
                    {ev.reason}
                    {ev.detail && (
                      <span style={{ color: "#666", marginLeft: 4 }}>
                        — {ev.detail}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
