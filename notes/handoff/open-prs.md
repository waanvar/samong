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

**ปล่อยแล้วใน v0.7.0 (2026-11-06)** — release commit `3c1517e` (#14), tag ชี้ที่
`142b737` (main หลัง #15 ซึ่งต่างกันแค่ไฟล์ workflow) ตรวจหลังปล่อยแล้ว: archive
ห้าแพลตฟอร์ม + `.mcpb` บนหน้า release, crates.io 0.7.0, MCP registry 0.7.0
`isLatest`, ไบนารี Linux ที่ดาวน์โหลดจริงผ่าน sha256 รายงาน 0.7.0 ค้นไทยถูก และ
ค้นแล้ว reindex --full ผ่าน 20/20

ค้างหลังปล่อย:
- **winget ล้ม** "The forked repository could not be synced with the upstream
  commits" เหมือน 0.6.0 และ 0.6.1 — fork ของ `microsoft/winget-pkgs` ต้อง sync
  ด้วยมือ แล้วรัน job winget ใหม่ (Actions → Release → re-run failed jobs)
- **AUR ไม่ได้ push** — step `Push to the AUR` ถูกข้ามเพราะไม่มี `AUR_SSH_KEY`
  (เหมือนรุ่นก่อน)
- ลอง `samong update` จาก 0.6.1 บนเครื่องจริงหนึ่งครั้ง — `zip` ขึ้น major

**tag ของรุ่นนี้ไม่ได้มาจาก git push** cloud session push tag ไม่ได้ (403) และ push
จากเครื่องเจ้าของสองรอบไม่ถึง GitHub จึงเพิ่ม `.github/workflows/tag-release.yml`
(#15): สั่งด้วย `version` แล้วมันตรวจกับ Cargo.toml และ CHANGELOG สร้าง tag และสั่ง
`release.yml` ให้ — รุ่นถัดไปใช้ทางนี้ได้เลย

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

- sync fork ของ winget-pkgs แล้ว re-run job winget ของ release v0.7.0
- ลอง `samong update` จาก 0.6.1 ด้วยมือครั้งหนึ่ง
- ลบ branch ที่ merge แล้วได้: `claude/audit-2026-11`, `claude/session-start-hook`,
  `claude/pensive-carson-5znch8`, `claude/index-reader-no-watcher`,
  `claude/provider-neutral-memory`, `claude/memory-notes` และของเก่าใน
  [[unmerged-branches]]
