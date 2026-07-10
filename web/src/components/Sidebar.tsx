import type { VaultInfo } from "../api";

interface Props {
  vaults: VaultInfo[];
  vault: string;
  notes: string[];
  active: string;
  onSwitchVault: (vault: string) => void;
  onOpen: (title: string) => void;
  onCreate: (title: string) => void;
}

export function Sidebar({
  vaults,
  vault,
  notes,
  active,
  onSwitchVault,
  onOpen,
  onCreate,
}: Props) {
  const create = () => {
    const title = window.prompt("ชื่อโน้ตใหม่");
    if (title?.trim()) onCreate(title.trim());
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <select
          className="vault-select"
          value={vault}
          onChange={(e) => onSwitchVault(e.target.value)}
          aria-label="เลือก vault"
        >
          {vaults.map((v) => (
            <option key={v.name} value={v.name}>
              🗄 {v.name}
            </option>
          ))}
        </select>
        <button className="btn primary" onClick={create}>
          + โน้ตใหม่
        </button>
      </div>
      <div className="side-label">
        โน้ต ({notes.length})
      </div>
      <ul className="note-list">
        {notes.map((t) => (
          <li key={t}>
            <button className={t === active ? "active" : ""} onClick={() => onOpen(t)}>
              {t}
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
