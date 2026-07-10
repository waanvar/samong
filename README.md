# Banyan 🌳

โน้ต = ไฟล์ Markdown ธรรมดา เข้ากันได้กับ [Obsidian](https://obsidian.md) (`[[wikilink]]` และ
`[[wikilink|alias]]`) — Banyan เพิ่ม link graph ที่เร็ว ([redb](https://github.com/cberner/redb))
และ full-text search ([tantivy](https://github.com/quickwit-oss/tantivy)) ให้กับ vault ของคุณ
โดยไฟล์ `.md` ยังคงเป็น source of truth เพียงแหล่งเดียวเสมอ — index ทั้งหมดอยู่ใน `<vault>/.brain/`
และสร้างใหม่ได้ทุกเมื่อด้วย `banyan reindex`

> **สถานะ**: Phase 1 (ดู `PLAN.md`) — วงจรชีวิตโน้ตครบ + incremental reindex + watch mode
> ยังไม่มี multi-vault/Thai tokenizer/API/Web UI

## ติดตั้ง / Build

```sh
cargo build --release
```

Binary จะอยู่ที่ `target/release/banyan` (หรือ `banyan.exe` บน Windows)

## การใช้งาน

Vault = current working directory เสมอในเวอร์ชันนี้

```sh
cd my-vault/

banyan new "My First Note"     # สร้างโน้ตใหม่แล้ว index ให้อัตโนมัติ
banyan edit "My First Note"    # เปิดโน้ตใน $EDITOR แล้ว reindex เมื่อปิด
banyan rename "Old" "New"      # เปลี่ยนชื่อ + แก้ [[wikilink]] ในทุกโน้ตที่ลิงก์มา
banyan delete "My First Note"  # ลบโน้ต + เตือน backlinks ที่จะค้าง
banyan links "My First Note"   # แสดง forward links และ backlinks
banyan orphans                 # โน้ตที่ไม่มีใครลิงก์หา
banyan broken                  # ลิงก์ที่ชี้ไปโน้ตที่ไม่มีจริง
banyan search "some query"     # ค้นหาแบบ full-text
banyan graph                   # แสดงทุก edge ของ link graph (from -> to)
banyan list                    # แสดงรายชื่อโน้ตทั้งหมด
banyan reindex                 # sync index กับไฟล์ .md (เฉพาะไฟล์ที่เปลี่ยน)
banyan reindex --full          # rebuild ทุกอย่างจากศูนย์
banyan watch                   # เฝ้า vault แล้วอัปเดต index อัตโนมัติ
```

## Development

```sh
cargo test
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

## License

[AGPL-3.0-only](LICENSE)
