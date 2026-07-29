import { useMemo, useState } from "react";
import type { NoteInfo } from "../api";
import { useT } from "../i18n";

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
/**
 * Collapse chains of folders that hold nothing but one another.
 *
 * Vendored documentation arrives as `node_modules/next/dist/docs/01-app/…`,
 * which as a literal tree is five rows deep before the first thing you can read,
 * each row a single child, and the indentation pushes leaf names so far right
 * that they truncate to `0…`. Joining those into one `node_modules/next/dist/docs`
 * row costs nothing — no information is in the intermediate steps — and gives the
 * width back to the names. File browsers have done this for years.
 */
function compressBranch(folder: Folder): Folder {
  let current = folder;
  while (current.notes.length === 0 && current.folders.length === 1) {
    const only = current.folders[0];
    // Keep the child's `path`, so the collapsed row still addresses a real
    // directory — it is what the expand/collapse state is keyed on.
    current = { ...only, name: `${current.name}/${only.name}` };
  }
  return { ...current, folders: current.folders.map(compressBranch) };
}

/** The root is the vault, so it is never merged into a child — only branches are. */
function compress(root: Folder): Folder {
  return { ...root, folders: root.folders.map(compressBranch) };
}

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
  const t = useT();
  // Reference notes are somebody else's documentation, so they sit in their own
  // group rather than mixed in with the notes you wrote.
  const [own, reference] = useMemo(() => {
    const own: NoteInfo[] = [];
    const reference: NoteInfo[] = [];
    for (const note of notes) (note.reference ? reference : own).push(note);
    return [own, reference];
  }, [notes]);

  const ownTree = useMemo(() => compress(buildTree(own)), [own]);
  const referenceTree = useMemo(() => compress(buildTree(reference)), [reference]);

  // Seeded from the compressed trees: compression rewrites which rows exist, so
  // paths collected from the raw tree would key collapse state on rows that are
  // no longer rendered.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => {
    const seed = initialCollapsed(compress(buildTree(notes)));
    return seed;
  });
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
              <span className="tree-lock" aria-label={t("detail.readOnly")}>
                🔒
              </span>
            )}
          </button>
        </li>
      ))}
    </>
  );

  if (notes.length === 0) {
    return <p className="empty-hint">{t("tree.empty")}</p>;
  }

  return (
    <div className="note-tree">
      <div className="side-label">
        {t("tree.own")} <span className="tree-count">{own.length}</span>
      </div>
      <ul className="tree-list">{renderFolder(ownTree, 0)}</ul>

      {reference.length > 0 && (
        <>
          <div className="side-label ref">
            {t("tree.reference")} <span className="tree-count">{reference.length}</span>
          </div>
          <ul className="tree-list">{renderFolder(referenceTree, 0)}</ul>
        </>
      )}
    </div>
  );
}
