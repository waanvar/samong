# Banyan 🌳

[![CI](https://github.com/waanvar/banyan/actions/workflows/ci.yml/badge.svg)](https://github.com/waanvar/banyan/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

**มันสมองที่สองแบบ local-first เขียนด้วย Rust — ค้นหาภาษาไทยแบบตัดคำได้จริง**

โน้ตของคุณคือไฟล์ Markdown ธรรมดา เข้ากันได้กับ [Obsidian](https://obsidian.md)
(`[[wikilink]]` / `[[wikilink|alias]]`) — Banyan เพิ่ม link graph ที่เร็ว,
full-text search ที่**ตัดคำไทยได้**, ลิงก์ข้าม vault, API และ Web UI
โดยไฟล์ `.md` เป็น source of truth เพียงแหล่งเดียวเสมอ

*[English version →](README.en.md)*

![Banyan editor](docs/editor-dark.png)

## ทำไมต้อง Banyan

- 🇹🇭 **ค้นคำไทยกลางประโยคเจอ** — "ตลาดหลักทรัพย์แห่งประเทศไทยเปิดทำการ"
  ค้นด้วย "ตลาดหลักทรัพย์" หรือ "ประเทศไทย" เจอทันทีพร้อมไฮไลต์
  (ตัดคำด้วยพจนานุกรม newmm ผ่าน [nlpo3](https://github.com/PyThaiNLP/nlpo3) —
  สิ่งที่ Obsidian ทำไม่ได้)
- 📁 **ไฟล์ของคุณ เครื่องของคุณ** — โน้ต = Markdown ธรรมดา ไม่มี lock-in
  index ทั้งหมดอยู่ใน `<vault>/.brain/` และสร้างใหม่ได้เสมอด้วย `banyan reindex`
- 🔗 **Multi-vault** — ลิงก์ข้ามโปรเจกต์ด้วย `[[ชื่อ-vault/ชื่อโน้ต]]`
  backlinks ข้าม vault แสดงครบ
- ⚡ **เร็ว** — link graph ใน [redb](https://github.com/cberner/redb),
  ค้นหาด้วย [tantivy](https://github.com/quickwit-oss/tantivy),
  incremental reindex แตะเฉพาะไฟล์ที่เปลี่ยน
- 🤖 **เป็นมันสมองของ AI agent ได้** — `banyan-mcp` เสียบเข้า Claude Code /
  Claude Desktop ผ่าน MCP ให้ agent ค้น-อ่าน-บันทึกความรู้เองได้
  ([วิธีตั้งค่า](docs/AI-AGENT.md))

## ติดตั้ง

### ความต้องการของระบบ

ตอนนี้ Banyan ติดตั้งด้วยการ **build จาก source** (ยังไม่ได้เปิด public / ยังไม่มี
release binary สำเร็จรูป) จึงต้องมีเครื่องมือของนักพัฒนา:

- [**Rust**](https://rustup.rs) (stable) — **จำเป็น** ใช้ build ตัวโปรแกรม
- [**Node.js**](https://nodejs.org) 20+ — **จำเป็นถ้าต้องการหน้าเว็บ** เพราะ UI ถูก
  ฝังลงในไบนารีตอน build ถ้าไม่ลง Node จะได้เฉพาะ CLI + API
  (`banyan-server` จะเสิร์ฟ API อย่างเดียว)

> **ในอนาคตเมื่อเปิด public**: จะมี release binary ให้โหลด — end user แค่แตกไฟล์แล้ว
> รัน `banyan-server start` ได้เลย **ไม่ต้องลง Rust หรือ Node** (release.yml build
> ไบนารีทั้ง 3 OS ให้อัตโนมัติเมื่อ tag เวอร์ชัน)

### build จาก source

```sh
git clone https://github.com/waanvar/banyan.git
cd banyan
cd web && npm install && npm run build   # build Web UI ก่อน (จะถูกฝังในไบนารี)
cd .. && cargo install --path .          # ติดตั้ง banyan / banyan-server / banyan-mcp ลงเครื่อง
```

> **ลำดับสำคัญ**: build Web UI ก่อน `cargo build`/`cargo install` เสมอ เพราะ
> `banyan-server` จะ**ฝังหน้าเว็บไว้ในตัวไบนารี** ทำให้แจกไฟล์เดียวจบ
> ไม่ต้องมีโฟลเดอร์ UI ข้างๆ (ถ้าอยากลอง build เฉยๆ ไม่ติดตั้ง ใช้ `cargo build --release`
> ได้ไบนารีใน `target/release/`)

อัปเดตเป็นเวอร์ชันล่าสุดภายหลังด้วย `banyan update` (ดูหัวข้อ *อัปเดตเวอร์ชัน* ด้านล่าง)

## เริ่มใช้งาน

```sh
mkdir my-vault && cd my-vault
banyan new "โน้ตแรกของฉัน"        # สร้างโน้ต + index อัตโนมัติ
banyan vault add my-vault .        # ลงทะเบียนเข้า registry (~/.config/banyan)
banyan-server start               # เปิดเบราว์เซอร์ที่ http://127.0.0.1:3000 ให้อัตโนมัติ
```

`banyan-server start` เสิร์ฟหน้าเว็บที่ฝังในตัว แล้วเปิดเบราว์เซอร์ให้เอง —
ไม่ต้องมีไฟล์ UI ข้างๆ ปรับพอร์ตด้วย `--port 8080`, ไม่ให้เปิดเบราว์เซอร์ด้วย
`--no-open` (รูปแบบเดิม `banyan-server --port 8080` ยังใช้ได้)

![Graph view](docs/graph-dark.png)

## คำสั่ง CLI

| คำสั่ง | หน้าที่ |
|---|---|
| `banyan new <ชื่อ>` | สร้างโน้ตใหม่ + index |
| `banyan edit <ชื่อ>` | เปิดใน `$EDITOR` แล้ว reindex เมื่อปิด |
| `banyan rename <เก่า> <ใหม่>` | เปลี่ยนชื่อ + แก้ `[[wikilink]]` ทุกโน้ตที่ลิงก์มา |
| `banyan delete <ชื่อ>` | ลบโน้ต + เตือน backlinks ที่จะค้าง |
| `banyan links <ชื่อ> [--all-vaults]` | forward links + backlinks (รวมข้าม vault) |
| `banyan orphans` / `banyan broken` | โน้ตที่ไม่มีใครลิงก์ / ลิงก์ที่ชี้ไปโน้ตที่ไม่มี |
| `banyan search <คำ> [--vault <ชื่อ>\|--all-vaults]` | ค้นหา full-text (ไทย/อังกฤษ) |
| `banyan graph [--all-vaults]` | edges ของ link graph |
| `banyan list` | รายชื่อโน้ตทั้งหมด |
| `banyan reindex [--full]` | sync index (เฉพาะไฟล์ที่เปลี่ยน / ทั้งหมด) |
| `banyan watch` | เฝ้า vault แล้วอัปเดต index อัตโนมัติ |
| `banyan vault add/list/remove` | จัดการ registry กลาง |
| `banyan update [--check]` | อัปเดตเป็นเวอร์ชันล่าสุดจาก GitHub release (--check = เช็คเฉยๆ) |

### อัปเดตเวอร์ชัน

`banyan update` ดาวน์โหลด release ล่าสุดจาก GitHub แล้วแทนที่ไบนารีทั้งสาม
(banyan / banyan-server / banyan-mcp) ให้อัตโนมัติ — รวมหน้าเว็บที่ฝังในตัวด้วย
`banyan update --check` เช็คว่ามีเวอร์ชันใหม่ไหมโดยไม่ติดตั้ง และ `banyan-server start`
จะแจ้งบรรทัดเดียวถ้ามีเวอร์ชันใหม่ (best-effort ไม่บล็อก ไม่ล้มถ้าออฟไลน์)

> ต้องมี release เผยแพร่บน GitHub ก่อน (`git tag v0.1.0 && git push origin v0.1.0`
> ให้ workflow build binary ทั้ง 3 OS) `banyan update` ถึงจะหา release เจอ

## Web UI

ดีไซน์ต้นฉบับธีม "ต้นไทร" (ไม่ลอก Obsidian) — typography ไทยด้วย
IBM Plex Sans Thai ฝังในตัว ใช้ offline ได้ และหน้าเว็บทั้งชุดถูก**ฝังในไบนารี**
`banyan-server` (rust-embed) แจกไฟล์เดียวเปิดใช้ได้ทันที

- Layout 3 ส่วน: รายชื่อโน้ต / editor (เขียน–คู่กัน–อ่าน) / backlinks + โครงร่าง
- พิมพ์ `[[` แล้ว autocomplete ชื่อโน้ตรวมข้าม vault — คลิก wikilink เพื่อกระโดด
  ถ้าโน้ตยังไม่มีจะสร้างให้
- `Ctrl+K` command palette: เปิดโน้ต / ค้น full-text / สร้างโน้ตใหม่
- Graph view (d3-force) คลิก node เปิดโน้ต โหมดรวมทุก vault แยกสีตาม vault
- ธีมมืด/สว่าง, บันทึกอัตโนมัติ, real-time ผ่าน WebSocket —
  แก้ไฟล์จาก Obsidian หรือ editor อื่นแล้วหน้าจออัปเดตเอง

พัฒนา UI: `cd web && npm run dev` (Vite proxy ไป banyan-server พอร์ต 3000)

## API (banyan-server)

Bind เฉพาะ `127.0.0.1` เท่านั้น (local-first ไม่มี auth)

| Endpoint | หน้าที่ |
|---|---|
| `GET /api/vaults` | รายชื่อ vault ที่ลงทะเบียน |
| `GET /api/vaults/{vault}/notes` | รายชื่อโน้ตใน vault |
| `GET/PUT/DELETE /api/notes/{vault}/{title}` | อ่าน / เขียน / ลบ markdown |
| `GET /api/notes/{vault}/{title}/links` | forward + backlinks + cross-vault |
| `GET /api/search?q=&vault=` | ค้นหา (ละ `vault` = ทุก vault) |
| `GET /api/graph?vault=` | nodes + edges เป็น JSON |
| `WS /ws` | event เมื่อไฟล์ .md เปลี่ยน |

## AI agent (banyan-mcp)

`banyan-mcp` เป็น MCP server บน stdio — agent ได้ tools: `search_notes`
(ค้นไทยตัดคำ), `read_note`, `save_note`, `get_links`, `list_notes`,
`list_vaults` (จงใจไม่มี delete — การลบเป็นเรื่องของมนุษย์)

```json
// .mcp.json ใน repo ของคุณ
{ "mcpServers": { "banyan": { "command": "banyan-mcp" } } }
```

ดูวิธีตั้งค่าเต็มและ recipe สำหรับ `CLAUDE.md` ที่ [docs/AI-AGENT.md](docs/AI-AGENT.md)

## สถาปัตยกรรม

```
<vault>/
  *.md            ← source of truth (Obsidian-compatible)
  .brain/
    graph.redb    ← forward/backlinks + mtimes + index version (redb)
    tantivy/      ← full-text index, tokenizer ไทย newmm (tantivy)
~/.config/banyan/
  registry.redb   ← ชื่อ vault -> path สำหรับลิงก์ข้าม vault
```

ลบ `.brain/` ทิ้งเมื่อไหร่ก็ได้ — `banyan reindex` สร้างใหม่จากไฟล์ .md ทั้งหมด
เมื่อ schema/tokenizer เปลี่ยนเวอร์ชัน index เก่าจะถูก rebuild อัตโนมัติ

## Development

```sh
cargo test                              # unit + integration tests
cargo clippy --all --all-targets -- -D warnings
cargo fmt --all -- --check
```

> หมายเหตุ: ต้อง `cd web && npm run build` ก่อน `cargo test` ครั้งแรก เพื่อให้
> เทสต์ที่เกี่ยวกับ UI ที่ฝังในไบนารีทำงานครบ (ถ้าไม่ build เทสต์จะข้ามส่วน UI ให้เอง)

## แผนต่อไป

- เปิด public + เผยแพร่ release binary (โหลดไปรันได้โดยไม่ต้องมี Rust/Node)
- พจนานุกรมคำศัพท์ผู้ใช้ (คำทับศัพท์ใหม่ๆ ที่ newmm ไม่รู้จัก)
- ปุ่มเพิ่ม vault ในหน้าเว็บ (ไม่ต้องใช้ terminal), แพ็กเป็น desktop app ด้วย Tauri
- Sync ข้ามเครื่อง / AI features (สรุปโน้ต, ถามตอบกับ vault) — เป็น open-core layer ภายหลัง

## License

[AGPL-3.0-only](LICENSE) — ใช้ฟรี แก้ได้ แต่ถ้านำไปให้บริการต้องเปิดซอร์สส่วนที่แก้

พจนานุกรมตัดคำ `words_th.txt` จาก [PyThaiNLP](https://github.com/PyThaiNLP/pythainlp)
(Apache-2.0)
