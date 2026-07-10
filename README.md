# Banyan 🌳

โน้ต = ไฟล์ Markdown ธรรมดา เข้ากันได้กับ [Obsidian](https://obsidian.md) (`[[wikilink]]` และ
`[[wikilink|alias]]`) — Banyan เพิ่ม link graph ที่เร็ว ([redb](https://github.com/cberner/redb))
และ full-text search ([tantivy](https://github.com/quickwit-oss/tantivy)) ให้กับ vault ของคุณ
โดยไฟล์ `.md` ยังคงเป็น source of truth เพียงแหล่งเดียวเสมอ — index ทั้งหมดอยู่ใน `<vault>/.brain/`
และสร้างใหม่ได้ทุกเมื่อด้วย `banyan reindex`

> **สถานะ**: Phase 5 (ดู `PLAN.md`) — มี Web UI แล้ว 🎉
> เหลือ Phase 6: polish & release

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

### Thai full-text search 🇹🇭

จุดที่ Obsidian ทำไม่ได้: Banyan ตัดคำไทยด้วยพจนานุกรม newmm
([nlpo3](https://github.com/PyThaiNLP/nlpo3)) ทำให้**ค้นคำไทยที่อยู่กลางประโยค
โดยไม่มีวรรคคั่นเจอ**

```sh
# โน้ตมีข้อความ "ตลาดหลักทรัพย์แห่งประเทศไทยเปิดทำการวันนี้"
banyan search "ตลาดหลักทรัพย์"   # เจอ พร้อมไฮไลต์ตำแหน่งคำ
banyan search "ประเทศไทย"        # เจอเช่นกัน
```

- ข้อความไทยปนอังกฤษในโน้ตเดียวกันค้นได้ทั้งสองภาษา
- index เก่าจะถูก rebuild อัตโนมัติเมื่อ schema/tokenizer เปลี่ยน
  (เก็บ index version ไว้ใน redb)
- ข้อจำกัด: คำทับศัพท์ใหม่ๆ ที่ไม่อยู่ในพจนานุกรม (เช่น "แอป") อาจถูกตัดคำ
  ไม่ตรงกับที่คาด — พจนานุกรมคำศัพท์ผู้ใช้จะตามมาภายหลัง
- พจนานุกรม: `words_th.txt` จาก [PyThaiNLP](https://github.com/PyThaiNLP/pythainlp)
  (Apache-2.0) ฝังมาในไบนารี ใช้งาน offline ได้

### Local API server

`banyan-server` เปิด REST + WebSocket API บนเครื่อง (bind `127.0.0.1` เท่านั้น
local-first ไม่มี auth) สำหรับ Web UI ใน Phase 5 หรือเครื่องมืออื่น

```sh
banyan-server --port 3000
```

| Endpoint | หน้าที่ |
|---|---|
| `GET /api/vaults` | รายชื่อ vault ที่ลงทะเบียน |
| `GET /api/vaults/{vault}/notes` | รายชื่อโน้ตใน vault |
| `GET /api/notes/{vault}/{title}` | อ่านเนื้อหา markdown |
| `PUT /api/notes/{vault}/{title}` | เขียน/สร้างโน้ต (body = markdown) |
| `DELETE /api/notes/{vault}/{title}` | ลบโน้ต + รายงาน backlinks ที่ค้าง |
| `GET /api/notes/{vault}/{title}/links` | forward + backlinks + cross-vault |
| `GET /api/search?q=&vault=` | ค้นหา (ละ `vault` = ค้นทุก vault) |
| `GET /api/graph?vault=` | nodes + edges เป็น JSON |
| `WS /ws` | event เมื่อไฟล์ .md เปลี่ยน (จาก watcher) |

server เฝ้าไฟล์ทุก vault อัตโนมัติ — แก้ .md ด้วยโปรแกรมอื่น (เช่น Obsidian)
index จะอัปเดตเองและ ws clients ได้รับแจ้ง

### Web UI

หน้าเว็บดีไซน์ต้นฉบับ (ธีม "ต้นไทร" — เขียวเรือนยอด + wikilink สีน้ำตาลราก)
รองรับภาษาไทยเป็น first-class ด้วยฟอนต์ IBM Plex Sans Thai ฝังในตัว

```sh
cd web && npm install && npm run build   # ครั้งแรกครั้งเดียว
banyan-server                            # เสิร์ฟ UI ที่ http://127.0.0.1:3000
```

- Layout 3 ส่วน: รายชื่อโน้ต / editor แบบ เขียน–คู่กัน–อ่าน / backlinks + โครงร่าง
- พิมพ์ `[[` แล้ว autocomplete ชื่อโน้ต (รวมข้าม vault) — คลิก wikilink เพื่อกระโดด
  ถ้าโน้ตยังไม่มีจะสร้างให้เลย
- `Ctrl+K` — command palette: เปิดโน้ต, ค้นหา full-text ภาษาไทย, สร้างโน้ตใหม่
- Graph view (d3-force): คลิก node เปิดโน้ต, โหมดรวมทุก vault แยกสีตาม vault
- ธีมมืด/สว่าง, บันทึกอัตโนมัติ, real-time ผ่าน WebSocket
  (แก้ไฟล์จากที่อื่นแล้วหน้าจออัปเดตเอง)

สำหรับพัฒนา UI: `cd web && npm run dev` (Vite proxy ไปที่ banyan-server พอร์ต 3000)

## Development

```sh
cargo test
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

## License

[AGPL-3.0-only](LICENSE)
