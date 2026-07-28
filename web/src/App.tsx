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
import { Sidebar } from "./components/Sidebar";
import { Editor } from "./components/Editor";
import { RightPanel } from "./components/RightPanel";
import { GraphView } from "./components/GraphView";
import { CommandPalette } from "./components/CommandPalette";
import { VaultHealth } from "./components/VaultHealth";
import { SamongMark } from "./components/SamongMark";

export function App() {
  const [vaults, setVaults] = useState<VaultInfo[]>([]);
  const [vault, setVault] = useState<string>("");
  const [notesByVault, setNotesByVault] = useState<Record<string, NoteInfo[]>>({});
  // Notes are addressed by key (their path in the vault); the title is display
  // only, because several files can share one.
  const [noteKey, setNoteKey] = useState<string>("");
  const [readOnly, setReadOnly] = useState(false);
  const [content, setContent] = useState<string>("");
  const [dirty, setDirty] = useState(false);
  const [links, setLinks] = useState<NoteLinks | null>(null);
  const [view, setView] = useState<"editor" | "graph">(() =>
    new URLSearchParams(location.search).get("view") === "graph" ? "graph" : "editor",
  );
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [healthOpen, setHealthOpen] = useState(false);
  const [status, setStatus] = useState("");
  const [theme, setTheme] = useState(
    () => document.documentElement.dataset.theme ?? "light",
  );

  const title = noteKey ? titleFromKey(noteKey) : "";

  // Refs so WebSocket / debounce callbacks never see stale state.
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const activeRef = useRef({ vault, noteKey, readOnly });
  activeRef.current = { vault, noteKey, readOnly };
  const contentRef = useRef(content);
  contentRef.current = content;

  const flash = useCallback((message: string) => {
    setStatus(message);
    window.setTimeout(() => setStatus(""), 2500);
  }, []);

  const notes = useMemo(() => notesByVault[vault] ?? [], [notesByVault, vault]);

  const refreshNotes = useCallback(async (vaultName: string) => {
    const list = await api.notes(vaultName);
    setNotesByVault((prev) => ({ ...prev, [vaultName]: list }));
    return list;
  }, []);

  const refreshLinks = useCallback(async (vaultName: string, key: string) => {
    try {
      setLinks(await api.links(vaultName, key));
    } catch {
      setLinks(null);
    }
  }, []);

  /** Persist the current buffer now (used by ctrl+S, debounce, and
   *  navigation away from a dirty note). */
  const saveNow = useCallback(
    async (vaultName: string, key: string, body: string) => {
      if (!vaultName || !key) return;
      // Reference notes belong to a dependency: the server refuses the write,
      // so don't pretend we tried.
      if (activeRef.current.noteKey === key && activeRef.current.readOnly) return;
      try {
        const result = await api.save(vaultName, key, body);
        setDirty(false);
        flash(
          result.indexed
            ? "บันทึกแล้ว ✓"
            : "บันทึกแล้ว — แต่ไฟล์นี้อยู่นอก scope จึงค้นหาไม่เจอ",
        );
        void refreshNotes(vaultName);
        void refreshLinks(vaultName, key);
      } catch (err) {
        flash(`บันทึกไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, refreshLinks, refreshNotes],
  );

  const saveTimer = useRef<number>(0);
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
    async (vaultName: string, key: string) => {
      window.clearTimeout(saveTimer.current);
      const previous = activeRef.current;
      if (dirtyRef.current && previous.noteKey) {
        await saveNow(previous.vault, previous.noteKey, contentRef.current);
      }
      try {
        const note = await api.note(vaultName, key);
        setVault(vaultName);
        setNoteKey(note.key);
        setReadOnly(note.reference);
        setContent(note.content);
        setDirty(false);
        setView("editor");
        void refreshLinks(vaultName, note.key);
      } catch (err) {
        flash(`เปิดโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, refreshLinks, saveNow],
  );

  const createNote = useCallback(
    async (vaultName: string, noteTitle: string) => {
      const key = keyForTitle(noteTitle);
      try {
        await api.save(vaultName, key, `# ${noteTitle}\n\n`);
        await refreshNotes(vaultName);
        await openNote(vaultName, key);
        flash(`สร้าง "${noteTitle}" แล้ว`);
      } catch (err) {
        flash(`สร้างโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, openNote, refreshNotes],
  );

  const deleteNote = useCallback(async () => {
    const { vault: v, noteKey: k } = activeRef.current;
    if (!k || !window.confirm(`ลบโน้ต "${k}" ?`)) return;
    try {
      const result = await api.remove(v, k);
      const remaining = await refreshNotes(v);
      const danglingNote =
        result.dangling_backlinks.length > 0
          ? ` (มี ${result.dangling_backlinks.length} โน้ตยังลิงก์มา)`
          : "";
      flash(`ลบ "${k}" แล้ว${danglingNote}`);
      setNoteKey("");
      setContent("");
      setLinks(null);
      if (remaining.length > 0) void openNote(v, remaining[0].key);
    } catch (err) {
      flash(`ลบไม่สำเร็จ: ${(err as Error).message}`);
    }
  }, [flash, openNote, refreshNotes]);

  /** Follow a [[wikilink]]. Targets name a *title*, so resolve it to a note
   *  path: cross-vault when prefixed with a registered vault name, otherwise
   *  inside the current vault — creating the note when it doesn't exist yet
   *  (Obsidian behavior). An ambiguous title opens the first match, which is
   *  what `samong doctor` warns about. */
  const followWikilink = useCallback(
    (target: string) => {
      const resolve = (vaultName: string, wanted: string) =>
        (notesByVault[vaultName] ?? []).find(
          (n) => n.title === wanted || n.key === wanted,
        );

      const slash = target.indexOf("/");
      if (slash > 0) {
        const prefix = target.slice(0, slash);
        if (vaults.some((v) => v.name === prefix)) {
          const rest = target.slice(slash + 1);
          const found = resolve(prefix, rest);
          void openNote(prefix, found ? found.key : keyForTitle(rest));
          return;
        }
      }
      const { vault: v } = activeRef.current;
      const found = resolve(v, target);
      if (found) void openNote(v, found.key);
      else void createNote(v, target);
    },
    [createNote, notesByVault, openNote, vaults],
  );

  const switchVault = useCallback(
    async (vaultName: string) => {
      setVault(vaultName);
      const list = notesByVault[vaultName] ?? (await refreshNotes(vaultName));
      if (list.length > 0) void openNote(vaultName, list[0].key);
      else {
        setNoteKey("");
        setContent("");
        setLinks(null);
      }
    },
    [notesByVault, openNote, refreshNotes],
  );

  const addVault = useCallback(
    async (name: string, path: string) => {
      try {
        const added = await api.addVault(name, path);
        setVaults((prev) => [...prev, added].sort((a, b) => a.name.localeCompare(b.name)));
        await switchVault(added.name);
        flash(`เพิ่ม vault "${added.name}" แล้ว`);
      } catch (err) {
        flash(`เพิ่ม vault ไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, switchVault],
  );

  const toggleTheme = useCallback(() => {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    localStorage.setItem("samong-theme", next);
    setTheme(next);
  }, [theme]);

  // ---- boot: vaults + notes of every vault (for palette + autocomplete) ----
  useEffect(() => {
    void (async () => {
      try {
        const list = await api.vaults();
        setVaults(list);
        for (const v of list) void refreshNotes(v.name);
        if (list.length > 0) {
          const first = list[0].name;
          setVault(first);
          const notes = await api.notes(first);
          setNotesByVault((prev) => ({ ...prev, [first]: notes }));
          if (notes.length > 0) {
            const note = await api.note(first, notes[0].key);
            setNoteKey(note.key);
            setReadOnly(note.reference);
            setContent(note.content);
            void refreshLinks(first, note.key);
          }
        }
      } catch (err) {
        flash(`เชื่อมต่อ server ไม่ได้: ${(err as Error).message}`);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- real-time: refresh on server change events ----
  useEffect(() => {
    return listenChanges((event) => {
      void refreshNotes(event.vault);
      const { vault: v, noteKey: k } = activeRef.current;
      if (event.vault === v && k) {
        void refreshLinks(v, k);
        if (!dirtyRef.current) {
          void api.note(v, k).then(
            (note) => setContent(note.content),
            () => undefined, // the active note may have been deleted
          );
        }
      }
    });
  }, [refreshLinks, refreshNotes]);

  // ---- global shortcuts ----
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        const { vault: v, noteKey: k } = activeRef.current;
        window.clearTimeout(saveTimer.current);
        void saveNow(v, k, contentRef.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveNow]);

  /** Every note across every vault, for the palette and `[[` autocomplete. */
  const allNotes = useMemo(() => {
    const out: { vault: string; key: string; title: string }[] = [];
    for (const v of vaults) {
      for (const n of notesByVault[v.name] ?? []) {
        out.push({ vault: v.name, key: n.key, title: n.title });
      }
    }
    return out;
  }, [vaults, notesByVault]);

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">
          <SamongMark size={22} />
          Samong
        </span>
        <span className="crumb">
          {vault && (
            <>
              {vault} / <b>{title || "—"}</b>
              {readOnly && " 🔒"}
              {dirty && " •"}
            </>
          )}
        </span>
        <span className="spacer" />
        <span className="status-flash" role="status">
          {status}
        </span>
        <button className="btn" onClick={() => setPaletteOpen(true)}>
          ค้นหา… <kbd>Ctrl K</kbd>
        </button>
        <button
          className={`btn ${view === "graph" ? "active" : ""}`}
          onClick={() => setView(view === "graph" ? "editor" : "graph")}
        >
          กราฟ
        </button>
        <button className="btn" onClick={() => setHealthOpen(true)} disabled={!vault}>
          สภาพ vault
        </button>
        <button className="btn" onClick={toggleTheme} aria-label="สลับธีม">
          {theme === "dark" ? "☀️" : "🌙"}
        </button>
      </header>

      <div className="workspace">
        <Sidebar
          vaults={vaults}
          vault={vault}
          notes={notes}
          active={noteKey}
          onSwitchVault={(v) => void switchVault(v)}
          onOpen={(key) => void openNote(vault, key)}
          onCreate={(t) => void createNote(vault, t)}
          onAddVault={(name, path) => void addVault(name, path)}
        />

        {view === "graph" ? (
          <GraphView
            vault={vault}
            vaults={vaults.map((v) => v.name)}
            onOpen={(v, key) => void openNote(v, key)}
          />
        ) : noteKey ? (
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
        ) : (
          <div className="welcome">
            <SamongMark size={56} />
            <p>
              เลือกโน้ตจากด้านซ้าย หรือกด <kbd>Ctrl K</kbd> เพื่อค้นหา/สร้างโน้ตใหม่
            </p>
          </div>
        )}

        {view === "editor" && (
          <RightPanel
            links={links}
            content={content}
            onOpenKey={(key) => void openNote(vault, key)}
            onOpenTarget={followWikilink}
          />
        )}
      </div>

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

      {paletteOpen && (
        <CommandPalette
          allNotes={allNotes}
          onClose={() => setPaletteOpen(false)}
          onOpen={(v, key) => {
            setPaletteOpen(false);
            void openNote(v, key);
          }}
          onCreate={(t) => {
            setPaletteOpen(false);
            void createNote(vault, t);
          }}
        />
      )}
    </div>
  );
}
