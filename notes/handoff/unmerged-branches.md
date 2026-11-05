---
status: active
date: 2026-11-05
branch: claude/sleepy-cray-dza0jb, claude/user-dictionary, claude/read-section
---
# ส่งต่อ: branch ที่มีงานแต่ยังไม่มี PR (unmerged branches)

สาม branch มีงานที่ทำเสร็จแล้วแต่ไม่เคยเปิด PR ทั้งหมดถูกทดลอง merge กับ `main`
(ที่ `ad7e98b`) แบบไม่เขียนจริง: ชนกันเฉพาะ `CHANGELOG.md` หัวข้อ Unreleased
โค้ดไม่ชน แต่ branch ที่ตามหลัง 19 commit ต้องรันชุดตรวจใน `AGENTS.md` ใหม่หลัง merge

ลำดับที่แนะนำ: sleepy-cray → user-dictionary → read-section

## 1. `claude/sleepy-cray-dza0jb` — คำสั่งติดตั้ง brew ที่ใช้ไม่ได้แล้ว (เร่งที่สุด)

- 1 commit (`e4b27fd`) ตามหลัง main 5 commit เอกสารล้วน: `README.md`,
  `site/index.html`, `CHANGELOG.md`
- Homebrew 6.0 ไม่โหลด tap ของบุคคลที่สามจนกว่าจะ `brew trust` และ `brew tap`
  ที่ยังไม่ trust จะพิมพ์ "Invalid formula" ทีละบรรทัดแล้วจบด้วย
  "Cannot tap: invalid syntax in tap!" ทั้งที่ไฟล์ไม่ได้ผิด
- **คนที่ติดตั้งใหม่ด้วย brew เจออยู่ตอนนี้** — คำสั่งใน README และหน้าเว็บใช้ไม่ได้
  ตั้งแต่ผู้ใช้อัปเดตเป็น brew 6.0 branch นี้เพิ่ม `brew trust waanvar/samong`
  เป็นบรรทัดแรก

## 2. `claude/user-dictionary` — คำที่ทีมตั้งขึ้นเองต้องค้นเจอ

- 1 commit (`359bc5e`) ตามหลัง main 19 commit · 8 ไฟล์ +560 −34
  (`src/thai.rs` +331, `tests/vault_dictionary.rs` ใหม่)
- ปัญหาที่วัดได้: โน้ต "LHVendor ต่อ Informix ไม่ได้" ค้นด้วย `vendor` แล้วไม่ขึ้น
  อันดับหนึ่ง เพราะ tokenizer ตัดที่เครื่องหมายวรรคตอนอย่างเดียว `LHVendor` จึงเป็น
  โทเคนเดียว
- สองกลไก: (1) ตัดคำย่อยที่ขอบตัวเล็ก-ตัวใหญ่ (`getUser` → `get` + `User`,
  `JDBCDriver` → `JDBC` + `Driver`) (2) พจนานุกรมระดับ vault สำหรับคำที่ newmm
  ไม่รู้จัก
- เกี่ยวข้อง: commit `ad7e98b` บันทึกไว้ว่าค้น "ใบลา" ใน "ดึงใบลาจาก HR" ไม่เจอ
  เพราะ newmm ตัดเป็น ดึง|ใบ|ลาจาก — งานของพจนานุกรมระดับ vault
- เพิ่ม `INDEX_VERSION` จาก 3 เป็น 4 แล้ว (`src/indexer.rs`) และ reindex ใหม่เมื่อ
  พจนานุกรมเปลี่ยน ถ้ามี branch อื่นเพิ่มเลขนี้ก่อน merge ต้องเลื่อนเป็นเลขถัดไป
- งาน [[tokenizer-beyond-thai]] แก้ `src/thai.rs` ไฟล์เดียวกัน ควร merge branch นี้
  ก่อนเริ่มงานนั้น

## 3. `claude/read-section` — `read_note` อ่านทีละหัวข้อ

- 1 commit (`0f52403`) ตามหลัง main 19 commit · 5 ไฟล์ +480
  (`src/vault.rs` +196, `tests/read_section.rs` ใหม่)
- `read_note` รับ `section` (ชื่อหัวข้อ ไม่สนตัวพิมพ์) คืนหัวข้อนั้นกับเนื้อใต้มัน
  runbook ตัวอย่าง 877 → 344 ไบต์ (ไบต์ ไม่ใช่ token และจากไฟล์ตัวอย่างไฟล์เดียว)
- **ความเสี่ยงที่ commit message บันทึกไว้เอง:** `save_note` แทนที่ทั้งไฟล์ ถ้า agent
  อ่านแค่หัวข้อเดียวแล้วได้ `base_hash` มาด้วย มันอาจส่งเนื้อหัวข้อนั้นกลับไปเป็นเนื้อ
  โน้ตทั้งไฟล์ แล้วส่วนที่เหลือหายหมด branch นี้ปิดทางนี้แล้ว: การอ่านแบบ section
  **ไม่คืน `base_hash`** และขึ้นบรรทัดบอกให้อ่านทั้งโน้ตก่อนแก้ (`src/mcp.rs`)
  เทสต์ที่ป้องกัน: `tests/read_section.rs` →
  `a_section_read_is_smaller_and_grants_no_right_to_write`
- PR #9 ไม่ได้แก้คำอธิบาย `read_note` แต่เพิ่ม `INSTRUCTIONS` ไว้ใน `src/mcp.rs`
  ไฟล์เดียวกัน ต้องรันเทสต์ MCP ทั้งชุดหลัง merge

## branch อื่นที่ลบได้

`claude/awesome-hawking-eczb5v` (#5), `claude/embedder-once` (#6),
`claude/search-eval` (#7) merge แล้ว และ `claude/analyze-latest-version-w5aig0`
ตรงกับ main ทุก commit
