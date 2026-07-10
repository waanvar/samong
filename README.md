# Banyan 🌳

โน้ต = ไฟล์ Markdown ธรรมดา เข้ากันได้กับ [Obsidian](https://obsidian.md) (`[[wikilink]]` และ
`[[wikilink|alias]]`) — Banyan เพิ่ม link graph ที่เร็ว ([redb](https://github.com/cberner/redb))
และ full-text search ([tantivy](https://github.com/quickwit-oss/tantivy)) ให้กับ vault ของคุณ
โดยไฟล์ `.md` ยังคงเป็น source of truth เพียงแหล่งเดียวเสมอ — index ทั้งหมดอยู่ใน `<vault>/.brain/`
และสร้างใหม่ได้ทุกเมื่อด้วย `banyan reindex`

> **สถานะ**: Phase 0 (ดู `PLAN.md`) — core CLI ใช้งานได้, ยังไม่มี edit/delete/rename/watch/
> multi-vault/Thai tokenizer/API/Web UI

## ติดตั้ง / Build

```sh
cargo build --release
```

Binary จะอยู่ที่ `target/release/banyan` (หรือ `banyan.exe` บน Windows)

## การใช้งาน

Vault = current working directory เสมอในเวอร์ชันนี้

```sh
cd my-vault/

banyan new "My First Note"     # สร้างโน้ตใหม่แล้ว reindex ให้อัตโนมัติ
banyan reindex                 # rebuild link graph + full-text index จากทุกไฟล์ .md
banyan links "My First Note"   # แสดง forward links และ backlinks
banyan search "some query"     # ค้นหาแบบ full-text
banyan graph                   # แสดงทุก edge ของ link graph (from -> to)
banyan list                    # แสดงรายชื่อโน้ตทั้งหมด
```

## Development

```sh
cargo test
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

## License

[AGPL-3.0-only](LICENSE)
