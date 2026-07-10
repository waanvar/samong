import type { NoteLinks } from "../api";
import { outline } from "../markdown";

interface Props {
  links: NoteLinks | null;
  content: string;
  onOpen: (target: string) => void;
}

export function RightPanel({ links, content, onOpen }: Props) {
  const headings = outline(content);
  const backlinks = links?.backlinks ?? [];
  const cross = links?.cross_vault_backlinks ?? [];

  return (
    <aside className="right-panel">
      <div className="panel-section">
        <div className="side-label">ลิงก์มาที่นี่ ({backlinks.length + cross.length})</div>
        {backlinks.length + cross.length === 0 ? (
          <p className="empty-hint">ยังไม่มีโน้ตอื่นลิงก์มา</p>
        ) : (
          <ul className="backlink-list">
            {backlinks.map((source) => (
              <li key={source}>
                <button onClick={() => onOpen(source)}>{source}</button>
              </li>
            ))}
            {cross.map((source) => (
              <li key={source} className="cross">
                <button onClick={() => onOpen(source)}>{source}</button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="panel-section">
        <div className="side-label">โครงร่าง</div>
        {headings.length === 0 ? (
          <p className="empty-hint">ยังไม่มีหัวข้อ</p>
        ) : (
          <ul className="outline-list">
            {headings.map((h) => (
              <li key={`${h.line}`} className={`lvl-${Math.min(h.level, 3)}`}>
                <button
                  onClick={() => {
                    document
                      .querySelectorAll(".preview h1, .preview h2, .preview h3, .preview h4")
                      .forEach((el) => {
                        if (el.textContent?.trim() === h.text) {
                          el.scrollIntoView({ behavior: "smooth", block: "start" });
                        }
                      });
                  }}
                >
                  {h.text}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}
