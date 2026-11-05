---
status: active
date: 2026-11-05
branch:
---
# ส่งต่อ: เทสต์ phase12 ที่ล้มเป็นบางครั้ง (flaky test)

**เทสต์** `tests/phase12.rs` → `search_limit_is_a_total_across_vaults_not_per_vault`

**ที่เห็น** ล้มหนึ่งครั้งระหว่าง `cargo test --all --locked` ชุดเต็ม (panic ที่
`unwrap` ของ `Result` — `core::result::unwrap_failed`) จากนั้น:

- รันเดี่ยว `cargo test --locked --test phase12` ผ่าน 10/10
- รันชุดเต็มรอบถัดไป ผ่าน
- ตอนที่ล้ม commit ที่ทดสอบไม่ได้แตะ `src/` เลย

**สาเหตุที่น่าจะเป็น (ยังไม่ได้พิสูจน์)** เทสต์เรียก
`std::env::set_var("SAMONG_CONFIG_DIR", ...)` แล้วเปิด redb registry ขณะที่เทสต์อื่น
ใน binary เดียวกันรันขนานในเธรดอื่น environment variable เป็นของทั้งโปรเซส
เทสต์อื่นจึงเปลี่ยนค่าได้ระหว่าง `set_var` กับ `Registry::open()` หรือกับ child
process ที่มัน spawn แล้วสองเทสต์ไปใช้ registry หรือ lock ของกันและกัน

**ขั้นต่อไป**
1. ทำซ้ำก่อน: รัน `cargo test --locked --test phase12` เป็น loop ภายใต้โหลด
   (เช่น `-- --test-threads=8`) และเก็บข้อความ panic **ห้าม assert ด้วยเวลา**
2. แก้ที่ต้นเหตุ ไม่ใช่ retry: ส่ง config dir ให้ child ด้วย `Command::env` แทน
   environment ของทั้งโปรเซส หรือให้ registry รับ path ตรงๆ หรือจัดเทสต์ที่พึ่ง env
   ในไฟล์นี้ให้รันทีละตัว
3. ห้ามแตะ registry จริงที่ `~/.config/samong`

**ลองแล้วไม่ได้ผล** ยังไม่มี — ยังไม่ได้เริ่มแก้
