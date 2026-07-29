import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  keyForTitle,
  titleFromKey,
  type NoteInfo,
  type NoteLinks,
  type VaultInfo,
} from "./api";
import { listenChanges } from "./ws";
import { GraphCanvas } from "./components/GraphCanvas";
import { SearchPanel } from "./components/SearchPanel";
import { DetailPanel } from "./components/DetailPanel";
import { NoteTree } from "./components/NoteTree";
import { Editor } from "./components/Editor";
import { VaultHealth } from "./components/VaultHealth";
import { SamongMark } from "./components/SamongMark";

/**
 * The graph is the home surface: a vault is a shape, not a folder listing, and
 * searching is how you enter it. Reading a note is a focused state on top of
 * that map rather than a different application.
 */
export function App() {
  const [vaults, setVaults] = useState<VaultInfo[]>([]);
  const [vault, setVault] = useState("");
  const [notesByVault, setNotesByVault] = useState<Record<string, NoteInfo[]>>({});
  const [noteKey, setNoteKey] = useState("");
  const [readOnly, setReadOnly] = useState(false);
  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);
  const [links, setLinks] = useState<NoteLinks | null>(null);
  const [matched, setMatched] = useState<Set<string> | null>(null);
  const [allVaults, setAllVaults] = useState(false);
  const [reading, setReading] = useState(false);
  const [treeOpen, setTreeOpen] = useState(true);
  const [healthOpen, setHealthOpen] = useState(false);
  const [status, setStatus] = useState("");
  const [revision, setRevision] = useState(0);
  const [theme, setTheme] = useState(() => document.documentElement.dataset.theme ?? "dark");

  const title = noteKey ? titleFromKey(noteKey) : "";
  const notes = useMemo(() => notesByVault[vault] ?? [], [notesByVault, vault]);

  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const activeRef = useRef({ vault, noteKey, readOnly });
  activeRef.current = { vault, noteKey, readOnly };
  const contentRef = useRef(content);
  contentRef.current = content;
  const saveTimer = useRef(0);

  const flash = useCallback((message: string) => {
    setStatus(message);
    window.setTimeout(() => setStatus(""), 2600);
  }, []);

  const refreshNotes = useCallback(async (name: string) => {
    const list = await api.notes(name);
    setNotesByVault((prev) => ({ ...prev, [name]: list }));
    return list;
  }, []);

  const refreshLinks = useCallback(async (name: string, key: string) => {
    try {
      setLinks(await api.links(name, key));
    } catch {
      setLinks(null);
    }
  }, []);

  const saveNow = useCallback(
    async (name: string, key: string, body: string) => {
      if (!name || !key) return;
      if (activeRef.current.noteKey === key && activeRef.current.readOnly) return;
      try {
        const result = await api.save(name, key, body);
        setDirty(false);
        flash(result.indexed ? "บันทึกแล้ว" : "บันทึกแล้ว — ไฟล์นี้อยู่นอก scope จึงค้นไม่เจอ");
        void refreshNotes(name);
        void refreshLinks(name, key);
        setRevision((r) => r + 1);
      } catch (err) {
        flash(`บันทึกไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, refreshLinks, refreshNotes],
  );

  const scheduleSave = useCallback(
    (body: string) => {
      setContent(body);
      setDirty(true);
      window.clearTimeout(saveTimer.current);
      const { vault: v, noteKey: k } = activeRef.current;
      saveTimer.current = window.setTimeout(() => void saveNow(v, k, body), 900);
    },
    [saveNow],
  );

  const openNote = useCallback(
    async (name: string, key: string) => {
      window.clearTimeout(saveTimer.current);
      const previous = activeRef.current;
      if (dirtyRef.current && previous.noteKey) {
        await saveNow(previous.vault, previous.noteKey, contentRef.current);
      }
      try {
        const note = await api.note(name, key);
        setVault(name);
        setNoteKey(note.key);
        setReadOnly(note.reference);
        setContent(note.content);
        setDirty(false);
        void refreshLinks(name, note.key);
      } catch (err) {
        flash(`เปิดโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, refreshLinks, saveNow],
  );

  const createNote = useCallback(
    async (name: string, noteTitle: string) => {
      const key = keyForTitle(noteTitle);
      try {
        await api.save(name, key, `# ${noteTitle}\n\n`);
        await refreshNotes(name);
        await openNote(name, key);
        setReading(true);
        setRevision((r) => r + 1);
        flash(`สร้าง “${noteTitle}” แล้ว`);
      } catch (err) {
        flash(`สร้างโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, openNote, refreshNotes],
  );

  const deleteNote = useCallback(async () => {
    const { vault: v, noteKey: k } = activeRef.current;
    if (!k || !window.confirm(`ลบโน้ต “${k}” ?`)) return;
    try {
      const result = await api.remove(v, k);
      await refreshNotes(v);
      const dangling = result.dangling_backlinks.length;
      flash(`ลบ “${k}” แล้ว${dangling ? ` (ยังมี ${dangling} โน้ตลิงก์มา)` : ""}`);
      setNoteKey("");
      setContent("");
      setLinks(null);
      setReading(false);
      setRevision((r) => r + 1);
    } catch (err) {
      flash(`ลบไม่สำเร็จ: ${(err as Error).message}`);
    }
  }, [flash, refreshNotes]);

  /** A [[target]] names a title; resolve it to a path, creating the note if it
   *  does not exist yet (Obsidian behaviour). */
  const followWikilink = useCallback(
    (target: string) => {
      const find = (name: string, wanted: string) =>
        (notesByVault[name] ?? []).find((n) => n.title === wanted || n.key === wanted);
      const slash = target.indexOf("/");
      if (slash > 0) {
        const prefix = target.slice(0, slash);
        if (vaults.some((v) => v.name === prefix)) {
          const rest = target.slice(slash + 1);
          const found = find(prefix, rest);
          void openNote(prefix, found ? found.key : keyForTitle(rest));
          return;
        }
      }
      const found = find(activeRef.current.vault, target);
      if (found) void openNote(activeRef.current.vault, found.key);
      else void createNote(activeRef.current.vault, target);
    },
    [createNote, notesByVault, openNote, vaults],
  );

  const switchVault = useCallback(
    async (name: string) => {
      setVault(name);
      setNoteKey("");
      setContent("");
      setLinks(null);
      setReading(false);
      await refreshNotes(name);
      setRevision((r) => r + 1);
    },
    [refreshNotes],
  );

  const addVault = useCallback(async () => {
    const path = window.prompt(
      "โฟลเดอร์ของ vault (พาธเต็ม)\nชี้ที่ root ของโปรเจกต์ได้เลย — Samong ข้าม node_modules และไฟล์ที่ gitignore ให้เอง",
    );
    if (!path?.trim()) return;
    const suggested = path.trim().replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? "";
    const name = window.prompt("ชื่อ vault (ใช้ใน [[ชื่อ/โน้ต]])", suggested);
    if (!name?.trim()) return;
    try {
      const added = await api.addVault(name.trim(), path.trim());
      setVaults((prev) => [...prev, added].sort((a, b) => a.name.localeCompare(b.name)));
      await switchVault(added.name);
      flash(`เพิ่ม vault “${added.name}” แล้ว`);
    } catch (err) {
      flash(`เพิ่ม vault ไม่สำเร็จ: ${(err as Error).message}`);
    }
  }, [flash, switchVault]);

  const toggleTheme = useCallback(() => {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    localStorage.setItem("samong-theme", next);
    setTheme(next);
  }, [theme]);

  // ---- boot ----
  useEffect(() => {
    void (async () => {
      try {
        const list = await api.vaults();
        setVaults(list);
        for (const v of list) void refreshNotes(v.name);
        if (list.length > 0) setVault(list[0].name);
      } catch (err) {
        flash(`เชื่อมต่อ server ไม่ได้: ${(err as Error).message}`);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(
    () =>
      listenChanges((event) => {
        void refreshNotes(event.vault);
        setRevision((r) => r + 1);
        const { vault: v, noteKey: k } = activeRef.current;
        if (event.vault === v && k) {
          void refreshLinks(v, k);
          if (!dirtyRef.current) {
            void api.note(v, k).then(
              (note) => setContent(note.content),
              () => undefined,
            );
          }
        }
      }),
    [refreshLinks, refreshNotes],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        const { vault: v, noteKey: k } = activeRef.current;
        window.clearTimeout(saveTimer.current);
        void saveNow(v, k, contentRef.current);
      }
      if (e.key === "Escape" && reading) setReading(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveNow, reading]);

  const vaultNames = useMemo(() => vaults.map((v) => v.name), [vaults]);
  const allNotes = useMemo(() => {
    const out: { vault: string; key: string; title: string }[] = [];
    for (const v of vaults) {
      for (const n of notesByVault[v.name] ?? []) {
        out.push({ vault: v.name, key: n.key, title: n.title });
      }
    }
    return out;
  }, [vaults, notesByVault]);

  if (vaults.length === 0) {
    return (
      <div className="app onboard">
        <div className="onboard-card">
          <SamongMark size={44} />
          <h1>Samong</h1>
          <p>
            ชี้ไปที่โฟลเดอร์โน้ตหรือ root ของโปรเจกต์ แล้ว Samong จะทำแผนที่ความรู้ให้
            โดยข้าม <code>node_modules</code> และไฟล์ที่ <code>.gitignore</code> กันไว้เอง
          </p>
          <button className="btn primary lg" onClick={() => void addVault()}>
            เพิ่ม vault แรก
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={`app ${reading ? "is-reading" : ""}`}>
      <header className="frame-top">
        <span className="brand">
          <SamongMark size={20} />
          Samong
        </span>

        <div className="vault-switch">
          <select
            value={vault}
            onChange={(e) =>
              e.target.value === "__add" ? void addVault() : void switchVault(e.target.value)
            }
            aria-label="เลือก vault"
          >
            {vaults.map((v) => (
              <option key={v.name} value={v.name}>
                {v.name}
              </option>
            ))}
            <option value="__add">+ เพิ่ม vault…</option>
          </select>
        </div>

        <SearchPanel
          vault={vault}
          allVaults={allVaults}
          onMatches={setMatched}
          onOpen={(v, k) => void openNote(v, k)}
          onCreate={(t) => void createNote(vault, t)}
        />

        <span className="status-flash" role="status">
          {status}
        </span>

        {vaults.length > 1 && (
          <button
            className={`btn ${allVaults ? "on" : ""}`}
            onClick={() => setAllVaults(!allVaults)}
            title="รวมทุก vault ในแผนที่เดียว"
          >
            ทุก vault
          </button>
        )}
        <button className="btn" onClick={() => setHealthOpen(true)}>
          สภาพ vault
        </button>
        <button className="btn icon" onClick={toggleTheme} aria-label="สลับธีม">
          {theme === "dark" ? "☀" : "☾"}
        </button>
      </header>

      <div className="frame-body">
        <aside className={`rail ${treeOpen ? "open" : ""}`}>
          <button
            className="rail-toggle"
            onClick={() => setTreeOpen(!treeOpen)}
            aria-expanded={treeOpen}
          >
            <span aria-hidden>{treeOpen ? "‹" : "›"}</span>
            <span className="rail-toggle-label">รายการโน้ต</span>
          </button>
          {treeOpen && (
            <div className="rail-body">
              <NoteTree notes={notes} active={noteKey} onOpen={(k) => void openNote(vault, k)} />
            </div>
          )}
        </aside>

        <main className="stage">
          <GraphCanvas
            vault={vault}
            vaults={vaultNames}
            allVaults={allVaults}
            matched={matched}
            selectedKey={noteKey}
            onSelect={(v, k) => void openNote(v, k)}
            revision={revision}
          />
          {matched && (
            <div className="stage-note">
              เน้นเฉพาะ {matched.size} โน้ตที่ตรงกับคำค้น — กด Esc ในช่องค้นเพื่อกลับมาดูทั้งหมด
            </div>
          )}
        </main>

        <DetailPanel
          vault={vault}
          noteKey={noteKey}
          title={title}
          content={content}
          readOnly={readOnly}
          links={links}
          onOpenKey={(k) => void openNote(vault, k)}
          onFollow={followWikilink}
          onExpand={() => setReading(true)}
          onDelete={() => void deleteNote()}
        />
      </div>

      {reading && noteKey && (
        <div className="reader">
          <div className="reader-bar">
            <span className="reader-title">{title}</span>
            <span className="path">{noteKey}</span>
            {dirty && <span className="reader-dirty">ยังไม่บันทึก</span>}
            <span className="spacer" />
            <button className="btn" onClick={() => setReading(false)}>
              กลับไปที่แผนที่ <kbd>Esc</kbd>
            </button>
          </div>
          <Editor
            title={title}
            noteKey={noteKey}
            content={content}
            readOnly={readOnly}
            allTitles={allNotes}
            vault={vault}
            onChange={scheduleSave}
            onFollow={followWikilink}
            onDelete={() => void deleteNote()}
          />
        </div>
      )}

      {healthOpen && vault && (
        <VaultHealth
          vault={vault}
          onClose={() => setHealthOpen(false)}
          onOpen={(key) => {
            setHealthOpen(false);
            void openNote(vault, key);
          }}
        />
      )}
    </div>
  );
}
