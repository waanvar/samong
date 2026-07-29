import { renderMarkdown } from "../markdown";
import type { NoteLinks } from "../api";

interface Props {
  vault: string;
  noteKey: string;
  title: string;
  content: string;
  readOnly: boolean;
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
  links,
  onOpenKey,
  onFollow,
  onExpand,
  onDelete,
}: Props) {
  if (!noteKey) {
    return (
      <aside className="detail">
        <div className="detail-empty">
          <p>เลือกโน้ตจากแผนที่ หรือค้นหาด้านบน</p>
          <p className="empty-hint">
            แต่ละวงคือโน้ตหนึ่งไฟล์ · ขนาดวงบอกจำนวนลิงก์ · วงกลวงคือโน้ตอ่านเท่านั้น
          </p>
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
            <span className="ref-badge" title="มาจาก scope.include — แก้ไม่ได้">
              อ่านเท่านั้น
            </span>
          )}
        </div>
        <div className="detail-path path">
          {vault} / {noteKey}
        </div>
        <div className="detail-actions">
          <button className="btn primary" onClick={onExpand}>
            เปิดอ่านเต็มจอ
          </button>
          <button className="btn danger" onClick={onDelete} disabled={readOnly}>
            ลบ
          </button>
        </div>
      </header>

      <div className="detail-links">
        {forward.length > 0 && (
          <div className="chip-row">
            <span className="chip-label">ออกไป</span>
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
                    ? "ยังไม่มีโน้ตนี้ — กดเพื่อสร้าง"
                    : link.keys.length > 1
                      ? `ชื่อนี้ตรงกับ ${link.keys.length} ไฟล์: ${link.keys.join(", ")}`
                      : link.keys[0]
                }
              >
                {link.target}
                {link.keys.length === 0 && <span className="chip-flag">ยังไม่มี</span>}
                {link.keys.length > 1 && (
                  <span className="chip-flag">{link.keys.length} ไฟล์</span>
                )}
              </button>
            ))}
          </div>
        )}

        {(backlinks.length > 0 || cross.length > 0) && (
          <div className="chip-row">
            <span className="chip-label">เข้ามา</span>
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
          <p className="empty-hint">โน้ตนี้ยังไม่เชื่อมกับใคร — ใส่ [[ลิงก์]] เพื่อต่อเข้าแผนที่</p>
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
