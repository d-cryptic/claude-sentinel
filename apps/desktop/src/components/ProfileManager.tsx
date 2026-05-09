import { useState } from "react";
import { useProfileStore } from "../store/profiles";

export function ProfileManager() {
  const { profiles, active, switchTo, createProfile, deleteProfile, fetch, error } =
    useProfileStore();
  const [selected, setSelected] = useState<string | null>(null);
  const [showNew, setShowNew] = useState(false);
  // Stores the name of the profile pending deletion. Cleared when selection changes.
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newAuth, setNewAuth] = useState("oauth");

  const selectedProfile = profiles.find((p) => p.name === selected) ?? profiles[0];

  const handleCreate = async () => {
    if (!newName.trim()) return;
    await createProfile(newName.trim(), newAuth);
    // Only close the modal if creation succeeded (no error in store)
    if (!useProfileStore.getState().error) {
      setShowNew(false);
      setNewName("");
    }
  };

  return (
    <div className="split" style={{ height: "100%" }}>
      {error && (
        <div
          role="alert"
          style={{
            gridColumn: "1 / -1",
            background: "#fff0f0",
            border: "2px solid #c00",
            color: "#c00",
            padding: "6px 12px",
            fontSize: 12,
            marginBottom: 8,
          }}
        >
          {error}
        </div>
      )}
      {/* Left: profile list */}
      <div className="split-left">
        <div
          style={{
            padding: "10px 12px",
            borderBottom: "var(--border-thick)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <span className="label">Profiles</span>
          <button className="btn btn-sm" onClick={() => setShowNew(true)}>
            + New
          </button>
        </div>

        {profiles.map((p) => (
          <button
            key={p.name}
            role="option"
            aria-selected={selected === p.name || (!selected && p.is_active)}
            className={`list-item${selected === p.name || (!selected && p.is_active) ? " selected" : ""}`}
            style={{ width: "100%", textAlign: "left", background: "none", border: "none", padding: 0, font: "inherit", cursor: "pointer" }}
            onClick={() => { setSelected(p.name); setConfirmDelete(null); }}
          >
            {p.is_active && <span>▶</span>}
            <span style={{ flex: 1 }}>{p.name}</span>
            <span className="badge">{p.auth_type}</span>
          </button>
        ))}
      </div>

      {/* Right: detail */}
      <div className="split-right">
        {selectedProfile ? (
          <>
            <div style={{ marginBottom: 16 }}>
              <h2>{selectedProfile.name}</h2>
              <div className="label" style={{ marginTop: 4 }}>
                Auth: {selectedProfile.auth_type}
                {selectedProfile.is_active && (
                  <span
                    style={{
                      marginLeft: 12,
                      background: "var(--black)",
                      color: "var(--white)",
                      padding: "2px 8px",
                    }}
                  >
                    ACTIVE
                  </span>
                )}
              </div>
            </div>

            <hr className="divider" />

            <div className="label" style={{ marginBottom: 8 }}>
              Sessions
            </div>
            {selectedProfile.sessions.map((s) => (
              <div
                key={s}
                className="list-item"
                style={{ marginBottom: 2 }}
                onClick={() => switchTo(selectedProfile.name, s)}
              >
                {active.profile === selectedProfile.name && active.session === s && (
                  <span>▶</span>
                )}
                <span style={{ flex: 1 }}>{s}</span>
                <button
                  className="btn btn-sm btn-primary"
                  onClick={(e) => {
                    e.stopPropagation();
                    switchTo(selectedProfile.name, s);
                  }}
                >
                  Use
                </button>
              </div>
            ))}

            <hr className="divider" />

            {confirmDelete === selectedProfile.name ? (
              <div
                role="alertdialog"
                aria-label={`Confirm deletion of profile "${selectedProfile.name}"`}
                style={{ display: "flex", gap: 8, alignItems: "center" }}
              >
                <span style={{ fontSize: 12 }}>Delete "{selectedProfile.name}"?</span>
                <button
                  className="btn btn-danger btn-sm"
                  onClick={async () => {
                    await deleteProfile(selectedProfile.name);
                    // Only clear selection if deletion succeeded
                    if (!useProfileStore.getState().error) {
                      setSelected(null);
                      setConfirmDelete(null);
                    }
                  }}
                >
                  Confirm
                </button>
                <button
                  className="btn btn-sm"
                  autoFocus
                  onClick={() => setConfirmDelete(null)}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                className="btn btn-danger"
                onClick={() => setConfirmDelete(selectedProfile.name)}
              >
                Delete Profile
              </button>
            )}
          </>
        ) : (
          <div style={{ color: "#999", padding: 16 }}>
            Select a profile to see details.
          </div>
        )}
      </div>

      {/* New profile modal */}
      {showNew && (
        <div
          className="modal-overlay"
          onClick={() => setShowNew(false)}
          onKeyDown={(e) => e.key === "Escape" && setShowNew(false)}
          role="presentation"
        >
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="new-profile-title"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-title" id="new-profile-title">New Profile</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              <div>
                <div className="label" style={{ marginBottom: 4 }}>
                  Name
                </div>
                <input
                  className="input"
                  placeholder="e.g. work"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleCreate()}
                  autoFocus
                />
              </div>
              <div>
                <div className="label" style={{ marginBottom: 4 }}>
                  Auth Type
                </div>
                <select
                  className="select"
                  value={newAuth}
                  onChange={(e) => setNewAuth(e.target.value)}
                >
                  <option value="oauth">OAuth (Pro/Max)</option>
                  <option value="api">API Key</option>
                  <option value="bedrock">AWS Bedrock</option>
                  <option value="vertex">Google Vertex AI</option>
                </select>
              </div>
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setShowNew(false)}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={handleCreate}>
                Create
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
