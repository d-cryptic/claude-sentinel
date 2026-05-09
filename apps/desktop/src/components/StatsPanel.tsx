import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface StatsDto {
  profile: string;
  session: string;
  session_count: number;
  rate_limit_hits: number;
  key_rotations: number;
  tokens_in: number;
  tokens_out: number;
  estimated_cost_usd: number;
  first_used: string | null;
  last_used: string | null;
}

function fmtNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** Simple ASCII bar chart column */
function AsciiBar({ value, max, width = 20 }: { value: number; max: number; width?: number }) {
  const filled = max > 0 ? Math.round((value / max) * width) : 0;
  return (
    <span style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
      {"█".repeat(filled)}{"░".repeat(width - filled)}
    </span>
  );
}

export function StatsPanel() {
  const [stats, setStats] = useState<StatsDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<StatsDto[]>("get_stats", { profile: null, session: null })
      .then((data) => { setStats(data); setError(null); })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return <div className="pane" style={{ color: "#999" }}>Loading stats...</div>;
  }

  if (error) {
    return (
      <div className="pane" style={{ color: "#c00", textAlign: "center", paddingTop: 40 }}>
        Failed to load stats: {error}
      </div>
    );
  }

  if (stats.length === 0) {
    return (
      <div className="pane" style={{ color: "#999", textAlign: "center", paddingTop: 40 }}>
        No stats recorded yet. Use claude via a profile to start tracking.
      </div>
    );
  }

  const maxTokensIn = Math.max(...stats.map((s) => s.tokens_in), 1);
  const maxTokensOut = Math.max(...stats.map((s) => s.tokens_out), 1);
  const totalCost = stats.reduce((acc, s) => acc + s.estimated_cost_usd, 0);
  const totalTokensIn = stats.reduce((acc, s) => acc + s.tokens_in, 0);
  const totalTokensOut = stats.reduce((acc, s) => acc + s.tokens_out, 0);

  return (
    <div className="pane">
      {/* Summary row */}
      <div style={{ display: "flex", gap: 12, marginBottom: 20 }}>
        {[
          { label: "Total Cost", value: `$${totalCost.toFixed(4)}` },
          { label: "Tokens In", value: fmtNum(totalTokensIn) },
          { label: "Tokens Out", value: fmtNum(totalTokensOut) },
          { label: "Profiles", value: String(new Set(stats.map((s) => s.profile)).size) },
        ].map((kv) => (
          <div key={kv.label} className="card" style={{ flex: 1, textAlign: "center" }}>
            <div className="label" style={{ fontSize: 10, marginBottom: 4 }}>{kv.label}</div>
            <div style={{ fontSize: 16, fontWeight: 900 }}>{kv.value}</div>
          </div>
        ))}
      </div>

      {/* Token usage chart */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-title">Token Usage</div>
        <table style={{ width: "100%" }}>
          <thead>
            <tr>
              <th>Profile:Session</th>
              <th>Tokens In</th>
              <th style={{ width: "30%" }}>Chart</th>
              <th>Tokens Out</th>
              <th>Cost (USD)</th>
            </tr>
          </thead>
          <tbody>
            {stats.map((s) => (
              <tr key={`${s.profile}:${s.session}`}>
                <td>{s.profile}:{s.session}</td>
                <td>{fmtNum(s.tokens_in)}</td>
                <td>
                  <AsciiBar value={s.tokens_in} max={maxTokensIn} />
                </td>
                <td>{fmtNum(s.tokens_out)}</td>
                <td>${s.estimated_cost_usd.toFixed(4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Rate limits */}
      <div className="card">
        <div className="card-title">Rate Limits & Rotations</div>
        <table>
          <thead>
            <tr>
              <th>Profile:Session</th>
              <th>Sessions</th>
              <th>Rate Limits</th>
              <th>Key Rotations</th>
              <th>Last Used</th>
            </tr>
          </thead>
          <tbody>
            {stats.map((s) => (
              <tr key={`${s.profile}:${s.session}`}>
                <td>{s.profile}:{s.session}</td>
                <td>{s.session_count}</td>
                <td>{s.rate_limit_hits}</td>
                <td>{s.key_rotations}</td>
                <td style={{ fontSize: 11 }}>
                  {s.last_used ? new Date(s.last_used).toLocaleString() : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
