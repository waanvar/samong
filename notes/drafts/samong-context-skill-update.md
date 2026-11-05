---
status: active        # เปลี่ยนเป็น done เมื่อนำไปแก้ skill แล้ว
date: 2026-11-05
---
# ร่าง: อัปเดต skill `samong-context` (จาก v0.3.6 → 0.6.1)

skill `samong-context` อยู่นอก repo (ใน skill ของบัญชี claude.ai) agent ใน session
แก้มันไม่ได้ — เจ้าของต้องนำข้อความนี้ไปวางเอง ทุกข้อด้านล่างตรวจกับ `README.md`,
`CHANGELOG.md` และ `Cargo.toml` ที่ 2026-11-05 แล้ว

## 1. แก้ด่วน — เคลมที่ผิด

ในหัวข้อ "หน้าที่ของ agent → โปรโมต/คอนเทนต์" ข้อ 1 ตอนนี้เขียนว่า:

> ค้นภาษาไทย/ญี่ปุ่น/จีน ด้วยการตัดคำ — คู่แข่งเกือบทั้งหมดพัง

**ไม่จริง** วัดแล้วว่าจีนและญี่ปุ่นทั้งประโยคกลายเป็นโทเคนเดียว
(ดู [[tokenizer-beyond-thai]]) เปลี่ยนเป็น:

> ค้นภาษาไทยด้วยการตัดคำ (newmm) — ค้นคำกลางประโยคเจอ ซึ่งเครื่องมือส่วนใหญ่ทำไม่ได้
> **ญี่ปุ่นและจีนยังไม่รองรับ** — เว็บพูดถึงสองภาษานี้ในฐานะปัญหาของภาษาที่ไม่มี
> ช่องว่าง และบอกตรงๆ ว่า "Thai ships today" ห้ามเคลมเกินนั้น

และในส่วน "ความสามารถหลัก" ข้อแรก เติมท้าย: "เฉพาะภาษาไทย ภาษาอื่นที่ไม่มีช่องว่าง
ยังไม่รองรับ"

## 2. บรรทัดสถานะ

เปลี่ยน "(เอกสารนี้อ้างอิงสถานะ ณ v0.3.6)" เป็น "(อ้างอิงสถานะ ณ v0.6.1 และ
Unreleased บน main)"

และเพิ่มในรายการ "ไฟล์ที่ควรอ่านซ้ำก่อนงานสำคัญ": `AGENTS.md`, `notes/START.md`,
`docs/DESIGN.md`

## 3. ความสามารถที่เพิ่มมาหลัง 0.3.6 (ปล่อยแล้ว)

- **`samong eval`** วัดคุณภาพการค้นกับคำถามจริง: hit@k, MRR และอัตราที่คำถามที่ตอบ
  ไม่ได้ถูกตอบ (`docs/EVAL.md`) — 0.5.0
- **`--semantic-floor` และ `--floors`** ตั้ง/กวาดค่า floor ของ semantic search
  **ยังไม่มีค่าเริ่มต้น** เพราะวัดบน vault จริงแล้วยังไม่ได้ตัวเลขที่ควรเป็นค่าเริ่มต้น
  โมเดล e5-small ให้ cosine ของข้อความไม่เกี่ยวกันราว 0.75 ค่าที่ลองต้องเริ่มที่
  0.70 ไม่ใช่ 0.2–0.4 — 0.5.0/0.6.0
- **ไบนารี Linux ARM64** (`samong-aarch64-linux.tar.gz`) — 0.6.0
  Windows on ARM ยังไม่มี
- **release notes มากับ release** ตัดจาก `CHANGELOG.md` อัตโนมัติ — 0.6.0
- **`samong pack` / `vault install` / `vault update` / `vault verify`** (มีอยู่แล้ว
  ใน skill แต่ให้ตรวจว่ายังตรง)
- **พอร์ตเริ่มต้นของ web UI คือ 3117** (`http://127.0.0.1:3117`) ไม่ใช่ 3000
  ไล่ขึ้นถึง 3128 ถ้าไม่ว่าง และตั้งเองได้ด้วย `--port` / `SAMONG_PORT`
- **ติดตั้งผ่าน package manager:** Homebrew tap, Scoop bucket, AUR (`packaging/aur/`
  — ยังไม่ได้ submit), winget (manifest พร้อม รอ Microsoft รับ) ช่องทางเหล่านี้
  **ไม่โดน Gatekeeper / SmartScreen** เพราะไม่ได้โหลดผ่านเบราว์เซอร์ และตรวจ SHA-256
  เอง — เปลี่ยนคำตอบเรื่อง "เปิดไฟล์ไม่ได้" ให้แนะนำทางนี้ก่อน `xattr`

## 4. อยู่บน main แต่ยังไม่ปล่อย (Unreleased) — ห้ามเคลมว่าปล่อยแล้ว

- `--explain` บน `search` และ `eval`
- `docs/DESIGN.md`, MCP `instructions`, `AGENTS.md`, dependency ที่แก้ advisory
  (รวม `dompurify` critical), race ใน search, SessionStart hook — merge แล้วแต่
  ยังไม่ได้ปล่อย ดู [[open-prs]]

## 5. ข้อจำกัดที่ต้องพูดตรงๆ (เพิ่ม)

- **Homebrew 6.0** ต้อง `brew trust waanvar/samong` ก่อน `brew tap` ไม่งั้นจะเจอ
  "invalid syntax in tap!" คำสั่งใน README ยังไม่ได้แก้จนกว่า branch
  `claude/sleepy-cray-dza0jb` จะ merge (ดู [[unmerged-branches]]) — ถ้าผู้ใช้
  รายงานข้อความนี้ นี่คือคำตอบ
- **ตัดคำได้แค่ไทย** (ดูข้อ 1)
- **คำที่ทีมตั้งเอง** (`LHVendor`, `getUserById`) ค้นด้วยคำย่อยไม่เจอ จนกว่า
  `claude/user-dictionary` จะ merge

## 6. Roadmap — อัปเดต

- **similarity floor:** มีแล้ว แต่ยังไม่มีค่าเริ่มต้น (ข้อ 3) — ไม่ใช่ "ยังไม่มี"
- **user dictionary:** มีงานใน branch แล้ว ยังไม่ merge
- ที่เหลือคงเดิม: โมเดล embedding ที่เล็กลง, Tauri, central server ที่ index จาก git

## 7. หน้าที่ของ agent — เพิ่ม

- **ก่อนทำงานกับโค้ด** อ่าน `AGENTS.md` แล้ว `notes/START.md` ซึ่งรวมงานค้าง ลำดับ
  merge และสิ่งที่วัดไว้แล้ว
- **ก่อนแตะหน้าจอ** อ่าน `docs/DESIGN.md`
- **หลังทำงาน** เขียนโน้ตส่งต่องานใน `notes/handoff/` ตาม template ใน
  `docs/AI-AGENT.md` ถ้างานยังไม่เสร็จ
- **ตรวจ PR:** ถ้า `Audit` แดงใน PR ที่ไม่ได้แตะ dependency ให้ดู main ก่อน —
  มันเคยแดงบน main ห้าสัปดาห์โดยไม่มีใครเห็น
