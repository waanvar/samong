---
status: active        # เปลี่ยนเป็น done เมื่อ PR ทั้งหมดในโน้ตนี้ merge หรือปิดแล้ว
date: 2026-11-05
branch: claude/audit-2026-11, claude/pensive-carson-5znch8, claude/provider-neutral-memory, claude/memory-notes
---
# ส่งต่อ: PR ที่เปิดอยู่ และลำดับการ merge (open pull requests, merge order)

## ลำดับที่ต้อง merge

1. **waanvar/samong#10** `claude/audit-2026-11` → `main`
   dependency advisory 20 ตัว และ `dompurify` ระดับ critical (DOM XSS ใน Markdown
   ที่ render ใน web UI) **ต้อง merge ก่อนปล่อย release ถัดไป**
2. **waanvar/samong#8** `claude/pensive-carson-5znch8` → `main`
   `docs/DESIGN.md` และ `tests/design_system.rs` (มีการแก้ของ #10 merge เข้ามาแล้ว
   ถ้า merge #8 ก่อน #10 ก็ได้ของ #10 ไปด้วย)
3. **waanvar/samong#9** `claude/provider-neutral-memory` → base เป็น branch ของ #8
   MCP `instructions`, `AGENTS.md`, template โน้ต หลัง #8 merge ให้เปลี่ยน base เป็น
   `main` (GitHub เปลี่ยนให้เองถ้าลบ branch ของ #8 ตอน merge)
4. **waanvar/samong#11** `claude/memory-notes` — โฟลเดอร์ `notes/` นี้ base เป็น
   branch ของ #9 ทำแบบเดียวกัน
5. **waanvar/samong#12** `claude/index-reader-no-watcher` → `main` แก้ race ที่การค้น
   ทำให้ reindex ถัดไปล้ม ("Directory not empty") อิสระจากสายข้างบน merge เมื่อไหร่
   ก็ได้ มีการแก้ของ #10 อยู่ข้างในแล้ว และถูก merge เข้า #11 แล้ว

#8–#11 ซ้อนกันเพราะแก้ `CLAUDE.md` / `CHANGELOG.md` ร่วมกัน PR ที่ merge แล้วถูก
squash (ดู `git log` ของ main: `(#7)`) ถ้า squash อันล่าง อันบนจะยังมี commit เดิม
อยู่ใน branch — diff หลังเปลี่ยน base ควรเหลือเฉพาะของตัวเอง ถ้าไม่ใช่ ให้ merge
`main` เข้า branch นั้น ห้าม rebase branch ของคนอื่น

6. **waanvar/samong#13** `claude/session-start-hook` → `main` SessionStart hook ที่รัน
   `.devcontainer/setup.sh` ใน cloud session ของ Claude Code ทำให้ `samong-mcp` มีตั้งแต่
   เริ่ม session อิสระจากสายข้างบน มีการแก้ของ #10 อยู่ข้างในแล้ว

## สถานะ ณ 2026-11-05

- CI ของ #8 และ #9 เคยแดงที่ job `cargo` และ `npm` ของ workflow `Audit` เท่านั้น
  (test, clippy, msrv, semantic, package, web เขียวหมด) สาเหตุคือ advisory บน
  `main` ไม่ใช่ของ PR — แก้ใน #10 และ merge เข้าทั้งสอง branch แล้ว ยังไม่ได้เห็น
  ผล CI รอบใหม่ตอนเขียนโน้ตนี้
- ในเครื่องหลัง merge: `cargo audit` exit 0, `npm audit --omit=dev` 0, 246 tests

## สิ่งที่ต้องรู้ซึ่งไม่อยู่ในโค้ด

- **Audit แดงบน main ห้าสัปดาห์โดยไม่มีใครเห็น** ล้มทุกรอบตั้งแต่ push วันที่
  1 ต.ค. 2026 และ schedule วันจันทร์อีกห้ารอบ (5, 12, 19, 26 ต.ค., 2 พ.ย.)
  ไม่มีอะไรอื่นแดง สัญญาณเดียวคือ check ที่ไม่มีใครเปิดดู ดู `.github/workflows/audit.yml`
  — ถ้าเห็น Audit แดงใน PR ที่ไม่ได้แตะ dependency ให้ดู main ก่อน
- `self_update` ถูกยกพื้นใน `Cargo.toml` เป็น `1.3.3` เพราะเป็นตัวแรกที่ใช้ `zip` 9
  ลดกลับไม่ได้: `zip` 8 แตก symlink และ SUID bit ตามที่ archive บอก และ
  `samong update` แตก archive ของ release
- ยังเหลือ warning ที่ไม่ทำให้ audit ล้ม: `lru` (unsound), `paste` (unmaintained)
  ผ่าน `tantivy` และ `vite` (dev dependency, high) ที่ audit ไม่ได้ตรวจเพราะใช้
  `--omit=dev`

## ขั้นต่อไป

ดู CI ของ #10, #8, #9 ให้เขียว แล้ว merge ตามลำดับข้างบน
หลังปล่อย release ถัดไป ลอง `samong update` กับ release จริงด้วยมือครั้งหนึ่ง
เพราะเส้นทางที่แตก archive จริงจาก GitHub ไม่ได้ถูกเทสต์ใน CI
