import { useProfileStore } from "../store/profiles";

export function SessionGrid() {
  const { profiles, active, switchTo } = useProfileStore();

  return (
    <div className="pane">
      {profiles.map((p) => (
        <div key={p.name} style={{ marginBottom: 20 }}>
          <div
            className="card-title"
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              marginBottom: 8,
            }}
          >
            {p.name}
            <span className="badge">{p.auth_type}</span>
            {p.is_active && (
              <span
                style={{
                  background: "var(--black)",
                  color: "var(--white)",
                  padding: "2px 6px",
                  fontSize: 10,
                }}
              >
                ACTIVE
              </span>
            )}
          </div>

          <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
            {p.sessions.map((s) => {
              const isActive = active.profile === p.name && active.session === s;
              return (
                <div
                  key={s}
                  className={`card${isActive ? " active" : ""}`}
                  style={{
                    minWidth: 160,
                    cursor: "pointer",
                    userSelect: "none",
                  }}
                  onClick={() => switchTo(p.name, s)}
                >
                  <div className="card-title" style={{ fontSize: 12 }}>
                    {isActive && "▶ "}
                    {s}
                  </div>
                  <div
                    className="label"
                    style={{ fontSize: 10, marginTop: 4 }}
                  >
                    {p.name}:{s}
                  </div>
                  {isActive && (
                    <div
                      className="label"
                      style={{
                        fontSize: 10,
                        marginTop: 6,
                        padding: "2px 0",
                        color: "var(--white)",
                      }}
                    >
                      CURRENT SESSION
                    </div>
                  )}
                </div>
              );
            })}

            {p.sessions.length === 0 && (
              <div
                style={{
                  border: "2px dashed #999",
                  padding: "12px 20px",
                  color: "#999",
                  fontSize: 12,
                }}
              >
                No sessions yet
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
