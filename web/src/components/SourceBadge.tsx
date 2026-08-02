import type { NoteSource } from "../api";
import { useT } from "../i18n";

/**
 * Who published a note that the reader did not write.
 *
 * One component for both places it appears — a search result and the note being
 * read — because those are the two moments where a paragraph gets copied out of
 * somebody else's vault, and the two must not be able to disagree about the
 * licence they show.
 *
 * A missing licence is stated, not omitted: "not stated" is an answer the reader
 * needs, and a badge that quietly shortens itself reads like there was nothing
 * to say.
 */
export function SourceBadge({ source }: { source: NoteSource | null }) {
  const t = useT();
  if (!source) return null;
  return (
    <span className="source-badge" title={t("source.tooltip", { name: source.name })}>
      {t("source.from", { name: source.name })}
      <span className="source-licence">
        {source.license ?? t("source.licenceUnstated")}
      </span>
    </span>
  );
}
