---
status: done          # ทุก PR ในโน้ตนี้ merge แล้ว — เก็บไว้เป็นบันทึกว่าเกิดอะไรขึ้น
date: 2026-11-05
branch:
---
# PR ชุดวันที่ 5 พ.ย. 2026 — merge ครบแล้ว (merged pull requests)

โน้ตนี้เคยเป็นลำดับ merge ของ PR ที่เปิดค้าง ตอนนี้ merge ครบแล้วทั้งหมด (squash)
เก็บไว้เพราะเหตุผลของแต่ละอันและสิ่งที่เจอระหว่างทางไม่อยู่ในโค้ด

| ลำดับ | PR | commit บน main | อะไร |
|---|---|---|---|
| 1 | waanvar/samong#10 | `8b9f78b` | dependency advisory 20 ตัว และ `dompurify` critical |
| 2 | waanvar/samong#13 | `65c1a38` | SessionStart hook ให้ cloud session มี `samong-mcp` |
| 3 | waanvar/samong#8 | `937eeca` | `docs/DESIGN.md` และ `tests/design_system.rs` |
| 4 | waanvar/samong#12 | `2aec45f` | race: การค้นทำให้ reindex ถัดไปล้ม ("Directory not empty") |
| 5 | waanvar/samong#9 | `6a59086` | MCP `instructions`, `AGENTS.md`, template โน้ต |
| 6 | waanvar/samong#11 | (PR ของโฟลเดอร์ `notes/` นี้) | ความจำของการทำงาน |

ทั้งหมดยังอยู่ใต้ Unreleased ใน `CHANGELOG.md` — **ยังไม่มี release ที่มีของเหล่านี้**
release ถัดไปควรออกเร็ว เพราะ v0.6.1 ที่ผู้ใช้โหลดอยู่ยังมี `dompurify` ที่มี
advisory ระดับ critical

## สิ่งที่ต้องรู้ซึ่งไม่อยู่ในโค้ด

- **Audit แดงบน main ห้าสัปดาห์โดยไม่มีใครเห็น** ล้มทุกรอบตั้งแต่ push วันที่
  1 ต.ค. 2026 และ schedule วันจันทร์อีกห้ารอบ ไม่มีอะไรอื่นแดง สัญญาณเดียวคือ check
  ที่ไม่มีใครเปิดดู (`.github/workflows/audit.yml`) ถ้าเห็น Audit แดงใน PR ที่ไม่ได้
  แตะ dependency ให้ดู main ก่อน
- `self_update` ถูกยกพื้นใน `Cargo.toml` เป็น `1.3.3` เพราะเป็นตัวแรกที่ใช้ `zip` 9
  ลดกลับไม่ได้: `zip` 8 แตก symlink และ SUID bit ตามที่ archive บอก และ
  `samong update` แตก archive ของ release
- warning ที่ยังเหลือและไม่ทำให้ audit ล้ม: `lru` (unsound), `paste` (unmaintained)
  ผ่าน `tantivy` และ `vite` (dev dependency, high) ที่ audit ไม่ได้ตรวจเพราะใช้
  `--omit=dev`
- job `aur` ล้มสองครั้งในวันเดียวด้วย HTTP 504 ตอนโหลด release asset ของ v0.6.1
  จาก GitHub — เป็นเครือข่าย ไม่ใช่โค้ด re-run แล้วผ่านทั้งสองครั้ง
- **squash กับ PR ที่ซ้อนกันชน `CHANGELOG.md` ทุกครั้ง** เพราะ commit บน main เป็น
  commit ใหม่ที่ git ไม่รู้ว่าเหมือนของใน branch ครั้งหนึ่ง `docs/AI-AGENT.md` ได้
  หัวข้อซ้ำสองชุดจากการ merge อัตโนมัติที่ไม่มี conflict — หลัง merge `main` เข้า
  branch ให้นับหัวข้อซ้ำเสมอ ไม่ใช่ดูแค่ว่ามี conflict marker ไหม

## ขั้นต่อไป

- ปล่อย release ถัดไป (ดู `docs/RELEASE-LESSONS.md` ก่อน)
- หลังปล่อย ลอง `samong update` กับ release จริงด้วยมือครั้งหนึ่ง เพราะเส้นทาง
  ที่แตก archive จริงจาก GitHub ไม่ได้ถูกเทสต์ใน CI และ `zip` เพิ่งขึ้น major
- ลบ branch ที่ merge แล้วได้: `claude/audit-2026-11`, `claude/session-start-hook`,
  `claude/pensive-carson-5znch8`, `claude/index-reader-no-watcher`,
  `claude/provider-neutral-memory`, `claude/memory-notes` และของเก่าใน
  [[unmerged-branches]]
