import { useEffect, useRef, useState } from "react";
import { api, type SearchHit } from "../api";
import { useT } from "../i18n";
import { SourceBadge } from "./SourceBadge";

interface Props {
  vault: string;
  /** Search every vault instead of only the current one. */
  allVaults: boolean;
  /** Keys that matched, or null when the field is empty. Drives graph dimming. */
  onMatches: (keys: Set<string> | null) => void;
  onOpen: (vault: string, key: string) => void;
  onCreate: (title: string) => void;
}

/**
 * Search is the way into the graph, so it lives in the frame permanently rather
 * than behind a shortcut. Typing dims everything in the graph that does not
 * match, which turns a query into a place.
 */
export function SearchPanel({ vault, allVaults, onMatches, onOpen, onCreate }: Props) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState(0);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Ctrl/Cmd+K focuses the field instead of opening a dialog — there is nothing
  // to open, the field is already on screen.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const text = query.trim();
    if (!text) {
      setHits([]);
      setOpen(false);
      onMatches(null);
      return;
    }
    setBusy(true);
    const timer = window.setTimeout(() => {
      api.search(text, allVaults ? undefined : vault, 30).then(
        (results) => {
          setHits(results);
          setOpen(true);
          setSelected(0);
          setBusy(false);
          onMatches(new Set(results.map((h) => h.path)));
        },
        () => {
          setHits([]);
          setBusy(false);
          onMatches(new Set());
        },
      );
    }, 220);
    return () => window.clearTimeout(timer);
    // onMatches is stable in the parent.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, vault, allVaults]);

  const choose = (hit: SearchHit) => {
    setOpen(false);
    onOpen(hit.vault, hit.path);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      if (query) {
        setQuery("");
      } else {
        inputRef.current?.blur();
      }
      setOpen(false);
      return;
    }
    if (!hits.length) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setSelected((s) => (s + 1) % hits.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => (s - 1 + hits.length) % hits.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(hits[selected]);
    }
  };

  const canCreate =
    query.trim().length > 0 && !hits.some((h) => h.title === query.trim());

  return (
    <div className="search-panel">
      <div className="search-field">
        <span className="search-icon" aria-hidden>
          ⌕
        </span>
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          onFocus={() => hits.length && setOpen(true)}
          placeholder={t("search.placeholder")}
          aria-label={t("search.aria")}
          spellCheck={false}
        />
        {query && (
          <span className="search-meta">
            {busy ? "…" : t("search.count", { count: hits.length })}
          </span>
        )}
        {query && (
          <button className="search-clear" onClick={() => setQuery("")} aria-label={t("search.clear")}>
            ✕
          </button>
        )}
        <kbd>Ctrl K</kbd>
      </div>

      {open && (query.trim() ? true : false) && (
        <div className="search-results" role="listbox">
          {hits.length === 0 && !busy && (
            <p className="empty-hint">
              {t("search.none", { query: query.trim() })}
              {canCreate && t("search.noneCreate")}
            </p>
          )}
          {hits.map((hit, i) => (
            <button
              key={`${hit.vault}/${hit.path}`}
              className={`search-hit ${i === selected ? "sel" : ""}`}
              role="option"
              aria-selected={i === selected}
              onMouseEnter={() => setSelected(i)}
              onClick={() => choose(hit)}
            >
              <span className="hit-head">
                <span className="hit-title">{hit.title}</span>
                <span className="path">{hit.path}</span>
                <SourceBadge source={hit.source} />
              </span>
              <span
                className="snippet"
                // Snippets arrive with <b> around each matched token; the
                // stylesheet turns those into the word-boundary marks.
                dangerouslySetInnerHTML={{ __html: hit.snippet }}
              />
            </button>
          ))}
          {canCreate && (
            <button
              className="search-hit create"
              onClick={() => {
                const title = query.trim();
                setQuery("");
                onCreate(title);
              }}
            >
              <span className="hit-head">
                <span className="hit-title">{t("search.create", { query: query.trim() })}</span>
              </span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}
