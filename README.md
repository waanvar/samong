# Banyan 🌳

โน้ต = ไฟล์ Markdown ธรรมดา เข้ากันได้กับ [Obsidian](https://obsidian.md) (`[[wikilink]]` และ
`[[wikilink|alias]]`) — Banyan เพิ่ม link graph ที่เร็ว ([redb](https://github.com/cberner/redb))
และ full-text search ([tantivy](https://github.com/quickwit-oss/tantivy)) ให้กับ vault ของคุณ
โดยไฟล์ `.md` ยังคงเป็น source of truth เพียงแหล่งเดียวเสมอ — index ทั้งหมดอยู่ใน `<vault>/.brain/`
และสร้างใหม่ได้ทุกเมื่อด้วย `banyan reindex`

> **สถานะ**: Phase 2 (ดู `PLAN.md`) — multi-vault + cross-vault links แล้ว
> ยังไม่มี Thai tokenizer/API/Web UI

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

### Multi-vault

ลงทะเบียน vault ไว้ที่ registry กลาง (`~/.config/banyan/`) แล้วลิงก์ข้าม vault ได้ด้วย
`[[ชื่อ-vault/ชื่อโน้ต]]` — ลิงก์ภายใน vault เดิมยังเป็น `[[โน้ต]]` ตามปกติ
(เข้ากันได้กับ Obsidian เหมือนเดิม)

```sh
banyan vault add work ~/vaults/work    # ลงทะเบียน + index ให้ทันที
banyan vault list
banyan vault remove work               # เอาออกจาก registry (ไฟล์ไม่ถูกแตะ)

banyan links "Note" --all-vaults       # รวม backlinks จากทุก vault ที่ลงทะเบียน
banyan graph --all-vaults              # รวม graph ทุก vault (node เป็น vault/note)
banyan search --vault work "คำค้น"     # ค้นเฉพาะ vault ที่ระบุ
banyan search --all-vaults "คำค้น"     # ค้นทุก vault
```

## Development

```sh
cargo test
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

## License

[AGPL-3.0-only](LICENSE)
