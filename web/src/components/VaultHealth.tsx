import { useEffect, useState } from "react";
import { api, type DoctorReport } from "../api";
import { useT } from "../i18n";

interface Props {
  vault: string;
  onClose: () => void;
  onOpen: (key: string) => void;
}

/**
 * What the vault actually indexed, and what it left out.
 *
 * Until now this only existed in `samong doctor`. Someone working in the browser
 * could open a vault showing four notes with no way to learn that ninety more
 * were skipped, or that a `scope.include` directory is missing on this machine —
 * and would reasonably conclude that search was broken.
 */
export function VaultHealth({ vault, onClose, onOpen }: Props) {
  const t = useT();
  const [report, setReport] = useState<DoctorReport | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;
    api.doctor(vault).then(
      (r) => !cancelled && setReport(r),
      (err: Error) => !cancelled && setError(err.message),
    );
    return () => {
      cancelled = true;
    };
  }, [vault]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="sheet-backdrop" onMouseDown={onClose}>
      <aside
        className="sheet"
        role="dialog"
        aria-label={t("health.aria", { vault })}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="sheet-head">
          <h2>{t("health.title")}</h2>
          <button className="btn" onClick={onClose} aria-label={t("health.close")}>
            {t("health.close")}
          </button>
        </header>

        {error && <p className="empty-hint">{t("health.error", { error })}</p>}
        {!report && !error && <p className="empty-hint">{t("health.loading")}</p>}

        {report && (
          <div className="sheet-body">
            <div className="stat-row">
              <div className="stat">
                <span className="stat-num">{report.project_notes}</span>
                <span className="stat-label">{t("health.stat.project")}</span>
              </div>
              <div className="stat">
                <span className="stat-num">{report.reference_notes}</span>
                <span className="stat-label">{t("health.stat.reference")}</span>
              </div>
              <div className="stat">
                <span className="stat-num">{report.skipped}</span>
                <span className="stat-label">{t("health.stat.skipped")}</span>
              </div>
            </div>

            {/* A personal vault leaves all of this blank; a vault that came from
                somebody else is where it starts to matter. */}
            <dl className="health-list">
              {report.manifest.description && (
                <>
                  <dt>{t("health.about")}</dt>
                  <dd>{report.manifest.description}</dd>
                </>
              )}
              {report.manifest.version && (
                <>
                  <dt>{t("health.version")}</dt>
                  <dd>{report.manifest.version}</dd>
                </>
              )}
              {report.manifest.license && (
                <>
                  <dt>{t("health.license")}</dt>
                  <dd>{report.manifest.license}</dd>
                </>
              )}
              {report.manifest.source && (
                <>
                  <dt>{t("health.source")}</dt>
                  <dd className="path">{report.manifest.source}</dd>
                </>
              )}
              <dt>{t("health.folder")}</dt>
              <dd className="path">{report.vault}</dd>
              <dt>{t("health.scannedFrom")}</dt>
              <dd className="path">{report.notes_dir}</dd>
              <dt>.gitignore</dt>
              <dd>{t(report.follow_gitignore ? "health.gitignore.on" : "health.gitignore.off")}</dd>
            </dl>

            {report.include_roots.length > 0 && (
              <section>
                <div className="side-label">{t("health.includeRoots")}</div>
                <ul className="plain-list">
                  {report.include_roots.map((root) => (
                    <li key={root.path}>
                      <code className="path">{root.path}</code>{" "}
                      {root.present ? (
                        <span className="tag ok">{t("health.present")}</span>
                      ) : (
                        <span className="tag warn">{t("health.absent")}</span>
                      )}
                    </li>
                  ))}
                </ul>
                {report.include_roots.some((r) => !r.present) && (
                  <p className="empty-hint">{t("health.absentHint")}</p>
                )}
              </section>
            )}

            {report.skipped > 0 && (
              <section>
                <div className="side-label">{t("health.skippedTitle")}</div>
                <ul className="bar-list">
                  {report.skipped_by_dir.slice(0, 6).map(([dir, count]) => (
                    <li key={dir}>
                      <span className="bar-name">{dir}</span>
                      <span
                        className="bar"
                        style={{
                          // Relative to the largest group, so the shape reads at a glance.
                          inlineSize: `${Math.round(
                            (count / report.skipped_by_dir[0][1]) * 100,
                          )}%`,
                        }}
                      />
                      <span className="bar-num">{count}</span>
                    </li>
                  ))}
                </ul>
                {report.skipped_dependency > 0 && (
                  <p className="empty-hint">
                    {t("health.skippedDependency", { count: report.skipped_dependency })}
                  </p>
                )}
              </section>
            )}

            {/* Only rendered when the server can do semantic search at all: a
                build without it should not advertise a feature it does not have. */}
            {report.embeddings && (
              <section>
                <div className="side-label">{t("health.semantic")}</div>
                {report.embeddings.notes === 0 ? (
                  <p className="empty-hint">{t("health.semantic.none")}</p>
                ) : (
                  <>
                    <p className="empty-hint">
                      {t("health.semantic.count", { count: report.embeddings.notes })}
                      {report.embeddings.model ? ` · ${report.embeddings.model}` : ""}
                    </p>
                    {report.embeddings.missing_project > 0 && (
                      <p className="empty-hint">
                        {t("health.semantic.missingProject", {
                          count: report.embeddings.missing_project,
                        })}
                      </p>
                    )}
                    {report.embeddings.missing_reference > 0 && (
                      <p className="empty-hint">
                        {t("health.semantic.missingReference", {
                          count: report.embeddings.missing_reference,
                        })}
                      </p>
                    )}
                  </>
                )}
              </section>
            )}

            <section>
              <div className="side-label">{t("health.ambiguousTitle")}</div>
              {report.ambiguous_titles.length === 0 ? (
                <p className="empty-hint">{t("health.noAmbiguous")}</p>
              ) : (
                <ul className="plain-list">
                  {report.ambiguous_titles.map((entry) => (
                    <li key={entry.title}>
                      <b>{entry.title}</b>
                      <ul className="plain-list nested">
                        {entry.keys.map((key) => (
                          <li key={key}>
                            <button className="link-btn path" onClick={() => onOpen(key)}>
                              {key}
                            </button>
                          </li>
                        ))}
                      </ul>
                    </li>
                  ))}
                </ul>
              )}
              {report.reference_only_collisions > 0 && (
                <p className="empty-hint">
                  {t("health.referenceOnly", { count: report.reference_only_collisions })}
                </p>
              )}
            </section>
          </div>
        )}
      </aside>
    </div>
  );
}
