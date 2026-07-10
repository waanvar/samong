import { useEffect, useMemo, useRef, useState } from "react";
import { renderMarkdown } from "../markdown";

type Mode = "edit" | "split" | "preview";

interface Props {
  title: string;
  content: string;
  vault: string;
  allTitles: { vault: string; title: string }[];
  onChange: (content: string) => void;
  onFollow: (target: string) => void;
  onDelete: () => void;
}

interface SuggestState {
  query: string;
  /** Caret index right after "[[query". */
  caret: number;
  selected: number;
}

export function Editor({
  title,
  content,
  vault,
  allTitles,
  onChange,
  onFollow,
  onDelete,
}: Props) {
  const [mode, setMode] = useState<Mode>("split");
  const [suggest, setSuggest] = useState<SuggestState | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Close the suggestion box when switching notes.
  useEffect(() => setSuggest(null), [title]);

  const suggestions = useMemo(() => {
    if (!suggest) return [];
    const q = suggest.query.toLowerCase();
    const local = allTitles
      .filter((t) => t.vault === vault && t.title.toLowerCase().includes(q))
      .map((t) => ({ label: t.title, cross: false }));
    const cross = allTitles
      .filter(
        (t) =>
          t.vault !== vault &&
          `${t.vault}/${t.title}`.toLowerCase().includes(q),
      )
      .map((t) => ({ label: `${t.vault}/${t.title}`, cross: true }));
    return [...local, ...cross].slice(0, 12);
  }, [suggest, allTitles, vault]);

  const detectSuggest = (value: string, caret: number) => {
    const before = value.slice(0, caret);
    const m = /\[\[([^[\]|]*)$/.exec(before);
    setSuggest(m ? { query: m[1], caret, selected: 0 } : null);
  };

  const acceptSuggestion = (label: string) => {
    const ta = textareaRef.current;
    if (!ta || !suggest) return;
    const value = ta.value;
    const start = suggest.caret - suggest.query.length;
    const next = `${value.slice(0, start)}${label}]]${value.slice(suggest.caret)}`;
    onChange(next);
    setSuggest(null);
    requestAnimationFrame(() => {
      ta.focus();
      const pos = start + label.length + 2;
      ta.setSelectionRange(pos, pos);
    });
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!suggest || suggestions.length === 0) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const delta = e.key === "ArrowDown" ? 1 : -1;
      setSuggest({
        ...suggest,
        selected:
          (suggest.selected + delta + suggestions.length) % suggestions.length,
      });
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      acceptSuggestion(suggestions[suggest.selected].label);
    } else if (e.key === "Escape") {
      setSuggest(null);
    }
  };

  const html = useMemo(
    () => (mode === "edit" ? "" : renderMarkdown(content)),
    [content, mode],
  );

  const onPreviewClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const link = (e.target as HTMLElement).closest("a.wikilink");
    const target = link?.getAttribute("data-target");
    if (target) {
      e.preventDefault();
      onFollow(target);
    }
  };

  return (
    <section className="editor-col">
      <div className="editor-toolbar">
        <span className="title">{title}</span>
        <span className="spacer" style={{ flex: 1 }} />
        <div className="mode-switch" role="tablist" aria-label="โหมดแก้ไข">
          {(["edit", "split", "preview"] as Mode[]).map((m) => (
            <button
              key={m}
              className={mode === m ? "on" : ""}
              onClick={() => setMode(m)}
              role="tab"
              aria-selected={mode === m}
            >
              {m === "edit" ? "เขียน" : m === "split" ? "คู่กัน" : "อ่าน"}
            </button>
          ))}
        </div>
        <button className="btn danger" onClick={onDelete}>
          ลบ
        </button>
      </div>

      <div className={`editor-body ${mode === "split" ? "split" : ""}`}>
        {mode !== "preview" && (
          <div className="editor-wrap">
            <textarea
              ref={textareaRef}
              className="editor-textarea"
              value={content}
              spellCheck={false}
              aria-label="เนื้อหาโน้ต"
              onChange={(e) => {
                onChange(e.target.value);
                detectSuggest(e.target.value, e.target.selectionStart);
              }}
              onKeyDown={onKeyDown}
              onClick={(e) =>
                detectSuggest(
                  e.currentTarget.value,
                  e.currentTarget.selectionStart,
                )
              }
            />
            {suggest && suggestions.length > 0 && (
              <div className="wiki-suggest">
                <header>ลิงก์ไปยังโน้ต — Enter เพื่อเลือก</header>
                <ul>
                  {suggestions.map((s, i) => (
                    <li
                      key={s.label}
                      className={`${i === suggest.selected ? "sel" : ""} ${
                        s.cross ? "cross" : ""
                      }`}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        acceptSuggestion(s.label);
                      }}
                    >
                      {s.label}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
        {mode !== "edit" && (
          <div
            className="preview"
            onClick={onPreviewClick}
            // Rendered from the user's own markdown, sanitized with DOMPurify.
            dangerouslySetInnerHTML={{ __html: html }}
          />
        )}
      </div>
    </section>
  );
}
