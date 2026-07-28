import { useMemo, useState } from "react";
import type { NoteInfo } from "../api";

interface Props {
  notes: NoteInfo[];
  /** Key of the open note. */
  active: string;
  onOpen: (key: string) => void;
}

interface Folder {
  name: string;
  path: string;
  folders: Folder[];
  notes: NoteInfo[];
  /** Notes in this folder and everything under it. */
  total: number;
}

function emptyFolder(name: string, path: string): Folder {
  return { name, path, folders: [], notes: [], total: 0 };
}

/**
 * Group notes by their path. A vault that pulls in vendored documentation can
 * hold hundreds of notes — the Next.js docs alone are 425 files — and a flat
 * alphabetical list of those is unusable. The paths already describe a shape;
 * this just shows it.
 */
function buildTree(notes: NoteInfo[]): Folder {
  const root = emptyFolder("", "");
  for (const note of notes) {
    const segments = note.key.split("/");
    const fileName = segments.pop()!;
    let folder = root;
    folder.total += 1;
    let path = "";
    for (const segment of segments) {
      path = path ? `${path}/${segment}` : segment;
      let next = folder.folders.find((f) => f.name === segment);
      if (!next) {
        next = emptyFolder(segment, path);
        folder.folders.push(next);
      }
      next.total += 1;
      folder = next;
    }
    void fileName;
    folder.notes.push(note);
  }
  return root;
}

/** Folders holding the most notes are the ones worth collapsing first. */
function initialCollapsed(root: Folder): Set<string> {
  const collapsed = new Set<string>();
  const visit = (folder: Folder) => {
    for (const child of folder.folders) {
      if (child.total > 25) collapsed.add(child.path);
      visit(child);
    }
  };
  visit(root);
  return collapsed;
}

export function NoteTree({ notes, active, onOpen }: Props) {
  // Reference notes are somebody else's documentation, so they sit in their own
  // group rather than mixed in with the notes you wrote.
  const [own, reference] = useMemo(() => {
    const own: NoteInfo[] = [];
    const reference: NoteInfo[] = [];
    for (const note of notes) (note.reference ? reference : own).push(note);
    return [own, reference];
  }, [notes]);

  const ownTree = useMemo(() => buildTree(own), [own]);
  const referenceTree = useMemo(() => buildTree(reference), [reference]);

  const [collapsed, setCollapsed] = useState<Set<string>>(() =>
    initialCollapsed(buildTree(notes)),
  );
  const toggle = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(path)) next.add(path);
      return next;
    });

  const renderFolder = (folder: Folder, depth: number) => (
    <>
      {folder.folders.map((child) => {
        const isCollapsed = collapsed.has(child.path);
        return (
          <li key={child.path} className="tree-branch">
            <button
              className="tree-folder"
              style={{ paddingInlineStart: `${8 + depth * 12}px` }}
              onClick={() => toggle(child.path)}
              aria-expanded={!isCollapsed}
            >
              <span className={`tree-caret ${isCollapsed ? "" : "open"}`} aria-hidden>
                ▸
              </span>
              <span className="tree-name">{child.name}</span>
              <span className="tree-count">{child.total}</span>
            </button>
            {!isCollapsed && (
              <ul className="tree-list">{renderFolder(child, depth + 1)}</ul>
            )}
          </li>
        );
      })}
      {folder.notes.map((note) => (
        <li key={note.key}>
          <button
            className={`tree-note ${note.key === active ? "active" : ""}`}
            style={{ paddingInlineStart: `${20 + depth * 12}px` }}
            onClick={() => onOpen(note.key)}
            title={note.key}
          >
            <span className="tree-name">{note.title}</span>
            {note.reference && (
              <span className="tree-lock" aria-label="อ่านเท่านั้น">
                🔒
              </span>
            )}
          </button>
        </li>
      ))}
    </>
  );

  if (notes.length === 0) {
    return <p className="empty-hint">vault นี้ยังไม่มีโน้ต — กด “โน้ตใหม่” เพื่อเริ่ม</p>;
  }

  return (
    <div className="note-tree">
      <div className="side-label">
        โน้ตของโปรเจกต์ <span className="tree-count">{own.length}</span>
      </div>
      <ul className="tree-list">{renderFolder(ownTree, 0)}</ul>

      {reference.length > 0 && (
        <>
          <div className="side-label ref">
            อ้างอิง (อ่านเท่านั้น) <span className="tree-count">{reference.length}</span>
          </div>
          <ul className="tree-list">{renderFolder(referenceTree, 0)}</ul>
        </>
      )}
    </div>
  );
}
