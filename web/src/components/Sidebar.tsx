import type { NoteInfo, VaultInfo } from "../api";
import { NoteTree } from "./NoteTree";

interface Props {
  vaults: VaultInfo[];
  vault: string;
  notes: NoteInfo[];
  /** Key of the open note. */
  active: string;
  onSwitchVault: (vault: string) => void;
  onOpen: (key: string) => void;
  onCreate: (title: string) => void;
  onAddVault: (name: string, path: string) => void;
}

export function Sidebar({
  vaults,
  vault,
  notes,
  active,
  onSwitchVault,
  onOpen,
  onCreate,
  onAddVault,
}: Props) {
  const create = () => {
    const title = window.prompt("ชื่อโน้ตใหม่");
    if (title?.trim()) onCreate(title.trim());
  };

  const addVault = () => {
    const path = window.prompt(
      "โฟลเดอร์ของ vault (พาธเต็ม)\nชี้ที่ root ของโปรเจกต์ได้เลย — Samong ข้าม node_modules และไฟล์ที่ gitignore ให้เอง",
    );
    if (!path?.trim()) return;
    const suggested = path.trim().replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? "";
    const name = window.prompt("ชื่อ vault (ใช้ใน [[ชื่อ/โน้ต]])", suggested);
    if (name?.trim()) onAddVault(name.trim(), path.trim());
  };

  // A first-time user has no vault yet, and until Phase 14 the only way to add
  // one was a CLI command they had not read about — so offer it right here.
  if (vaults.length === 0) {
    return (
      <aside className="sidebar">
        <div className="sidebar-head">
          <button className="btn primary" onClick={addVault}>
            + เพิ่ม vault
          </button>
        </div>
        <p className="empty-hint">
          ยังไม่มี vault — เพิ่มโฟลเดอร์โน้ตของคุณเพื่อเริ่มใช้งาน
        </p>
      </aside>
    );
  }

  return (
    <aside className="sidebar">
      <div className="sidebar-head">
        <select
          className="vault-select"
          value={vault}
          onChange={(e) =>
            e.target.value === "__add" ? addVault() : onSwitchVault(e.target.value)
          }
          aria-label="เลือก vault"
        >
          {vaults.map((v) => (
            <option key={v.name} value={v.name}>
              🗄 {v.name}
            </option>
          ))}
          <option value="__add">+ เพิ่ม vault…</option>
        </select>
        <button className="btn primary" onClick={create}>
          + โน้ตใหม่
        </button>
      </div>
      <NoteTree notes={notes} active={active} onOpen={onOpen} />
    </aside>
  );
}
