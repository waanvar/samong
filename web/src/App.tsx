import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type NoteLinks, type VaultInfo } from "./api";
import { listenChanges } from "./ws";
import { Sidebar } from "./components/Sidebar";
import { Editor } from "./components/Editor";
import { RightPanel } from "./components/RightPanel";
import { GraphView } from "./components/GraphView";
import { CommandPalette } from "./components/CommandPalette";
import { BanyanMark } from "./components/BanyanMark";

export function App() {
  const [vaults, setVaults] = useState<VaultInfo[]>([]);
  const [vault, setVault] = useState<string>("");
  const [notesByVault, setNotesByVault] = useState<Record<string, string[]>>({});
  const [title, setTitle] = useState<string>("");
  const [content, setContent] = useState<string>("");
  const [dirty, setDirty] = useState(false);
  const [links, setLinks] = useState<NoteLinks | null>(null);
  const [view, setView] = useState<"editor" | "graph">("editor");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [status, setStatus] = useState("");
  const [theme, setTheme] = useState(
    () => document.documentElement.dataset.theme ?? "light",
  );

  // Refs so WebSocket / debounce callbacks never see stale state.
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const activeRef = useRef({ vault, title });
  activeRef.current = { vault, title };
  const contentRef = useRef(content);
  contentRef.current = content;

  const flash = useCallback((message: string) => {
    setStatus(message);
    window.setTimeout(() => setStatus(""), 2500);
  }, []);

  const notes = useMemo(() => notesByVault[vault] ?? [], [notesByVault, vault]);

  const refreshNotes = useCallback(async (vaultName: string) => {
    const titles = await api.notes(vaultName);
    setNotesByVault((prev) => ({ ...prev, [vaultName]: titles }));
    return titles;
  }, []);

  const refreshLinks = useCallback(async (vaultName: string, noteTitle: string) => {
    try {
      setLinks(await api.links(vaultName, noteTitle));
    } catch {
      setLinks(null);
    }
  }, []);

  /** Persist the current buffer now (used by ctrl+S, debounce, and
   *  navigation away from a dirty note). */
  const saveNow = useCallback(
    async (vaultName: string, noteTitle: string, body: string) => {
      if (!vaultName || !noteTitle) return;
      try {
        await api.save(vaultName, noteTitle, body);
        setDirty(false);
        flash("บันทึกแล้ว ✓");
        void refreshNotes(vaultName);
        void refreshLinks(vaultName, noteTitle);
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
      const { vault: v, title: t } = activeRef.current;
      saveTimer.current = window.setTimeout(() => void saveNow(v, t, body), 900);
    },
    [saveNow],
  );

  const openNote = useCallback(
    async (vaultName: string, noteTitle: string) => {
      window.clearTimeout(saveTimer.current);
      const previous = activeRef.current;
      if (dirtyRef.current && previous.title) {
        await saveNow(previous.vault, previous.title, contentRef.current);
      }
      try {
        const note = await api.note(vaultName, noteTitle);
        setVault(vaultName);
        setTitle(note.title);
        setContent(note.content);
        setDirty(false);
        setView("editor");
        void refreshLinks(vaultName, noteTitle);
      } catch (err) {
        flash(`เปิดโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, refreshLinks, saveNow],
  );

  const createNote = useCallback(
    async (vaultName: string, noteTitle: string) => {
      try {
        await api.save(vaultName, noteTitle, `# ${noteTitle}\n\n`);
        await refreshNotes(vaultName);
        await openNote(vaultName, noteTitle);
        flash(`สร้าง "${noteTitle}" แล้ว`);
      } catch (err) {
        flash(`สร้างโน้ตไม่สำเร็จ: ${(err as Error).message}`);
      }
    },
    [flash, openNote, refreshNotes],
  );

  const deleteNote = useCallback(async () => {
    const { vault: v, title: t } = activeRef.current;
    if (!t || !window.confirm(`ลบโน้ต "${t}" ?`)) return;
    try {
      const result = await api.remove(v, t);
      const remaining = await refreshNotes(v);
      const danglingNote =
        result.dangling_backlinks.length > 0
          ? ` (มี ${result.dangling_backlinks.length} โน้ตยังลิงก์มา)`
          : "";
      flash(`ลบ "${t}" แล้ว${danglingNote}`);
      setTitle("");
      setContent("");
      setLinks(null);
      if (remaining.length > 0) void openNote(v, remaining[0]);
    } catch (err) {
      flash(`ลบไม่สำเร็จ: ${(err as Error).message}`);
    }
  }, [flash, openNote, refreshNotes]);

  /** Follow a [[wikilink]]: cross-vault when prefixed with a registered
   *  vault name, otherwise inside the current vault — creating the note
   *  first when it doesn't exist yet (Obsidian behavior). */
  const followWikilink = useCallback(
    (target: string) => {
      const slash = target.indexOf("/");
      if (slash > 0) {
        const prefix = target.slice(0, slash);
        if (vaults.some((v) => v.name === prefix)) {
          void openNote(prefix, target.slice(slash + 1));
          return;
        }
      }
      const { vault: v } = activeRef.current;
      if ((notesByVault[v] ?? []).includes(target)) {
        void openNote(v, target);
      } else {
        void createNote(v, target);
      }
    },
    [createNote, notesByVault, openNote, vaults],
  );

  const switchVault = useCallback(
    async (vaultName: string) => {
      setVault(vaultName);
      const titles = notesByVault[vaultName] ?? (await refreshNotes(vaultName));
      if (titles.length > 0) void openNote(vaultName, titles[0]);
      else {
        setTitle("");
        setContent("");
        setLinks(null);
      }
    },
    [notesByVault, openNote, refreshNotes],
  );

  const toggleTheme = useCallback(() => {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    localStorage.setItem("banyan-theme", next);
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
          const titles = await api.notes(first);
          setNotesByVault((prev) => ({ ...prev, [first]: titles }));
          if (titles.length > 0) {
            const note = await api.note(first, titles[0]);
            setTitle(note.title);
            setContent(note.content);
            void refreshLinks(first, note.title);
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
      const { vault: v, title: t } = activeRef.current;
      if (event.vault === v && t) {
        void refreshLinks(v, t);
        if (!dirtyRef.current) {
          void api.note(v, t).then(
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
        const { vault: v, title: t } = activeRef.current;
        window.clearTimeout(saveTimer.current);
        void saveNow(v, t, contentRef.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveNow]);

  const allTitles = useMemo(() => {
    const out: { vault: string; title: string }[] = [];
    for (const v of vaults) {
      for (const t of notesByVault[v.name] ?? []) out.push({ vault: v.name, title: t });
    }
    return out;
  }, [vaults, notesByVault]);

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">
          <BanyanMark size={22} />
          Banyan
        </span>
        <span className="crumb">
          {vault && (
            <>
              {vault} / <b>{title || "—"}</b>
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
        <button className="btn" onClick={toggleTheme} aria-label="สลับธีม">
          {theme === "dark" ? "☀️" : "🌙"}
        </button>
      </header>

      <div className="workspace">
        <Sidebar
          vaults={vaults}
          vault={vault}
          notes={notes}
          active={title}
          onSwitchVault={(v) => void switchVault(v)}
          onOpen={(t) => void openNote(vault, t)}
          onCreate={(t) => void createNote(vault, t)}
        />

        {view === "graph" ? (
          <GraphView
            vault={vault}
            vaults={vaults.map((v) => v.name)}
            onOpen={(v, t) => void openNote(v, t)}
          />
        ) : title ? (
          <Editor
            title={title}
            content={content}
            allTitles={allTitles}
            vault={vault}
            onChange={scheduleSave}
            onFollow={followWikilink}
            onDelete={() => void deleteNote()}
          />
        ) : (
          <div className="welcome">
            <BanyanMark size={56} />
            <p>
              เลือกโน้ตจากด้านซ้าย หรือกด <kbd>Ctrl K</kbd> เพื่อค้นหา/สร้างโน้ตใหม่
            </p>
          </div>
        )}

        {view === "editor" && (
          <RightPanel
            links={links}
            content={content}
            onOpen={followWikilink}
          />
        )}
      </div>

      {paletteOpen && (
        <CommandPalette
          allTitles={allTitles}
          onClose={() => setPaletteOpen(false)}
          onOpen={(v, t) => {
            setPaletteOpen(false);
            void openNote(v, t);
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
