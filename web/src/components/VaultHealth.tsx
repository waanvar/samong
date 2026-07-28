import { useEffect, useState } from "react";
import { api, type DoctorReport } from "../api";

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
        aria-label={`สภาพ vault ${vault}`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="sheet-head">
          <h2>สภาพของ vault</h2>
          <button className="btn" onClick={onClose} aria-label="ปิด">
            ปิด
          </button>
        </header>

        {error && <p className="empty-hint">อ่านรายงานไม่ได้: {error}</p>}
        {!report && !error && <p className="empty-hint">กำลังตรวจ…</p>}

        {report && (
          <div className="sheet-body">
            <div className="stat-row">
              <div className="stat">
                <span className="stat-num">{report.project_notes}</span>
                <span className="stat-label">โน้ตของโปรเจกต์</span>
              </div>
              <div className="stat">
                <span className="stat-num">{report.reference_notes}</span>
                <span className="stat-label">อ้างอิง</span>
              </div>
              <div className="stat">
                <span className="stat-num">{report.skipped}</span>
                <span className="stat-label">ข้ามไป</span>
              </div>
            </div>

            <dl className="health-list">
              <dt>โฟลเดอร์</dt>
              <dd className="path">{report.vault}</dd>
              <dt>สแกนจาก</dt>
              <dd className="path">{report.notes_dir}</dd>
              <dt>.gitignore</dt>
              <dd>{report.follow_gitignore ? "เคารพ" : "ปิดไว้ใน samong.toml"}</dd>
            </dl>

            {report.include_roots.length > 0 && (
              <section>
                <div className="side-label">แหล่งอ้างอิง (scope.include)</div>
                <ul className="plain-list">
                  {report.include_roots.map((root) => (
                    <li key={root.path}>
                      <code className="path">{root.path}</code>{" "}
                      {root.present ? (
                        <span className="tag ok">พบ</span>
                      ) : (
                        <span className="tag warn">ไม่มีในเครื่องนี้</span>
                      )}
                    </li>
                  ))}
                </ul>
                {report.include_roots.some((r) => !r.present) && (
                  <p className="empty-hint">
                    แหล่งที่ไม่พบมักเป็นเพราะยังไม่ติดตั้ง dependency — โน้ตอ้างอิงจาก
                    ที่นั่นจะกลับมาเองเมื่อติดตั้งแล้ว
                  </p>
                )}
              </section>
            )}

            {report.skipped > 0 && (
              <section>
                <div className="side-label">ไฟล์ .md ที่ไม่ถูกนับเป็นโน้ต</div>
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
                    {report.skipped_dependency} ไฟล์อยู่ในโฟลเดอร์ dependency —
                    ถ้าอยากเรียนรู้จากเอกสารชุดไหน เพิ่ม path นั้นใน{" "}
                    <code>scope.include</code>
                  </p>
                )}
              </section>
            )}

            <section>
              <div className="side-label">ชื่อโน้ตที่กำกวม</div>
              {report.ambiguous_titles.length === 0 ? (
                <p className="empty-hint">
                  ไม่มีชื่อซ้ำในโน้ตของโปรเจกต์ — ทุก <code>[[ลิงก์]]</code> ชี้ได้ที่เดียว
                </p>
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
                  อีก {report.reference_only_collisions} ชื่อซ้ำกันเฉพาะในโน้ตอ้างอิง
                  ซึ่งปกติสำหรับเอกสารที่มาพร้อม dependency ไม่ต้องแก้
                </p>
              )}
            </section>
          </div>
        )}
      </aside>
    </div>
  );
}
