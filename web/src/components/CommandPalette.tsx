import { useEffect, useMemo, useRef, useState } from "react";
import { api, type SearchHit } from "../api";

interface Props {
  allTitles: { vault: string; title: string }[];
  onClose: () => void;
  onOpen: (vault: string, title: string) => void;
  onCreate: (title: string) => void;
}

interface Item {
  kind: "note" | "search" | "create";
  vault: string;
  title: string;
  snippet?: string;
}

export function CommandPalette({ allTitles, onClose, onOpen, onCreate }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

  // Full-text search, debounced against the API.
  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    const timer = window.setTimeout(() => {
      api.search(query).then(setHits, () => setHits([]));
    }, 250);
    return () => window.clearTimeout(timer);
  }, [query]);

  const items = useMemo<Item[]>(() => {
    const q = query.trim().toLowerCase();
    const titleMatches: Item[] = (
      q
        ? allTitles.filter((t) => t.title.toLowerCase().includes(q))
        : allTitles
    )
      .slice(0, 8)
      .map((t) => ({ kind: "note", vault: t.vault, title: t.title }));

    const seen = new Set(titleMatches.map((i) => `${i.vault}/${i.title}`));
    const searchMatches: Item[] = hits
      .filter((h) => !seen.has(`${h.vault}/${h.title}`))
      .slice(0, 8)
      .map((h) => ({
        kind: "search",
        vault: h.vault,
        title: h.title,
        snippet: h.snippet.replace(/<[^>]*>/g, (tag) =>
          tag === "<b>" || tag === "</b>" ? tag : "",
        ),
      }));

    const out = [...titleMatches, ...searchMatches];
    if (q && !allTitles.some((t) => t.title.toLowerCase() === q)) {
      out.push({ kind: "create", vault: "", title: query.trim() });
    }
    return out;
  }, [query, allTitles, hits]);

  useEffect(() => setSelected(0), [query, items.length]);

  const activate = (item: Item) => {
    if (item.kind === "create") onCreate(item.title);
    else onOpen(item.vault, item.title);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => (s + 1) % Math.max(items.length, 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => (s - 1 + items.length) % Math.max(items.length, 1));
    } else if (e.key === "Enter" && items[selected]) {
      activate(items[selected]);
    }
  };

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="ค้นหาและสร้างโน้ต"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          placeholder="พิมพ์เพื่อค้นหาโน้ต หรือตั้งชื่อโน้ตใหม่…"
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <ul>
          {items.map((item, i) => (
            <li
              key={`${item.kind}-${item.vault}-${item.title}`}
              className={i === selected ? "sel" : ""}
              onMouseEnter={() => setSelected(i)}
              onClick={() => activate(item)}
            >
              <span className={`kind ${item.kind === "create" ? "create" : ""}`}>
                {item.kind === "note"
                  ? item.vault
                  : item.kind === "search"
                    ? `ค้นเจอ · ${item.vault}`
                    : "สร้างใหม่"}
              </span>
              <span>{item.title}</span>
              {item.snippet && (
                <span
                  className="snippet"
                  dangerouslySetInnerHTML={{ __html: item.snippet }}
                />
              )}
            </li>
          ))}
          {items.length === 0 && <li className="empty-hint">ไม่พบผลลัพธ์</li>}
        </ul>
        <div className="hint">
          <span>
            <kbd>↑↓</kbd> เลือก
          </span>
          <span>
            <kbd>Enter</kbd> เปิด
          </span>
          <span>
            <kbd>Esc</kbd> ปิด
          </span>
        </div>
      </div>
    </div>
  );
}
