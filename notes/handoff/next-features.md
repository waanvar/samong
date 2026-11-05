---
status: active
date: 2026-11-05
branch:
---
# ส่งต่อ: ฟีเจอร์ที่เสนอแล้ว ยังไม่ได้ทำ (proposed features)

มาจากบทวิเคราะห์ใน [[provider-neutral-memory]] — แต่ละข้อแก้เคสที่ระบุไว้ที่นั่น
ข้อ 1–3 ของบทวิเคราะห์ทำแล้วใน PR #9 ทุกข้อด้านล่างทำในเครื่อง ไม่เพิ่ม network
call ไม่ต้องมีบัญชี และไม่สร้างรูปแบบที่มีแค่ Samong อ่าน (ผ่านกฎใน `AGENTS.md`)

## 1. ผลค้นติดป้ายโน้ตที่ถูกแทนแล้ว (superseded) — แก้เคส 3

- ปัญหา: โน้ตการตัดสินใจเก่ายังขึ้นอันดับสูง agent ทำตามสิ่งที่ถูกยกเลิกไปแล้ว
- template ใน `docs/AI-AGENT.md` มี frontmatter `status:` และ `superseded_by:`
  แล้ว แต่ **ตอนนี้เป็นข้อตกลงเท่านั้น** — ไม่มีโค้ดส่วนไหน parse frontmatter
  (ตรวจแล้ว: title มาจากชื่อไฟล์ `src/vault.rs` `title_from_path` และเนื้อทั้งหมด
  รวม frontmatter ถูก index เป็นข้อความ)
- ทำ: อ่าน YAML frontmatter ตอน index เก็บ `status` แล้วแสดงใน `search_notes`,
  CLI และ web UI ไม่ต้องลดอันดับ — บอกให้เห็นพอ
- ขนาด: กลาง ต้องเพิ่มเวอร์ชัน index และตัดสินว่าจะใช้ parser YAML ตัวไหน
  (dependency ใหม่ → `THIRD-PARTY.md`) หรืออ่านแค่ key บรรทัดเดียวเอง

## 2. `samong doctor` รายงานโน้ตที่อ้าง path ที่ไม่มีอยู่แล้ว — แก้เคส 10

- ปัญหา: โค้ดเปลี่ยน ไฟล์ถูกย้ายหรือลบ โน้ตที่อธิบายมันยังอยู่และยังพูดถึง path เดิม
  เป็นสัญญาณที่ชัดที่สุดว่าโน้ตนั้นเลิกจริงแล้ว
- ตอนนี้ `doctor` รายงานแค่ wikilink ที่ชี้ไปโน้ตที่ไม่มี (`broken` ใน `src/graph.rs`)
- ทำ: หา token รูป path ในโน้ต (`src/x.rs`, `docs/y.md` ใน backtick) ที่ไม่มีอยู่ใน
  vault แล้วรายงาน ระวังของที่ตั้งใจพูดถึงไฟล์ที่ไม่มี เช่น ตัวอย่างใน template
- ขนาด: กลาง

## 3. `save_note` ปฏิเสธเนื้อหาที่หน้าตาเหมือน secret — แก้เคส 7

- ปัญหา: โน้ตถูก commit ลง git agent ที่เผลอใส่ API key ทำให้ key หลุดไปกับ repo
  — repo นี้เป็น public
- MCP `instructions` (PR #9) สั่งห้ามไว้แล้ว แต่คำสั่งไม่ใช่การกัน
- ทำ: ตรวจรูปแบบที่ชัดเจน (`ghp_`, `sk-`, `AKIA`, `-----BEGIN ... PRIVATE KEY-----`)
  แล้ว **ปฏิเสธพร้อม error ที่บอกว่าเจออะไรตรงไหน** ไม่ใช่ตัดออกเงียบๆ (กฎใน
  `AGENTS.md`: loud error ดีกว่า quiet fallback) ต้องมีเทสต์ที่ล้มถ้ารับ secret ได้
- ขนาด: เล็ก–กลาง

## 4. merge `claude/user-dictionary` — แก้เคส 5

ดู [[unmerged-branches]]

## 5. SessionStart hook สำหรับ cloud session

- cloud session ของ Claude Code ไม่มี `samong-mcp` ใน `PATH` จึงต่อ Samong ไม่ได้
  ตั้งแต่ต้น (`.mcp.json` ของ repo นี้ชี้ไปที่ `samong-mcp`) agent ใน cloud จึงใช้
  ความจำในโฟลเดอร์นี้ผ่าน MCP ไม่ได้ อ่านไฟล์ตรงๆ ได้อย่างเดียว
- ทำ: hook ที่ build และติดตั้ง `samong-mcp` ตอนเริ่ม session

## ข้อที่จงใจไม่เสนอ

sync ข้ามเครื่อง, delete tool บน MCP, reference note ที่แก้ได้ — `AGENTS.md`
บอกว่าจะถูกปฏิเสธ "AI features" ใน Roadmap เป็น open-core ภายหลัง และเป็นการ
ตัดสินใจของเจ้าของโปรเจกต์
