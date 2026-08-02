import { renderMarkdown } from "../markdown";
import type { NoteLinks, NoteSource } from "../api";
import { SourceBadge } from "./SourceBadge";
import { useT } from "../i18n";

interface Props {
  vault: string;
  noteKey: string;
  title: string;
  content: string;
  readOnly: boolean;
  source: NoteSource | null;
  links: NoteLinks | null;
  onOpenKey: (key: string) => void;
  onFollow: (target: string) => void;
  onExpand: () => void;
  onDelete: () => void;
}

/**
 * The selected note, beside the graph.
 *
 * Connections live here as chips rather than in a far column nobody looks at,
 * and each outgoing link says whether it actually resolves — a dangling link and
 * an ambiguous title are both things you want to see while reading, not after
 * running a command.
 */
export function DetailPanel({
  vault,
  noteKey,
  title,
  content,
  readOnly,
  source,
  links,
  onOpenKey,
  onFollow,
  onExpand,
  onDelete,
}: Props) {
  const t = useT();

  if (!noteKey) {
    return (
      <aside className="detail">
        <div className="detail-empty">
          <p>{t("detail.empty")}</p>
          <p className="empty-hint">{t("detail.emptyHint")}</p>
        </div>
      </aside>
    );
  }

  const forward = links?.forward ?? [];
  const backlinks = links?.backlinks ?? [];
  const cross = links?.cross_vault_backlinks ?? [];

  return (
    <aside className="detail">
      <header className="detail-head">
        <div className="detail-title-row">
          <h2>{title}</h2>
          {readOnly && (
            <span className="ref-badge" title={t("detail.readOnly.title")}>
              {t("detail.readOnly")}
            </span>
          )}
        </div>
        <SourceBadge source={source} />
      <div className="detail-path path">
          {vault} / {noteKey}
        </div>
        <div className="detail-actions">
          <button className="btn primary" onClick={onExpand}>
            {t("detail.expand")}
          </button>
          <button className="btn danger" onClick={onDelete} disabled={readOnly}>
            {t("detail.delete")}
          </button>
        </div>
      </header>

      <div className="detail-links">
        {forward.length > 0 && (
          <div className="chip-row">
            <span className="chip-label">{t("detail.outgoing")}</span>
            {forward.map((link) => (
              <button
                key={link.target}
                className={`chip ${link.keys.length === 0 ? "dangling" : ""} ${
                  link.keys.length > 1 ? "ambiguous" : ""
                }`}
                onClick={() =>
                  link.keys.length === 1 ? onOpenKey(link.keys[0]) : onFollow(link.target)
                }
                title={
                  link.keys.length === 0
                    ? t("link.missing.title")
                    : link.keys.length > 1
                      ? t("link.ambiguous.title", {
                          count: link.keys.length,
                          keys: link.keys.join(", "),
                        })
                      : link.keys[0]
                }
              >
                {link.target}
                {link.keys.length === 0 && (
                  <span className="chip-flag">{t("chip.missing")}</span>
                )}
                {link.keys.length > 1 && (
                  <span className="chip-flag">{t("chip.files", { count: link.keys.length })}</span>
                )}
              </button>
            ))}
          </div>
        )}

        {(backlinks.length > 0 || cross.length > 0) && (
          <div className="chip-row">
            <span className="chip-label">{t("detail.incoming")}</span>
            {backlinks.map((ref) => (
              <button
                key={ref.key}
                className="chip"
                onClick={() => onOpenKey(ref.key)}
                title={ref.key}
              >
                {ref.title}
              </button>
            ))}
            {cross.map((source) => (
              <button key={source} className="chip cross" onClick={() => onFollow(source)}>
                {source}
              </button>
            ))}
          </div>
        )}

        {forward.length === 0 && backlinks.length === 0 && cross.length === 0 && (
          <p className="empty-hint">{t("detail.noLinks")}</p>
        )}
      </div>

      <div
        className="detail-body preview"
        dangerouslySetInnerHTML={{ __html: renderMarkdown(content) }}
        onClick={(e) => {
          const target = e.target as HTMLElement;
          if (target.classList.contains("wikilink")) {
            onFollow(target.dataset.target ?? target.textContent ?? "");
          }
        }}
      />
    </aside>
  );
}
