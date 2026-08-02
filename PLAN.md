# Engineering log — Samong 🧠

The record of how Samong was built: what each phase changed, what broke, and why
each decision went the way it did. Written in Thai, because it is a working log
kept as the work happened rather than documentation written afterwards — the
mistakes are left in on purpose, since a decision without its reason is folklore.

Product documentation is in [README.md](README.md); the release-by-release summary
is in [CHANGELOG.md](CHANGELOG.md).

## สถาปัตยกรรมที่ตัดสินใจแล้ว

- โน้ต = ไฟล์ Markdown ธรรมดาใน vault เข้ากันได้กับ Obsidian
  (`[[wikilink]]` และ `[[wikilink|alias]]`)
- Link graph (forward + backlinks) = **redb**
- Full-text search = **tantivy**
- Index ทั้งหมดอยู่ใน `<vault>/.brain/` และต้อง**สร้างใหม่ได้เสมอ**จากไฟล์ `.md`
  ซึ่งเป็น source of truth เพียงแหล่งเดียว
- โน้ตถูกอ้างด้วย **path** ไม่ใช่ title (ดู Phase 11 และ 14 ว่าทำไม)

## วิธีทำงานในรีโปนี้

1. ทำทีละ Phase จบ Phase ต้องผ่าน acceptance criteria ครบก่อนไปต่อ
2. ทุก Phase ต้องมีเทสต์ประกอบ และ `cargo test` ต้องเขียว
3. `cargo clippy -- -D warnings` และ `cargo fmt --check` ต้องผ่านก่อน commit
4. commit เล็กๆ แยกตาม feature
5. การตัดสินใจเชิงสถาปัตยกรรมที่ไม่อยู่ในแผน ให้หยุดถามก่อน ไม่เดา

---

## Phase 0 — Setup & Hardening พื้นฐาน

- [x] Rename โปรเจกต์: โฟลเดอร์ `secondbrain/` → `samong/`,
      แก้ `name = "secondbrain"` → `name = "samong"` ใน Cargo.toml,
      อัปเดต README และตัวอย่างคำสั่งทั้งหมดให้ใช้ `samong`
- [x] เพิ่มไฟล์ `LICENSE` (เดิม AGPL-3.0 → เปลี่ยนเป็น **Apache-2.0** ปี 2026)
      พร้อม `NOTICE` + `THIRD-PARTY.md` และ header `license` ใน Cargo.toml
- [x] อัป toolchain ตามหมายเหตุด้านบน (ถ้า Rust >= 1.81)
- [x] แยก `main.rs` เป็นโมดูล: `vault.rs`, `graph.rs` (redb), `search.rs` (tantivy), `cli.rs`
- [x] เพิ่ม integration test: สร้าง vault ชั่วคราว → new → links → search → ตรวจผลลัพธ์
- [x] ตั้ง GitHub Actions: build + test + clippy + fmt

**Acceptance**: `cargo test` เขียว, CI เขียว, มี LICENSE

## Phase 1 — Core commands ให้ครบวงจรชีวิตโน้ต

- [x] `edit <title>` — เปิดโน้ตใน `$EDITOR` แล้ว reindex เฉพาะไฟล์นั้น
- [x] `delete <title>` — ลบไฟล์ + ลบออกจาก index + รายงาน backlinks ที่จะ dangling
- [x] `rename <old> <new>` — เปลี่ยนชื่อไฟล์ และ **แก้ [[wikilink]] ในทุกโน้ตที่ลิงก์มา**
- [x] `orphans` — แสดงโน้ตที่ไม่มีใครลิงก์หา / `broken` — แสดงลิงก์ที่ชี้ไปโน้ตที่ไม่มีจริง
- [x] **Incremental reindex**: เก็บ mtime ต่อไฟล์ใน redb แล้ว reindex เฉพาะไฟล์ที่เปลี่ยน
- [x] `watch` — ใช้ crate `notify` เฝ้า vault แล้วอัปเดต index อัตโนมัติ

**Acceptance**: rename แล้วลิงก์ทุกจุดถูกแก้ตาม (มี test), reindex vault 1,000 โน้ตที่แก้ 1 ไฟล์ ต้องเร็วกว่า full reindex ชัดเจน

## Phase 2 — Multi-vault + cross-project links

- [x] Registry กลางที่ `~/.config/samong/registry.redb` เก็บรายชื่อ vault ทั้งหมด
- [x] คำสั่ง `vault add <name> <path>` / `vault list` / `vault remove <name>`
- [x] Cross-vault link ด้วย syntax `[[vault-name/note-title]]`
      (ลิงก์ภายใน vault เดิมยังใช้ `[[note]]` ตามปกติ — ห้าม break Obsidian compat)
- [x] `links` และ `graph` รองรับโหมด `--all-vaults`
- [x] search ระบุ vault ได้: `search --vault live-commerce "คำค้น"` หรือค้นทุก vault

**Acceptance**: สร้าง 2 vault ลิงก์ข้ามกัน แล้ว backlinks ข้าม vault แสดงถูกต้อง (มี test)

## Phase 3 — Full-text search ที่ตัดคำได้

- [x] เพิ่ม custom tokenizer สำหรับ tantivy ที่ตัดคำไทยด้วย **nlpo3** (newmm dictionary)
      โดยตรวจอักขระ: ช่วงที่เป็นไทยตัดด้วย newmm, ช่วงอื่นใช้ tokenizer ปกติ
- [x] reindex อัตโนมัติเมื่อ schema/tokenizer เปลี่ยน (เก็บ index version ใน redb)
- [x] test ภาษาไทยจริง เช่น โน้ตมีคำว่า "ตลาดหลักทรัพย์แห่งประเทศไทย"
      ต้องค้นเจอด้วยคำว่า "ตลาดหลักทรัพย์" และ "ประเทศไทย"

**Acceptance**: ค้นคำไทยที่อยู่กลางประโยค (ไม่มีวรรคคั่น) เจอถูกต้อง — นี่คือสิ่งที่ Obsidian ทำไม่ได้

## Phase 4 — Local API server

- [x] เพิ่ม binary ที่สอง `samong-server` ใช้ **axum**
- [x] REST endpoints:
  - `GET /api/vaults` , `GET /api/vaults/:vault/notes`
  - `GET/PUT/DELETE /api/notes/:vault/:title` (อ่าน/เขียน/ลบเนื้อหา markdown)
  - `GET /api/notes/:vault/:title/links` (forward + backlinks)
  - `GET /api/search?q=&vault=`
  - `GET /api/graph?vault=` (nodes + edges เป็น JSON)
- [x] WebSocket `/ws` แจ้งเตือนเมื่อไฟล์เปลี่ยน (ต่อจาก `watch` ใน Phase 1)
- [x] bind เฉพาะ `127.0.0.1` เท่านั้น (local-first, ยังไม่ทำ auth ใน phase นี้)

**Acceptance**: integration test ยิงทุก endpoint ผ่าน, แก้ไฟล์ .md ตรงๆ แล้ว WebSocket แจ้ง event

## Phase 5 — Web UI (ขั้นตอนสุดท้าย ทำหลังทุกอย่างสมบูรณ์)

> เป้าหมาย: หน้าเว็บที่ *ได้แรงบันดาลใจจาก* Obsidian แต่เป็นดีไซน์ต้นฉบับของเราเอง
> สวย ทันสมัย และรองรับภาษาไทยเป็น first-class
> **สำคัญ: ให้ Claude Code ใช้ skill `frontend-design` ก่อนเริ่มออกแบบทุกครั้ง**
> เพื่อให้ได้ดีไซน์ระดับ expert ไม่ใช่หน้าตา template ทั่วไป
> ห้ามลอก asset, ไอคอน, สี, หรือ layout เฉพาะตัวของ Obsidian — inspired only

- [x] Stack: SPA (แนะนำ React + Vite หรือ SolidJS — เลือกแล้วแจ้งเหตุผล)
      เสิร์ฟผ่าน axum จาก Phase 4, ธีมมืด/สว่าง
- [x] Layout 3 ส่วน: file tree (ซ้าย) / editor (กลาง) / panel ขวา (backlinks, outline)
- [x] Markdown editor แบบ live preview + autocomplete ตอนพิมพ์ `[[`
- [x] **Graph view** แบบ force-directed (d3) — คลิก node เพื่อเปิดโน้ต,
      โหมดแยกสีตาม vault สำหรับ cross-project view
- [x] Command palette (Cmd/Ctrl+K): เปิดโน้ต, ค้นหา, สร้างโน้ตใหม่
- [x] Typography ภาษาไทยต้องสวย: เลือกฟอนต์ไทยที่อ่านสบาย (เช่น IBM Plex Sans Thai,
      Noto Sans Thai) line-height เหมาะกับสระบน-ล่าง ทดสอบกับเนื้อหาไทยจริง
- [x] Real-time: ฟัง WebSocket แล้วรีเฟรช tree/backlinks อัตโนมัติ

**Acceptance**: เปิดเบราว์เซอร์ใช้แทน CLI ได้ครบทุก workflow หลัก
(สร้าง/แก้/ลิงก์/ค้น/ดูกราฟ), ดีไซน์ผ่านมาตรฐาน frontend-design skill,
เนื้อหาไทยแสดงผลและค้นหาได้สมบูรณ์

## Phase 6 — Polish & Release

- [x] README ฉบับเต็ม (ไทย + อังกฤษ) พร้อม screenshot
- [x] `cargo install` ได้จาก git, release binary ผ่าน GitHub Actions
- [ ] (ทางเลือก) ห่อด้วย Tauri เป็น desktop app ถ้าใช้ web UI แล้วอยากได้แอปแยก

---

## Phase 7 — MCP server สำหรับ AI agent (เพิ่มหลังจบแผนเดิม)

> เป้าหมายจริงของโปรเจกต์: เป็นมันสมองเก็บความรู้การเขียนโค้ดให้ AI agent ใช้
> MCP คือสะพานมาตรฐานที่ทำให้ agent เห็น Samong เป็นเครื่องมือ native

- [x] binary ที่สาม `samong-mcp` — MCP (JSON-RPC 2.0) บน stdio เขียนโปรโตคอลเองแบบ minimal
      (initialize / tools/list / tools/call / ping) ไม่พึ่ง SDK
- [x] tools 6 ตัว: list_vaults, list_notes, read_note, save_note, search_notes, get_links
      (จงใจไม่มี delete — agent ไม่ควรลบความรู้เองได้)
- [x] แยก helper ซ้ำจาก cli/server เป็น ops.rs (validate_title, cross_vault_backlinks)
- [x] docs/AI-AGENT.md: วิธีตั้ง .mcp.json + recipe วงจรความรู้ใน CLAUDE.md

**Acceptance**: integration test spawn ไบนารีจริง คุย JSON-RPC ครบทุก tool
รวมค้นไทยกลางประโยค, path traversal ต้องถูกปฏิเสธ, save แล้วไฟล์เกิดจริง

## Phase 8 — Distribution ให้ผู้ใช้ทั่วไปเปิดง่าย (เพิ่มหลัง Phase 7)

> ปัญหา: end user ต้องพิมพ์ `samong-server --ui <path>` เอง ซึ่งเทคนิคเกินไป
> เป้าหมาย: แตก zip หรือ cargo install แล้วสั่งทีเดียวหน้าเว็บเด้งเอง

- [x] `samong-server start [--port] [--ui] [--no-open]` — subcommand ใหม่
      (รูปแบบเดิม `samong-server` / `samong-server --port` ยังใช้ได้ backward compat)
- [x] ฝัง Web UI ทั้งชุดใน binary ด้วย rust-embed — cargo install แล้วได้ UI เลย
      ไม่ต้องมีโฟลเดอร์ web/dist ข้างๆ (`--ui` ยัง override เป็น filesystem ได้ตอน dev)
- [x] เปิดเบราว์เซอร์อัตโนมัติเมื่อ server พร้อม (ปิดด้วย --no-open)
- [x] release archive เล็กลง (ไม่ต้องแนบ web/dist), release.yml build web ก่อน cargo

**Acceptance**: รัน binary จากโฟลเดอร์ที่ไม่มีไฟล์ UI ข้างๆ แล้ว GET / ได้หน้าเว็บ,
SPA fallback route ได้ 200, /api ยังทำงาน; unit test CLI parsing + embedded serving

## Phase 9 — Version-check + self-update (เพิ่มหลัง Phase 8)

> ปัญหา: มีเวอร์ชันใหม่บน git แล้ว end user ต้องอัปเดตเอง manual
> เป้าหมาย: `samong update` คำสั่งเดียวดึง release ใหม่มาแทนที่ + แจ้งเตือนเมื่อมีใหม่

- [x] `samong update [--check]` — ใช้ crate self_update ดึง release ล่าสุดจาก
      GitHub (waanvar/samong) แทนที่ไบนารีทั้ง 3 (samong/samong-server/samong-mcp)
      รวม UI ที่ฝังในตัว; --check เช็คเฉยๆ ไม่ติดตั้ง
- [x] map platform → asset target ให้ตรง release.yml (x86_64-windows ฯลฯ)
- [x] `samong-server start` แจ้งเตือน 1 บรรทัดถ้ามีเวอร์ชันใหม่ (off-thread
      best-effort ไม่บล็อก ไม่ล้มถ้าออฟไลน์/ไม่มี release)
- [x] จัดการ error นุ่มนวล: ออฟไลน์/repo private/ยังไม่มี release ไม่ทำให้ command ล้ม
- [x] ยังไม่มี release: data migration รองรับอยู่แล้ว (INDEX_VERSION → rebuild
      อัตโนมัติ) จาก Phase 3 — อัปเดตแล้ว index เก่าไม่พัง

**Acceptance**: unit test เทียบ semver (is_newer) + asset target ตรง release naming;
`samong update --check` รันได้จริง จัดการกรณีไม่มี release อย่างเป็นมิตร

## Phase 10 — Vault scope (เพิ่มหลัง Phase 9)

> ปัญหา (รายงานจากโปรเจกต์จริงที่นำ samong ไปใช้): `vault add` ชี้ที่ root ของ repo
> แล้ว index ดูด `node_modules` เข้ามาหมด — README ซ้ำ 300+ ครั้ง CHANGELOG 150+
> ครั้ง ล้วนเป็นไฟล์ของ dependency ส่วนโน้ตจริงของโปรเจกต์มีแค่ 3 ไฟล์
> เป้าหมาย: **โน้ต = ไฟล์ .md ที่คุณจะ commit** โดยผู้ใช้ไม่ต้องตั้งอะไรเลย

- [x] `src/scope.rs` — เลิกใช้ walkdir เปลี่ยนไป crate `ignore` (ตัวเดียวกับ ripgrep):
      respect `.gitignore`, ข้าม dot-dir, deny-list dependency dir ที่ไม่มีทางเป็นโน้ต
      (`node_modules`, `vendor`, `site-packages`, `__pycache__`, `Pods`,
      `bower_components`) — build output ไม่ใส่ใน deny-list เพราะ gitignore
      จัดการอยู่แล้วและชื่อพวกนั้นอาจเป็นโฟลเดอร์โน้ตจริง
- [x] **determinism ข้ามเครื่อง** (ข้อบังคับสำหรับ server กลาง): scope ตัดสินจาก
      ไฟล์ที่ commit อยู่ใน vault เท่านั้น — ปิด global gitignore, `.git/info/exclude`,
      `.ignore`/`.rgignore` และ `.gitignore` ของ parent dir เหนือ vault ทั้งหมด
      (`require_git(false)` ให้ `.gitignore` มีผลแม้ vault ไม่ใช่ git repo)
- [x] `samong.toml` ที่ vault root (commit ไปกับ repo = ingestion contract):
      `[vault] name`, `[scope] notes_dir / exclude / follow_gitignore / max_depth`
      — ทุกฟิลด์ optional, ไม่มีไฟล์ = default ที่ถูกต้องอยู่แล้ว
- [x] `deny_unknown_fields` — พิมพ์ผิด (`excludes` แทน `exclude`) ต้อง error เสียงดัง
      ไม่ใช่เงียบแล้วกลับไป index ทั้ง repo
- [x] `.samongignore` (gitignore syntax + `!` negation) — ทางออกสำหรับ repo ที่
      gitignore โน้ตของตัวเอง เช่น `!notes/`
- [x] `vault add` / `reindex` **รายงานไม่ถาม**: บอกจำนวนโน้ตใน scope + จำนวนที่ข้าม
      แยกตาม top-level dir (ชี้ repo root เป็นเรื่องปกติ ไม่ต้องขออนุญาต)
      เตือนเฉพาะกรณีวิปริต (0 โน้ต)
- [x] `samong doctor` — สรุป scope, สิ่งที่ข้าม, และ title ที่กำกวม
- [x] `watch` ใช้ matcher ตัวเดียวกัน ทั้งกรอง event และ**เลือก dir ที่จะ watch**
      (ไม่ watch `node_modules`: บน Linux ก้อนเดียวกิน `max_user_watches` หมด
      ทำให้ watch mode ล่มทั้งตัว, และ `npm install` ปลุก indexer เป็นพันครั้ง)

**Acceptance**: unit test ครบทุกกฎ (dependency dir, gitignore, `.samongignore`
negation, per-machine source ต้องไม่มีผล, parent gitignore ต้องไม่มีผล, typo ต้อง
error); integration test `tests/phase10.rs` จำลอง repo แบบ JS จริงแล้วยืนยันว่า
index เฉพาะโน้ตของโปรเจกต์ — ทดสอบกับ repo ของ samong เองได้ 4 โน้ต ข้าม 90 ไฟล์

## Phase 11 — Note identity: path + content hash (เพิ่มหลัง Phase 10)

> ปัญหา: `title` = ชื่อไฟล์ ถูกใช้เป็น key ทั้งใน redb และ tantivy → `README.md`
> 300 ไฟล์กลายเป็น key เดียว: full rebuild ใส่ 300 doc ชื่อเดียวกัน, incremental
> เก็บ mtime ได้แค่ค่าเดียวต่อ title ทำให้ **reindex ไม่เคยนิ่ง** วนซ้ำทุกรอบ
> เป้าหมาย: identity ที่ unique และเสถียรข้ามเครื่อง — เตรียมทางให้ server กลาง

- [x] key = vault-relative path, slash-separated ทุก OS (`vault::relative_key`)
      — ตรงกับวิธีที่ git ตั้งชื่อไฟล์เดียวกัน ทำให้ ingest จาก commit ได้ตรงๆ
- [x] `graph.rs`: `FORWARD`/`BACKWARD` keyed by path, เพิ่ม `TITLES`
      (title → keys, one-to-many by design) สำหรับ resolve + ตรวจความกำกวม
- [x] `FILES` table เก็บ `(mtime, blake3 hash)` — **mtime ใช้เป็น identity ข้าม
      เครื่องไม่ได้** (git checkout / clone / copy เขียน mtime ใหม่ทั้งที่ byte
      ไม่เปลี่ยน) mtime เป็นแค่ pre-filter ราคาถูก, hash เป็นตัวตัดสิน
- [x] `search.rs`: field `path` เป็น key ของ `delete_term`, `title` เปลี่ยนเป็น
      tokenized (ค้นชื่อโน้ตไทยกลางคำได้ด้วย) + `SearchHit.key`
- [x] แยกชั้นชัดเจน: **ชั้นที่เก็บเป็น path-space, ชั้นที่แสดงเป็น title-space**
      (`ops::keys_to_titles`) — wikilink อ้างด้วย title ฉะนั้น node ของ graph view
      ต้องเป็น title ไม่งั้น edge จะชี้ไป node ที่ไม่มีอยู่
- [x] `rename` เขียนไฟล์ผ่าน key ตรงๆ ไม่ต้อง walk หา title ต่อ source
- [x] `samong search` แสดง path ต่อผลลัพธ์ (จุดที่ต้องแยก README สองไฟล์ให้ออก)
      และ `GET /api/search` เพิ่ม field `path`
- [x] INDEX_VERSION 2 → 3 (index เก่า rebuild เองอัตโนมัติ) + ลบ table `mtimes`
      ของเดิมทิ้งตอน rebuild ไม่ให้เหลือ dead data

**Acceptance**: unit test ยืนยัน `README.md` / `docs/README.md` / `api/README.md`
เป็น 3 โน้ตแยกกันทั้งใน graph และ search; reindex ครั้งที่สอง = 0 (นิ่ง);
เขียนไฟล์ด้วย byte เดิมแล้ว `indexed = 0` แต่ `untouched = 1`

### ต่อจากนี้ (ยังไม่ทำ — จดไว้กันลืม)

- Graph view resolve target → key เพื่อให้ node แยกไฟล์ที่ title ซ้ำได้จริง
  (ตอนนี้ยัง collapse ในการแสดงผล ซึ่งเท่ากับที่ Obsidian ทำ)
- API/MCP addressing ด้วย path (`read_note`/`save_note` ตอนนี้รับ title —
  ถ้า title ซ้ำจะได้ไฟล์แรกตาม key order ซึ่ง deterministic แต่ยังกำกวม)
- `find_note` ยังเดิน FS อยู่ (ถูกต้องและตอนนี้ถูกเพราะ scope แคบแล้ว) —
  เปลี่ยนเป็น redb lookup เมื่อ server กลางเริ่มรับ traffic หลายคน

## Phase 12 — Token budget ของ AI agent (เพิ่มหลัง Phase 11)

> ปัญหา: `search_notes` คืนได้ 20 hit **ต่อ vault** และไม่มีทางบอกให้คืนน้อยลง —
> agent ที่ค้นทุก vault (5 vault = ได้ถึง 100 snippet) ต้องจ่าย token ก้อนนั้น
> ซ้ำทุกเทิร์นที่เหลือของบทสนทนา และเนื้อหาไทยกิน token ต่อตัวอักษรสูงกว่าอังกฤษ
> เป้าหมาย: ให้ผู้เรียกคุมปริมาณ context ที่ได้กลับไปได้

- [x] `search::SearchOptions { limit, snippet_chars }` + `query_with()` —
      `query()` เดิมยังใช้ได้ด้วย default (20 / 150 ตัวอักษร)
- [x] `SearchHit.score` — ให้ผู้เรียกที่รวมผลจากหลาย vault จัดอันดับร่วมกันได้
      ไม่ใช่ต่อ vault แล้วเอามาต่อกันตามลำดับตัวอักษรของชื่อ vault
- [x] MCP `search_notes` รับ `limit` (default 8 — ต่ำกว่า CLI เพราะ agent
      มักอยากได้โน้ตใบที่ตอบคำถาม ไม่ใช่ลิสต์ให้ไล่อ่าน) และ **นับรวมทุก vault**
      ไม่ใช่ต่อ vault; clamp `1..=MAX_LIMIT`, ค่าที่ไม่ใช่ตัวเลข = ใช้ default
- [x] `samong search --limit N` และ `GET /api/search?limit=` ให้ทุก surface
      คุมได้เหมือนกัน
- [x] เขียน `limit` ไว้ใน tool schema ให้ชัดว่าเป็นยอดรวม — agent ใช้ได้เฉพาะ
      พารามิเตอร์ที่ schema บอกว่ามี

> หมายเหตุ: งาน Phase 10 ลด token ให้ agent ไปแล้วโดยไม่ต้องแตะโค้ด MCP —
> ก่อนแก้ 20 ช่องผลค้นหาเต็มไปด้วย README ของ dependency

**Acceptance**: unit test limit/clamp/snippet_chars/score; integration
`tests/phase12.rs` ตั้ง 3 vault × 12 โน้ตที่ match แล้วยืนยันว่า `limit` เป็นยอดรวม
(ไม่ใช่ 36), default = 8, ค่าเพี้ยนไม่ทำให้ tool ล้ม, และ schema ระบุคำว่า total

### ยังไม่ทำ (จดไว้)

- MCP search ยังไม่คืน `path` — agent จึงแยกโน้ตที่ title ซ้ำไม่ออก ต้องรอ
  addressing ด้วย path (อยู่ในรายการท้าย Phase 11)
- score จาก tantivy ต่าง index เทียบกันได้แบบหยาบๆ (BM25 คนละ corpus) —
  พอสำหรับจัดอันดับข้าม vault แต่ไม่ใช่ global ranking ที่ถูกต้องตามทฤษฎี

## Phase 13 — Reference notes: `scope.include` (เพิ่มหลัง Phase 12)

> ปัญหา: โปรเจกต์ต้องการเรียนรู้จาก docs ที่ Next ship มาใน
> `node_modules/next/dist/docs` (425 ไฟล์จริง) แต่แตะไม่ได้เลย — `node_modules`
> ถูก hard-code ใน `ALWAYS_EXCLUDE` และ `filter_entry` ตัดกิ่งทิ้ง**ก่อน**เดินเข้าไป
> จึงไม่มีรายการให้ `!node_modules/...` ใน `.samongignore` negate (คนละอาการกับ
> "ถูก override แพ้") ซ้ำร้าย gitignore เองก็ re-include ใต้ parent ที่ถูก exclude ไม่ได้
>
> **premise เดิมของ Phase 10 ผิด**: ผมเอา `.gitignore` มาตอบคำถามผิดข้อ —
> `.gitignore` ตอบว่า "จะแจกจ่ายอะไร" แต่ฐานความรู้ต้องตอบว่า "จะเรียนรู้จากอะไร"
> เราไม่ได้ commit ไฟล์ของ dependency เราแค่อ่านมันเพื่อเรียน
>
> เป้าหมาย: **หนึ่งโปรเจกต์ หนึ่งสมอง** ไม่แยก vault (แยกแล้ว backlink ข้ามไม่ติด,
> doctor ต้องดูสองที่, agent ต้องรู้ว่าความรู้อยู่ vault ไหน)

- [x] `[scope] include = ["node_modules/next/dist/docs"]` — index เพิ่มแม้ gitignore
      หรือ deny-list กันไว้; validate ว่าเป็น relative path ใน vault (กัน `..`)
- [x] implement เป็น **walk แยกรากต่อ include หนึ่งตัว** ไม่ไปสู้กับ precedence ของ
      crate `ignore` (สู้แล้วจะได้กฎที่ดูเหมือนทำงานแต่ไม่ทำ) แล้ว merge + dedup
- [x] `Note.reference` + `Scope::is_reference(key)` — คำนวณจาก config ไม่ต้องเพิ่ม
      table ไม่ต้อง migrate ไม่ต้อง bump INDEX_VERSION
- [x] **read-only guard ทุก write path** (`save_note`, `PUT`, `delete`, `rename`)
      — กับดักจริง: `save_note` resolve title ไปหาไฟล์ที่มีอยู่ ฉะนั้นโน้ตชื่อ
      `installation` จะไปทับหน้า docs ของ next แล้ว**หายเกลี้ยงตอน npm install
      ครั้งถัดไป** ความรู้ที่ agent เพิ่งบันทึกหายโดยไม่มีร่องรอย
- [x] `rename` ข้าม reference note ตอน rewrite backlink (docs ของ dependency
      อาจเอ่ยถึง title เดียวกัน) + รายงานว่าข้ามไปกี่ไฟล์
- [x] **missing root ต้องไม่ล้มและต้องไม่เงียบ**: `samong.toml` commit ไปกับ repo แต่
      `node_modules` ไม่ ฉะนั้น "หาไม่เจอ" เป็นสภาวะปกติ — `ReindexReport` พ่วง
      warning, `doctor` บอก present/NOT on this machine, `broken` เพิ่มหมายเหตุว่า
      target อาจ resolve ได้เมื่อติดตั้ง dependency
- [x] `doctor` แยกนับ project notes กับ reference notes และบอก**เหตุผล**ที่ข้ามไฟล์
      (dependency dir vs gitignore) — คนที่เดาว่า gitignore จะหยิบ `.samongignore`
      มาใช้ซึ่งเป็นคานงัดผิดอัน
- [x] `doctor` แยกความกำกวมสองชนิด (เจอจากข้อมูลจริง): docs ของ next ชนกันเอง
      108 title (mirror app/pages router) ซึ่งแก้ไม่ได้และไม่ต้องแก้ → สรุปบรรทัดเดียว;
      ส่วนที่ชนกับโน้ตของโปรเจกต์ → แสดงครบเพราะ `[[link]]` อาจไปผิดที่
- [x] regression test ตรึงพฤติกรรม **depth-0** (vault ที่ root อยู่ใน dependency dir
      ใช้ได้) — เดิมเป็นผลพลอยได้จาก `entry.depth() == 0` ถ้าวันหน้ามีใครแก้ให้เช็ค
      ทั้ง path พฤติกรรมนี้จะพังเงียบและ vault แบบนั้นจะว่างเปล่า

**Acceptance**: unit test ครบ (include ทะลุ gitignore, ไม่กินพี่น้องที่ prefix คล้ายกัน,
missing root, ไม่ซ้ำเมื่อ include อยู่ใน scan หลัก, escape vault ไม่ได้, watch ครอบ);
`tests/phase13.rs` ครอบ read-only guard / rename / warning / doctor;
ทดสอบกับ Next docs จริง 425 ไฟล์ → `4 project note(s) + 425 reference note(s)`,
reindex รอบสอง = 0

### ผลที่ยอมรับไว้แล้ว (ไม่ใช่ bug)

- **สมองมีสองชั้น**: reference notes ไม่เดินทางไปกับ git ฉะนั้นเมื่อ server กลาง
  ingest จาก git มันจะไม่มี 425 โน้ตนั้น — ลิงก์จากโน้ตโปรเจกต์ไปหามันจะขึ้น
  unavailable บน server รับได้ แต่ต้องรู้ตัว
- ทางเลือกที่ยังเปิดไว้: ถ้าเอกสารชุดไหนสำคัญถึงขั้นเป็นของโปรเจกต์อย่างถาวร ให้
  copy เข้า repo (`docs/vendor/...`) แล้ว determinism กลับมาเต็ม แลกกับขนาด repo
  และเรื่อง license

## Phase 14 — สัญญา API/MCP: อ้างโน้ตด้วย path (เพิ่มหลัง Phase 13)

> ปัญหา: index เปลี่ยนไปใช้ path เป็น identity ตั้งแต่ Phase 11 แต่ API กับ MCP
> ยังอ้างโน้ตด้วย **title** อยู่ ซึ่ง `validate_title` ห้าม `/` ฉะนั้นส่ง path
> เข้าไปไม่ได้เลยแม้จะอยากส่ง ผลคือใน vault ที่มี 108 title ซ้ำ (docs ของ next
> มี title `index` ถึง 39 ไฟล์) **เปิดโน้ตใบที่สองจากหน้าเว็บไม่ได้** และ
> sidebar มีแถวหน้าตาเหมือนกันซ้ำๆ
> เป้าหมาย: ทำสัญญาให้นิ่งก่อนลงมือทำ Web UI ไม่งั้นต้องเขียน UI สองรอบ

- [x] `ops::validate_key` + `ops::resolve_key` — path จาก URL/tool argument เป็น
      untrusted input: ปฏิเสธ absolute, drive prefix, `..`, `.`, segment ว่าง,
      backslash (คุมให้ key มีสะกดเดียว), null byte, และไฟล์ที่ไม่ใช่ `.md`
      แล้วยังเช็ค containment อีกชั้นด้วย canonicalize กัน symlink พาออกนอก vault
- [x] `GET /api/vaults/{vault}/notes` คืน `{key, title, reference}` ไม่ใช่ string
- [x] `GET/PUT/DELETE /api/notes/{vault}/{*path}` — axum บังคับให้ wildcard อยู่
      ท้าย pattern ฉะนั้น links ย้ายไป `GET /api/links/{vault}/{*path}`
- [x] `PUT` สร้างโฟลเดอร์แม่ให้ และคืน `indexed: bool` — โน้ตที่เขียนนอก scope
      จะบันทึกได้แต่ค้นไม่เจอ ต้องบอก ไม่ใช่ปล่อยให้หายเงียบ
- [x] `POST /api/vaults` — เพิ่ม vault จากเบราว์เซอร์ได้ **ตัวขัดขวางที่แท้จริงของ
      first-run**: เดิมดาวน์โหลด → รัน → หน้าจอเปล่า และทางแก้เดียวคือคำสั่ง CLI
      ที่ผู้ใช้ยังไม่เคยอ่าน
- [x] `GET /api/vaults/{vault}/doctor` — รายงาน scope แบบเดียวกับ CLI ให้เว็บ
      แสดงได้ ไม่งั้นเปิดเว็บเห็น 4 โน้ตแล้วไม่มีทางรู้ว่าข้ามไป 90
- [x] graph node เป็น **ไฟล์** (`{id, label, missing, reference}`) และ resolve
      raw target → key ก่อนวาด edge; target ที่ไม่มีโน้ตจริงกลายเป็น node
      `missing` แยกสี คลิกไม่ได้ — เดิม node เป็น title ทำให้ 39 ไฟล์ `index`
      รวมเป็นก้อนเดียวกลางกราฟ
- [x] MCP: `read_note` / `save_note` / `get_links` รับ `path`, `list_notes` คืน
      path + `[reference]`, `search_notes` แสดง `vault/path` — **หักดิบไม่รับ
      title เป็น fallback** เพราะการ resolve title ไปผิดไฟล์คือสิ่งที่กำลังแก้
- [x] `get_links` แสดง `[[target]] -> path` ให้ agent อ่านต่อได้โดยไม่ต้องเดา
      (0 = ไม่มีโน้ตนี้, หลายอัน = title กำกวม)
- [x] web client ตามสัญญาใหม่: `api.ts` typed ครบ, App keyed by path,
      sidebar แสดง badge อ่านเท่านั้น + tooltip เป็น path, editor `readOnly`
      (บล็อกการพิมพ์ดีกว่ารับแล้วไป fail ตอน save), palette เปิดด้วย key,
      empty state มีปุ่มเพิ่ม vault
- [x] manifest fields ใน `[vault]` (`description`, `version`, `license`,
      `source`) — ใส่ตอนนี้ทั้งที่ยังไม่มีใครอ่าน เพราะ `deny_unknown_fields`
      ทำให้การเพิ่มฟิลด์ทีหลังเจ็บกับ config ที่ commit ไปแล้ว

**Acceptance**: `tests/phase4.rs` เขียนใหม่ครบทุก endpoint รวม traversal 4 แบบ,
`POST /api/vaults` (สำเร็จ + ชื่อซ้ำ + path ไม่มี), doctor, graph ที่ edge เชื่อม
ไฟล์ไม่ใช่ title; `tests/phase7.rs` คุย MCP ด้วย path; `npm run build` type-check ผ่าน

### ยังไม่ทำ (ไปต่อ Phase 15)
- sidebar เป็น tree ตามโฟลเดอร์ (ตอนนี้ยังเป็นลิสต์แบน) + ยกระดับ visual
- panel แสดงผล `doctor` ในหน้าเว็บ (endpoint พร้อมแล้ว)

## Phase 15 — Web UI: ถูกต้องแล้วสวย (เพิ่มหลัง Phase 14)

> ใช้ skill `frontend-design` ตามกฎในไฟล์นี้ ทิศทางที่เลือก: **"graphite &
> highlighter"** — พื้นเทาเย็นอมน้ำเงิน (ตั้งใจเลี่ยง cream+serif+terracotta ซึ่ง
> เป็นค่า default ที่งาน AI generate ออกมาเหมือนกันหมด), indigo เป็นสี
> interactive และสีลิงก์ (การลิงก์โน้ต = การอ้างอิง)

**signature: รอยตัดคำ** — สีไฮไลต์ถูกสงวนไว้ใช้กับ "สิ่งที่ match" และ "สิ่งที่
active ตอนนี้" เท่านั้น ไม่ใช้กับ body text เลย และทุก match จะวาดขีดบางๆ ที่
ขอบคำที่ tokenizer ตัดได้ นี่คือวิทยานิพนธ์ของโปรดักต์ที่มองเห็นได้ทุกครั้งที่ค้น
พิสูจน์แล้วกับข้อมูลจริง: ค้น "ประเทศไทย" ในสตริงไม่มีเว้นวรรค
`ตลาดหลักทรัพย์แห่งประเทศไทยประกาศ...` → ได้ 2 token (`ประเทศ` + `ไทย`) เห็นสองแถบ
พร้อมขีดขอบคำ ซึ่งเป็นสิ่งที่ Obsidian ทำไม่ได้

- [x] type 3 บทบาท: **Bai Jamjuree** (display — semi-condensed เพราะชื่อโน้ตไทย
      ยาวและต้องพอในบรรทัดเดียว), IBM Plex Sans Thai (body — metrics สระบน-ล่างดี),
      IBM Plex Mono (**path และตัวเลข** เพราะ path คือ identifier ไม่ใช่ prose)
- [x] `NoteTree` — sidebar เป็น tree ตามโฟลเดอร์ พร้อมจำนวนโน้ตต่อโฟลเดอร์
      (โฟลเดอร์เกิน 25 โน้ตยุบไว้ก่อน) และ **แยกกลุ่มโน้ตอ้างอิงออกจากโน้ตของเรา**
      — ลิสต์แบน 429 บรรทัดใช้งานไม่ได้จริง แต่ path เป็นต้นไม้อยู่แล้ว
- [x] `VaultHealth` — sheet แสดงผล `doctor` endpoint: นับโน้ต, include root
      present/missing, ไฟล์ที่ข้ามเป็นกราฟแท่งต่อโฟลเดอร์, title ที่กำกวมกดเปิดได้
- [x] leading 1.75 สำหรับไทย, focus-visible ทุกที่, reduced-motion ครอบ animation
      ทั้ง 4 ตัว, responsive ถึงมือถือ

**สิ่งที่จับได้จากการตรวจงานตัวเอง** (ทำไมต้องเปิดดูของจริง ไม่ใช่แค่เขียนเสร็จ):
1. **แถบไฮไลต์ไม่ขึ้นเลย** — keyframe มีแต่ `from` + `fill-mode: both` ทำให้
   forwards fill ค้างที่ค่า 0% คือโปร่งใส ขีดขึ้นแต่แถบหาย = signature หายทั้งอัน
2. **บนมือถือ sidebar ไม่ยอมซ่อน** — วาง media query ไว้*ก่อน*
   `.sidebar { display: flex }` จึงถูกทับ (CSS ตัดสินด้วยลำดับเมื่อ specificity
   เท่ากัน) ย้าย responsive ไปท้ายไฟล์พร้อมคอมเมนต์อธิบายว่าทำไมต้องอยู่ท้าย
3. **`kbd` contrast 3.81** ตกมาตรฐาน AA (ตัวเล็ก 11px) → เปลี่ยนเป็น `--ink-2`
   ได้ 7.93; ตรวจครบทุกคู่สีทั้งสองธีมแล้วผ่าน AA หมด
4. API คืน path แบบ `\\?\C:\...` (verbatim prefix ของ Windows) ที่ CLI ตัดให้แล้ว
   แต่ server ไม่ได้ตัด → เพิ่ม `display_path` ฝั่ง server

**Acceptance**: `npm run build` type-check ผ่าน, ตรวจ computed style กับ server
จริง (tokens/fonts/tree/contrast/responsive/reduced-motion), 138 tests ผ่าน

### ยังไม่ทำ
- ตอนนี้ตรวจดีไซน์ด้วยการอ่าน computed style เพราะ browser pane ไม่แสดงผลจึง
  screenshot ไม่ได้ — ควรดูด้วยตาอีกรอบก่อนถ่ายรูปลง README/landing page
- graph view ยังไม่ได้ยกระดับ visual (node เป็นไฟล์ถูกต้องแล้วตั้งแต่ Phase 14)

## Phase 16 — Redesign: กราฟเป็นพื้นที่หลัก (เพิ่มหลัง Phase 15)

> Phase 15 เปลี่ยนแค่ token สี ฟอนต์ และเพิ่ม 2 component แต่**ไม่ได้แตะโครง** —
> ผังยังเป็น topbar + sidebar/editor/backlinks ของ Obsidian ทั้งดุ้น เปิดมาจึง
> รู้สึกว่า "แอปเดิม สีเปลี่ยนนิดหน่อย" รอบนี้แก้ที่ information architecture

**เปลี่ยนสมมติฐานเรื่องแบรนด์**: Samong เป็นโปรดักต์ระดับโลกที่*เก่ง*การตัดคำไทย
ไม่ใช่โปรดักต์ที่มีธีมเป็นไทย ฉะนั้นเอกลักษณ์ภาพต้องอ่านได้ทันทีสำหรับคนทั่วโลก
การตัดคำเหลือที่เดียวในภาษาภาพ: ขีดขอบคำบน snippet ที่ match ซึ่งเป็น*ข้อมูล*

**วิทยานิพนธ์ใหม่**: กราฟคือพื้นที่ทำงาน การค้นคือทางเข้าไปในกราฟ
- vault คือ*รูปร่าง*ที่จำได้ ไม่ใช่รายการไฟล์ — ไม่มี PKM ตัวไหนทำกราฟเป็นบ้าน
  (Obsidian ทำเป็นแท็บของแปลกที่เปิดดูครั้งเดียว)
- พิมพ์ค้น → node ที่ไม่ match หมองลง เหลือที่ตรงกันเรืองอยู่ = คำค้นกลายเป็นสถานที่
- อ่านโน้ตเป็น*สถานะ*ที่ทับบนแผนที่ ไม่ใช่อีกแอป (กด Esc กลับมาที่แผนที่)

- [x] `GraphCanvas` — d3-force + **Canvas 2D** (ไม่ใช่ three.js/WebGPU):
      หลายร้อย node เกินกำลัง SVG แต่ canvas ไหวสบาย และได้ dimming/glow มาด้วย
      **ปฏิเสธ 3D อย่างมีเหตุผล**: bundle ทั้งหมดถูกฝังในไบนารีตัวเดียว
      (ตอนนี้ 313 KB) 3D จะเพิ่มหลายร้อย KB เพื่อแลกกับสิ่งที่ไม่ช่วยให้หาโน้ตเจอ
      และทำให้ keyboard/screen reader ใช้ไม่ได้
- [x] ขนาด node = จำนวนลิงก์ (hub ดูเหมือน hub), สีตาม vault, **โน้ตอ่านเท่านั้น
      กับ target ที่ยังไม่มีโน้ตเป็น *สถานะ* (วงกลวง/เส้นประ) ไม่ใช่สีใหม่**
- [x] layout เกาะกลุ่มตามโฟลเดอร์ด้วย forceX/forceY — ไม่งั้นหลายร้อยโน้ตจับตัว
      เป็นก้อนเดียวแยกไม่ออก ตำแหน่งเลยทำหน้าที่แทนสีได้ (จึงไม่ต้องมีสีต่อโฟลเดอร์)
- [x] ใช้ **skill `dataviz`** กับชุดสี: รัน validator จริง ไม่เดา ผ่านทุกเช็คทั้งสองโหมด
      (CVD ΔE 11.9, normal 25.3) — และ validator บังคับให้**ลดจาก 6 สีเหลือ 4**
      ซึ่งทำให้ดีไซน์ดีขึ้น เพราะสถานะทำงานได้ดีกว่าสี; legend มีตลอดจึงไม่พึ่งสีเดียว
- [x] `SearchPanel` — ช่องค้นอยู่ในเฟรมถาวร (Ctrl+K = โฟกัส ไม่ใช่เปิด dialog)
- [x] `DetailPanel` — คอลัมน์ขวา: ลิงก์เป็น chip ที่บอกด้วยว่า resolve ได้ไหม
      (ค้าง/กำกวม) เห็นตอนอ่าน ไม่ใช่ตอนรันคำสั่ง
- [x] onboarding เมื่อยังไม่มี vault, rail ยุบได้, reader เต็มจอ

**บั๊กจริง 3 ตัวที่จับได้จากการวัดของจริง** (ไม่ใช่อ่านโค้ดแล้วเดา):
1. **canvas ไม่วาดเลยใน tab ที่ซ่อนอยู่** — `requestAnimationFrame` ไม่ยิงเมื่อ
   `document.hidden` ฉะนั้นถ้าเปิดแอปไว้ tab พื้นหลังแล้วกลับมา simulation จบไปแล้ว
   ไม่มีอะไรสั่งวาดซ้ำ → กราฟว่างเปล่า แก้ด้วย `paintNow()` ที่วาดตรงๆ +
   ResizeObserver + ฟัง `visibilitychange`
2. **transition สีที่มาจาก token ค้างข้ามธีม** — property ที่ transition แล้วค่าเป็น
   custom property interpolate ไม่ได้ตอน token เปลี่ยน computed color ค้างที่ธีมเดิม
   ทำให้ช่องค้นหาและปุ่มยังเป็นสีมืดในโหมด light (contrast 1.11!) แก้ด้วยการ
   **ไม่ transition สีเลยทั้งไฟล์** เหลือแต่ layout/transform
3. ระหว่างทางสคริปต์วัด contrast ของผมเองผิดสามรอบ (เอา oklab โปร่งใสมาเป็นฉากหลัง,
   ข้าม background ทึบของตัว element, วัดกลาง transition) — **ต้องแก้เครื่องมือวัด
   ก่อนจะเชื่อตัวเลขที่มันบอก**

**Acceptance**: วัดพิกเซลจริงบน server จริง — canvas 650×659 วาดจริง, พิมพ์คำไทย
แล้ว avgAlpha ตก 171 → 54 (หมอง 69%) พิกเซลทึบ 96 → 13 เหลือ node ที่ match เรือง,
detail column เปิดถูกโน้ต, contrast ผ่าน AA ทุกคู่ทั้งสองธีม, 138 tests ผ่าน

### ยังไม่ได้ทำ / ข้อจำกัดที่ต้องบอก
- **ผมยังไม่เคยเห็นหน้าตาด้วยตา** — browser pane ไม่ compositing จึง screenshot
  ไม่ได้ ตรวจได้ถึงระดับพิกเซลว่า *วาดอะไรออกมา* แต่พิสูจน์เรื่องสัดส่วนและ
  ความรู้สึกไม่ได้ ต้องให้เจ้าของโปรเจกต์เปิดดูก่อนถ่ายรูปลง landing page
- canvas เข้าถึงด้วยคีย์บอร์ดไม่ได้โดยธรรมชาติ — ทางเข้าที่ใช้คีย์บอร์ดได้คือ
  ช่องค้นกับ tree ซึ่งเป็น DOM จริง และ canvas มี aria-label บอกจำนวน + วิธีเข้าถึง
  ถ้าจะทำให้ครบควรเพิ่มการ focus/เดิน node ด้วยลูกศรในอนาคต

## Phase 17 — Release infra (เพิ่มหลัง Phase 16)

> เป้าหมาย: ให้ `git tag` ครั้งเดียวได้ไบนารีที่คนโหลดไปรันได้จริง โดยไม่ต้องมี
> Rust/Node — และให้ผู้ใช้ที่เจอ OS ขัดขวางรู้ว่าต้องทำอะไร

- [x] version → **0.3.0** ทั้ง `Cargo.toml` และ `web/package.json`
      (เริ่มที่ 0.3.0 เพราะ index กับ API ผ่านมาแล้วสองรุ่นในช่วงพัฒนา ถึงจะไม่เคย
      เผยแพร่ก็ตาม — เลขเวอร์ชันเลยเล่าลำดับจริงแทนที่จะแกล้งว่านี่คือรูปแรก)
- [x] `CHANGELOG.md` — เขียนจากมุมคนที่ไม่เคยเห็นโปรเจกต์นี้ ทุกอย่างคือของใหม่
      สำหรับเขา รวมหัวข้อ **ข้อจำกัดที่รู้อยู่** (ไบนารีไม่ได้เซ็น, ไม่มี auth,
      title ที่กำกวมยัง resolve เป็นไฟล์แรก)
- [x] `release.yml`: เพิ่ม target **`x86_64-macos`** (Intel Mac — `macos-latest`
      เป็น arm64 แล้ว ต้องใช้ runner `macos-13`), `cargo build --locked`
      (release ต้อง build จาก Cargo.lock ที่ commit ไว้ ไม่ใช่สิ่งที่ resolve ใหม่
      วันนี้), และสร้าง `.sha256` คู่กับทุก archive
- [x] **`src/update.rs` ต้องแก้คู่กัน** — `asset_target()` ตรึงรายชื่อ target ไว้
      ถ้าเพิ่มใน workflow แต่ไม่เพิ่มที่นี่ ผู้ใช้ Intel Mac จะได้
      "unsupported platform" จาก `samong update` ทั้งที่ไฟล์มีให้โหลด
      (มี test ตรึงว่าสองที่ต้องตรงกัน — ตรวจแล้ว 4/4)
- [x] README ทั้งสองภาษาเขียนใหม่เป็น **binary-first**: ดาวน์โหลด แตกไฟล์ รัน
      (ของเดิมยังบอกว่า "ยังไม่เปิด public" และสอนให้ build from source)
- [x] **บอกวิธีข้าม Gatekeeper/SmartScreen ให้ชัดบนหน้าติดตั้ง** — macOS
      *ปฏิเสธ*ไม่ให้เปิดไบนารีที่ไม่ได้เซ็น (ไม่ใช่แค่เตือน) ถ้าไม่เขียนไว้
      ผู้ใช้ Mac จะหลุดหมดโดยที่เราไม่รู้ว่าหลุดเพราะอะไร พร้อมคำสั่ง
      `xattr -d com.apple.quarantine` และวิธี verify checksum

**Acceptance**: YAML parse ผ่าน, target ใน workflow ตรงกับ `asset_target()` 4/4,
`cargo test` เขียว, clippy สะอาด

### ยังไม่ทำ — ต้องให้เจ้าของโปรเจกต์ตัดสิน
- **ยังไม่ tag** การ push tag = ปล่อย release สาธารณะจริง ซึ่งย้อนยากและเป็นการ
  กระทำที่ส่งออกไปข้างนอก จึงไม่ทำแทน
- code signing (Apple Developer ~$99/ปี, Windows cert แพงกว่า) — first release
  ยอมไม่เซ็นได้ถ้าเขียนวิธีข้ามไว้ชัด ซึ่งทำแล้ว
- release notes body ใน GitHub Release ยังไม่ได้ generate จาก CHANGELOG อัตโนมัติ

## Phase 18 — Landing Page (เพิ่มหลัง Phase 17)

> อยู่ที่ `site/index.html` — HTML ไฟล์เดียว CSS/JS inline ฟอนต์ self-host
> ไม่มี build step ไม่มี request ไปที่ third-party (สอดคล้องกับจุดขาย "ไม่มีคลาวด์"
> — จะดูตลกถ้าหน้าที่บอกว่าข้อมูลไม่ออกจากเครื่องดันโหลดฟอนต์จาก Google)

**บทเรียนเรื่องพาดหัวที่นำมาใช้**: พาดหัวต้องพูดถึง*ความสูญเสียที่
ผู้ใช้รู้สึกอยู่แล้ว* ไม่ใช่กลไก → **"You already solved this once."**
รอง: "Six months ago you worked it out and wrote it down. The note is still in your
repository." (ของเดิมใน README เป็นรายการฟีเจอร์)

**อังกฤษเป็นภาษาหลัก** ตามการตัดสินใจว่าเป็นโปรดักต์ระดับโลก — และเปลี่ยนกรอบของ
จุดขายเรื่องภาษา จาก "ค้นไทยได้" เป็น **"บางภาษาไม่เว้นวรรคระหว่างคำ"** (ไทย ญี่ปุ่น
จีน) ซึ่งเป็นตลาดที่ใหญ่กว่ามาก โดยบอกตรงๆ ว่าตอนนี้ไทยพร้อมใช้แล้ว

**hero ไม่ใช่ screenshot และไม่ใช่สโลแกน** — เป็นการกระทำเดียวที่เป็นเอกลักษณ์ของ
โปรดักต์ ที่เล่นได้ใน 3 วินาที: กราฟจำลอง vault ของ dev จริง พิมพ์แล้ว node ที่ไม่
ตรงหมองลง เหลือคำตอบเรืองอยู่ + auto-play ครั้งเดียวตอน scroll ถึง (คนส่วนใหญ่
ไม่พิมพ์) + ปุ่มตัวอย่างคำค้น เขียนด้วย canvas + spring relaxation สั้นๆ
**ไม่แนบ physics library ลงหน้า landing**
- ประหยัดจริง: ทั้งหน้าใช้ 0 dependency, กราฟ 18 node 24 edge
- ยอมรับตรงๆ ในโค้ดว่า node เป็นภาพประกอบ ไม่ใช่ข้อมูลสด แต่*พฤติกรรม*คือของจริง

**demo การตัดคำ** — ประโยคไทยที่ไม่มีเว้นวรรคเลย กดคำค้นแล้วเห็นแถบ + ขีดขอบคำ
จุดที่พิสูจน์ประเด็นได้แรงที่สุด: ค้น `ประเทศไทย` แล้ว**ไฮไลต์ 2 token** (`ประเทศ` +
`ไทย`) พร้อมอธิบายว่าทำไม substring matching ไม่พอ (`ตลาด` จะไปโดน
`ตลาดหลักทรัพย์` ซึ่งเป็นของละคน)

**ความซื่อสัตย์เป็นส่วนหนึ่งของดีไซน์** — มีหัวข้อบอกว่าไบนารีไม่ได้เซ็น พร้อมคำสั่ง
`xattr` และบอกว่า "นี่คือหน้าตาของโอเพนซอร์สที่ไม่มีงบซื้อใบรับรอง ไม่ใช่สัญญาณว่า
ไฟล์ผิดปกติ" — ปิดบังไว้จะเสียความเชื่อใจมากกว่าตอนผู้ใช้ไปเจอเอง

- [x] คอมมิตธีมมืดอย่างเดียว (ไม่ทำ light) — กราฟอ่านง่ายกว่าบนพื้นเข้ม และเป็น
      surface เดียวกับตัวแอป ทำให้สิ่งที่สัญญาไว้กับสิ่งที่โหลดไปเป็นของเดียวกัน
- [x] `.github/workflows/site.yml` — **`workflow_dispatch` เท่านั้น** การเผยแพร่หน้า
      สาธารณะเป็นการกระทำที่ส่งออกไปข้างนอก จึงเกิดตอนมีคนกดปุ่ม ไม่ใช่ผลข้างเคียง
      ของการ push (มีคอมเมนต์บอกวิธีเปิด auto-deploy ไว้ให้)
      · **ลบทิ้งภายหลัง** เมื่อเลือก Vercel เป็นบ้านของหน้าเว็บ — สองที่ที่เผยแพร่
      หน้าเดียวกันคือสองที่ที่วันหนึ่งจะไม่ตรงกัน และไม่มีใครรู้ว่าที่ไหนคือของจริง
      · หลักการเดิมไม่ได้หายไป: Vercel deploy จาก push แต่ `samong.dev` ชี้ที่
      production ซึ่งเปลี่ยนเมื่อ main เปลี่ยน ไม่ใช่เมื่อ branch ไหนก็ได้เปลี่ยน

**Acceptance** (วัดของจริงบน server): canvas 670×350 วาดจริง 126 sample ·
กดปุ่มคำค้นแล้ว alpha ตก 125 → 40 พร้อม "1 found" · demo ตัดคำไฮไลต์ 2 token ถูกต้อง ·
ฟอนต์โหลดครบ 6 ไฟล์ · **contrast ผ่าน AA ทั้ง 22 คู่ที่ตรวจ ไม่มีตกเลย** ·
มือถือ 375px ไม่มี horizontal scroll ไม่มี element ล้น

### ยังไม่ทำ
- **ยังไม่ได้เห็นด้วยตา** (browser pane ไม่ compositing) — ตรวจได้ถึงระดับพิกเซล
  และพฤติกรรม แต่สัดส่วน/จังหวะสายตาต้องให้เจ้าของโปรเจกต์ดู
- เวอร์ชันภาษาไทยของหน้านี้ (โครงพร้อมแล้ว แค่ยังไม่มีไฟล์ที่สอง)
- ยังไม่ enable GitHub Pages ใน repo settings และปุ่มดาวน์โหลดชี้ `/releases/latest`
  ซึ่งจะว่างจนกว่าจะ tag v0.3.0

## Phase 19 — Brand identity + ambient motion (เพิ่มหลัง Phase 18)

**ตรวจลิขสิทธิ์ฟอนต์แล้ว — ผลดีที่สุด**: ทั้งสามฟอนต์เป็น **OFL-1.1**
(Bai Jamjuree โดย Cadson Demak, IBM Plex Sans Thai / Mono โดย IBM) ใช้เชิงพาณิชย์
ได้ ฝังในไบนารีได้ และ**เอาข้อความที่ set ด้วยฟอนต์ไปสกรีนเสื้อ/แก้ว/ป้ายออฟฟิศได้**
เพราะ OFL คุมการแจกจ่าย*ตัวไฟล์ฟอนต์* ไม่ใช่ผลลัพธ์ที่ render ออกมา

- [x] **แก้ช่องว่างการปฏิบัติตาม OFL ที่เจอตอนตรวจ** — เราแจกไฟล์ฟอนต์ (ใน
      `site/fonts/`, ใน `web/dist`, และฝังในไบนารี) โดยไม่ได้แนบ license ซึ่ง
      OFL ข้อ 2 บังคับ → เพิ่ม `site/fonts/LICENSE-*.txt` ทั้งสามไฟล์ และหัวข้อ
      Fonts ใน `THIRD-PARTY.md`
- [x] Reserved Font Name: ถ้า*แก้*ฟอนต์ต้องเปลี่ยนชื่อ — เราไม่แก้ และ**โลโก้เป็น
      geometry ที่วาดเอง ไม่พึ่งฟอนต์เลย** จึงไม่ติดข้อนี้

**mark: "the found node"** — เครือข่ายเล็กๆ ที่มี node เดียวสว่าง
คือการกระทำเดียวของโปรดักต์ (คุณถาม แล้วสิ่งที่ต้องการคือสิ่งที่สว่างขึ้น) และ
node ที่สว่างเป็น element เดียวที่ได้ใส่สี accent ทั้งในโลโก้และในแอป
- 4 วงกลม 4 เส้น เท่านั้น → รอดที่ favicon 16px, สกรีนสีเดียว, และเข็มปักจักร
- `samong-mark.svg` (สี) + `samong-mark-mono.svg` (`currentColor` สำหรับสกรีน/ปัก/
  แกะสลัก/forced-colors) — เวอร์ชันสีเดียวแยก node ที่สว่างด้วย**น้ำหนัก**แทนสี
  (ทึบ vs วงแหวน) ซึ่งเป็นกลไกเดียวกับที่แอปใช้แยกโน้ตอ่านเท่านั้น
- [x] `SamongMark.tsx` ในแอปเปลี่ยนเป็น geometry เดียวกัน — ของเดิม**ยังเป็นรูป
      เรือนยอดต้นไทรกับรากอากาศ** ซึ่งเป็น metaphor ที่เลิกไปตอนเปลี่ยนชื่อแล้ว
- [x] lockup: mark + "Sam**o**ng" โดย **o เป็นสี `--found`** ทำให้ชื่อกับความคิด
      เป็นท่าทางเดียวกัน — o คือ node ที่สว่าง
- [x] ขยายแบรนด์ตามที่ขอ: nav 28px mark + wordmark 20.8px (เดิม ~17px),
      hero lockup mark 58px + wordmark 48px, favicon เปลี่ยนจาก emoji มาเป็น mark

**ข้อ 2 — ambient motion: ทำ แต่ไม่ใช่ three.js**
- ใช้ **canvas 2D วาด node field** ที่เป็น*วัสดุของโปรเจกต์เอง* (node + เส้นเชื่อม)
  ที่ความหนาแน่นต่ำพอให้อ่านเป็นพื้นผิว ไม่ใช่ของประดับ — 0 dependency
- **ปฏิเสธ three.js/WebGPU อย่างมีเหตุผล**: ~600 KB เพื่อพื้นหลังที่ไม่ได้ช่วยให้
  หาโน้ตเจอ กินแบต ทำ LCP แย่ลง และกับกลุ่มผู้ใช้ dev มักอ่านว่า "เว็บการตลาดที่
  ไม่มีอะไรจะพูด" — ถ้าจะใช้ 3D ต้องมีเหตุผลจากตัวเรื่อง ไม่ใช่เพื่อดึงดูด
- หยุดเมื่อ tab ซ่อน และไม่วาดเลยเมื่อ `prefers-reduced-motion`
- `pointer-events: none` ตรวจแล้วว่าไม่กินคลิกของช่อง search (elementFromPoint
  คืน `INPUT#q`)

**ข้อ 4 — ไม่มีภาษาไทยในหน้าเว็บ** ตรวจ DOM แล้ว: Thai ปรากฏแค่ใน `.tok`
(demo ตัดคำ) และ `<code>` ตัวอย่างที่อธิบายว่าทำไม substring matching ไม่พอ
**ไม่มีในเนื้อความเลย** — ไทยอยู่ในฐานะ*หลักฐานของความสามารถ* ไม่ใช่ภาษาของหน้า

**Acceptance**: contrast ผ่าน AA ทุกคู่ · ambient + graph วาดจริงทั้งคู่ ·
มือถือ 375px ไม่มี horizontal scroll · SVG ทั้งสองไฟล์ valid XML · 138 tests ผ่าน

### ข้อ 1 — สองอย่างที่ยังไม่ได้ทำกับตัวโปรดักต์ (หนี้ที่ต้องจ่าย)
ที่ทำแล้วคือ**เฉพาะ copy ของ landing page** (พาดหัว, กรอบเรื่องภาษา, ชื่อเครื่องมือ)
**ตัวโปรดักต์ยังไม่ได้แตะ** สองข้อที่ค้าง:
1. **ranking ด้วย degree** — ดึงผลมามากกว่าที่ขอ แล้วจัดอันดับใหม่โดยผสม BM25 กับ
   จำนวนลิงก์จาก graph ก่อนตัด "โน้ตที่หลายโน้ตลิงก์มามักเป็นสิ่งที่คุณหา"
   ถูก ไม่ต้องพึ่ง AI และเรามีข้อมูลนี้อยู่แล้ว (ใช้กำหนดขนาด node ในกราฟ)
2. **hybrid semantic search** — ช่องว่างจริงเทียบกับเขา ต้องยัง local + optional
   (fastembed-rs ใต้ feature flag) ถ้าทำเป็นคลาวด์คือทิ้งจุดยืนทั้งหมด

### ยังไม่ทำ (ปิดใน Phase 21)
- wordmark ยัง set ด้วย Bai Jamjuree (OFL อนุญาต) — **ขั้นถัดไปคือ outline เป็น
  path** ให้เป็นของเราจริงและไม่ต้องพึ่งฟอนต์ตอน render ผมยังไม่ทำเพราะ**มองผลไม่
  เห็น** (browser pane ไม่ compositing) การวาดตัวอักษรมือเปล่าโดยไม่เห็นผลเสี่ยงเกิน
- ยังไม่มีหน้า brand assets สำหรับดาวน์โหลด (ไฟล์พร้อมใน `site/brand/`)

## Phase 20 — i18n: อังกฤษเป็นค่าเริ่มต้น (เพิ่มหลัง Phase 19)

**ปัญหาที่โผล่ตอนตรวจงาน Phase 19**: landing page เป็นอังกฤษล้วนแล้ว แต่ *ตัวแอป*
ยังเป็นไทยล้วน — `<html lang="th">` กับ 88 สตริงไทยในบันเดิล คนทั่วโลกที่เดินมาจาก
หน้าเว็บจะโหลดแอปที่เขาอ่านไม่ออก **landing page ที่ดีก็พาไปตายที่หน้าแรกของแอป** — ข้อ 4 ที่สั่งไว้คือ "หน้าเว็บ" เลยแก้แค่หน้าเว็บ ซึ่งไม่พอ

- [x] `web/src/i18n.ts` — `en` เป็นแหล่งความจริง แล้ว `th` ถูก type-check เทียบมัน
      (`Record<MessageKey, Message>`) → **แปลไม่ครบกลายเป็น build error** ไม่ใช่
      label ว่างที่ไปเจอเอาตอนมีผู้ใช้จริง
- [x] **ไม่ใช้ i18n library** — 2 ภาษา ~70 สตริง ไม่ต้องมี plural-rule engine หรือ
      message parser และ UI นี้ฝังในไบนารี ทุก KB คือ KB ที่คนต้องดาวน์โหลด
- [x] plural: อังกฤษแยก one/other ไทยไม่แยก → type ยอมให้ฝั่งไทยยุบ `Plural` เป็น
      string เดียว เพราะนั่นคือ*คำแปลที่ถูก* ไม่ใช่คำแปลที่ขาด
- [x] ลำดับเลือกภาษา `?lang=` → localStorage → `navigator.languages` → `en`
      (ค่าที่ไม่รู้จักตกที่อังกฤษ ไม่ใช่ภาษาของคนเขียนโค้ด) และ `?lang=` **ไม่เขียน
      ทับค่าที่ผู้ใช้เลือกไว้** เหมือนที่ `?theme=` ไม่ทับ — มีไว้ให้ reproduce บั๊ก
      ในภาษาของคนแจ้งได้
- [x] `useT()` คืนฟังก์ชัน*ใหม่ต่อภาษา* → ใส่ใน dependency list ของ hook ได้จริง
      ถ้าคืนตัวเดิม component ที่ memo ไว้จะไม่รู้ว่าภาษาเปลี่ยน
- [x] ปุ่มสลับภาษาข้างปุ่มธีม แสดงภาษา*ปลายทาง* (EN/TH) set ด้วยฟอนต์ mono
      เพื่อให้อ่านเป็นตำแหน่งสวิตช์ ไม่ใช่คำที่รอการแปล
- [x] `index.html` ตั้ง `lang` ก่อน first paint จากสามแหล่งเดียวกัน (`i18n.ts` เป็น
      ผู้ตัดสินและแก้ให้ตรงตอน load) เพื่อให้ไทยได้ฟอนต์ถูกตั้งแต่เฟรมแรก

**ลบโค้ดตาย 4 ไฟล์** — `Sidebar`, `GraphView`, `RightPanel`, `CommandPalette` เป็น
ของเหลือจาก UI ก่อน Phase 16 ไม่มีใคร import แต่ถือสตริงไทยไว้ 23 อัน: ลบดีกว่าแปล

**ที่ยอมแลก**: 3 สตริงที่เคยมี `<code>` คร่อมชื่อ literal กลางประโยค
(`node_modules`, `scope.include`, `[[ลิงก์]]`) ตอนนี้เป็นข้อความล้วน เพราะ `t()`
คืน string ไม่ใช่ JSX ทางเลือกคือทำ `<Trans>` หรือ split ตาม marker ซึ่งเป็น API
ที่ใหญ่กว่าปัญหาที่แก้

**บั๊กที่เจอระหว่างทาง**: `tree.empty` บอกให้ "กด 'โน้ตใหม่'" แต่ปุ่มนั้นหายไป
พร้อม `Sidebar` ตอน Phase 16 → เปลี่ยนไปชี้ช่องค้นหา ซึ่งเป็นทางสร้างโน้ตจริง

**Acceptance**: `tsc -b` ผ่าน · ไทยเหลือใน `i18n.ts` ไฟล์เดียว · `dist/index.html`
เป็น `lang="en"` · ทั้งสองพจนานุกรมอยู่ในบันเดิล · 138 tests + clippy + fmt ผ่าน

## Phase 21 — Wordmark outlined + brand assets (เพิ่มหลัง Phase 20)

**ปิดหนี้ที่ Phase 19 เลื่อนไว้** เหตุผลเดิมคือ "วาดตัวอักษรโดยมองผลไม่เห็นเสี่ยงเกิน"
— ซึ่งยังจริง browser pane ยัง screenshot ไม่ได้ **แต่โจทย์ตั้งผิด: เราไม่ต้องวาด
เราแค่ต้อง extract** ดึงเส้นรอบตัวอักษรจาก Bai Jamjuree 700 ที่หน้าเว็บใช้อยู่แล้ว
ออกมาเป็น path ตรงๆ ตัวอักษรจึงเป็น*ชุดเดิมทุกจุด* ไม่มีการออกแบบใหม่แม้แต่นิด
เป็นงาน mechanical ที่พิสูจน์ด้วยการวัดได้ ไม่ต้องใช้ตา

- [x] `fontTools` อ่าน `.woff` (ไม่ใช่ woff2 เพราะไม่มี brotli) → glyph outlines
      ของ S a m o n g, upem 1000, cap height 700
- [x] **ตรวจ GPOS ก่อน**: ทั้ง 5 คู่ (Sa am mo on ng) **ไม่มี kerning** ฉะนั้น
      advance width + track `-0.04em` reproduce สิ่งที่เบราว์เซอร์วาดได้เป๊ะ
- [x] **พิสูจน์ว่าตรง**: ให้เบราว์เซอร์วัดความกว้างข้อความจริงที่ font-size 1000px
      → **3888 หน่วยแบบไม่มี track, 3648 หน่วยเมื่อ track -0.04em** ตรงกับที่คำนวณ
      จาก advance ทุกหน่วย และ aspect ของ SVG ที่ได้ = 3.9427 vs คำนวณ 3.9429
- [x] `site/brand/samong-{wordmark,lockup}{,-mono}.svg` + `<symbol id="wordmark">`
      inline ในหน้าเว็บ + `web/src/components/SamongWordmark.tsx` — **ชุดเดียวกัน
      จากไฟล์ต้นทางเดียวกัน** แอปกับเว็บจึงเป็นแบรนด์เดียว ไม่ใช่การ render ฟอนต์
      สองที่
- [x] lockup มีสัดส่วนตายตัว: mark 1.208em, gap 0.283em ของ wordmark — **เอามาจาก
      hero ที่ใช้อยู่แล้ว** (58px / 13.6px / 48px) ไม่ได้คิดสัดส่วนใหม่
      mark จัดกลางที่ cap-height midpoint ซึ่งเป็น convention มาตรฐานของ lockup

**ข้อ 3 — ขยายจริงและวัดได้** เปลี่ยนจากคุมด้วย `font-size` มาคุมด้วย **cap height**
เพราะ cap height คือสิ่งที่ตาอ่านว่า "คำนี้ใหญ่แค่ไหน" และเป็นตัวเลขเดียวที่ยัง
เทียบกันได้เมื่อ type กลายเป็น path (กล่อง outline สูง 911 หน่วยต่อ cap 700 →
`height = cap x 1.3`)

| | เดิม (cap) | ใหม่ (cap) | mark เดิม → ใหม่ |
|---|---|---|---|
| nav | 14.6px | **17px** (+17%) | 28 → 29px |
| hero @1280 | 33.6px | **40px** (+19%) | 58 → 69px |
| แอป toolbar | 11.2px | **13px** (+16%) | 20 → 22px |
| แอป onboarding | 21px (h1 30px) | **26px** | 44 → 45px |

- [x] `.lockup-mark` ใช้ `6.9vw` ไม่ใช่ `7vw` เพื่อให้อัตราส่วน 1.726x cap คงที่
      **ทุกความกว้าง** ไม่ใช่แค่ที่ปลายสอง clamp — วัดได้ 1.727 ที่ 1280px
- [x] **เจอบั๊กที่มีอยู่ก่อนแล้ว**: ที่ 375px nav ล้น (349 > 335) มาตั้งแต่ก่อน
      ขยาย wordmark แต่ไม่มีใครจับได้เพราะมันถูก *clip* ไม่ได้ทำให้ document
      scroll → ซ่อนปุ่ม GitHub ที่ ≤430px (Download คุ้มความกว้าง GitHub อยู่ footer)

**หน้า brand assets** `site/brand.html` — lockup / wordmark / mark, สองเวอร์ชัน
ต่ออัน, กฎการใช้ (clear space, ขนาดต่ำสุด, one lit node, ห้าม redraw), swatch สี,
ตารางฟอนต์+license และประโยคที่บอกว่า **Apache-2.0 ไม่ครอบชื่อกับโลโก้**
- ตรวจ pixel แล้วว่า mono ทั้งสามไฟล์วาดออกมาจริง (opaque = dark 100%) และ
  ไฟล์สีมี accent อยู่ 2264 px จาก 11720 — ไม่มีไฟล์ไหน "โปร่งใสแบบมองไม่เห็น"
- เขียนกำกับไว้ว่าไฟล์สีวาดด้วย ink สว่างสำหรับพื้นเข้ม → **บนพื้นขาวให้ใช้ mono**
  (กับดักที่คนดาวน์โหลดไปวางบนสไลด์ขาวแล้วเห็นแค่ node สีเดียว)

**ลิขสิทธิ์ — ดีขึ้นกว่าเดิมอีกชั้น**: outline ทำให้โรงพิมพ์/ร้านสกรีน
**ไม่ต้องมี license ฟอนต์เลย** เพราะไม่มีฟอนต์อยู่ในสายงานอีกต่อไป OFL คุมการแจก
*ตัวซอฟต์แวร์ฟอนต์* ไม่ใช่ artwork ที่ outline แล้ว และเราไม่ได้แก้ฟอนต์ Reserved
Font Name จึงไม่ถูกแตะ — จดเหตุผลนี้ไว้ในหัวไฟล์ SVG ทุกใบและใน `brand.html`

**Acceptance**: SVG 6 ไฟล์ valid XML · aspect ตรงกับที่วัดจากเบราว์เซอร์ทุกจุด ·
375/430/1280px ไม่มี overflow · brand.html ไม่มีรูปเสีย ไม่มีลิงก์เสีย ·
138 tests + clippy + fmt ผ่าน

### บั๊กที่การวัดจับไม่ได้เลย — และทำไม (สำคัญ)
รอบแรก wordmark **ขึ้นเว็บแบบพัง** โดยที่ทุกตัวเลขผ่านหมด: `getBBox()` ของทั้ง 6
path ถูก, กล่องนอก 87.1x22.1 ถูก, aspect 3.9427 ถูก, computed fill ถูกทั้ง ink
และ accent — **แต่บนจอเห็นแค่ก้อนมืดเล็กๆ** คือหางของตัว `g`

เหตุ: `<symbol viewBox="38 -708 3592 911">` ที่ถูกอ้างด้วย `<use>`
**สร้าง viewport ซ้อนที่ยึดมุมไว้ที่ (0,0)** เนื้อหาจึงถูกเลื่อน `(-38, +708)`
เหลือให้เห็นแค่ 203 หน่วยล่างสุด `getBBox()` วัด geometry ใน user space ของเนื้อหา
**ไม่ได้วัดว่า viewport ซ้อนพาไปวาดไว้ที่ไหน** จึงรายงานว่าปกติทุกอย่าง
`#mark` รอดมาตลอดเพราะ viewBox ของมันเริ่มที่ `0 0` พอดี — ความบังเอิญ ไม่ใช่ดีไซน์

แก้ที่ราก: generator ทำสองรอบ รอบสองปล่อย path ที่ **normalize ให้หมึกเริ่มที่
(0,0)** ทุกไฟล์จึงเป็น `viewBox="0 0 3592 911"` การซ้อน viewport กลายเป็น identity

**บทเรียน**: การวัดพิสูจน์ได้ว่า*เรขาคณิตถูก* แต่พิสูจน์ไม่ได้ว่า*มันถูกวาดที่ไหน*
สองอย่างนี้ไม่ใช่เรื่องเดียวกัน และ 3 phase ที่ผ่านมาเราตรวจแค่อย่างแรก

### แก้ตามที่เห็นด้วยตา (เจ้าของสั่งหลังดู screenshot)
- [x] **เอา lockup ออกจาก hero** — พอมองด้วยตาก็เห็นว่า "Samong" (cap 40px) กับ
      พาดหัว (cap ~40px) **ขนาดเท่ากันพอดี อัตราส่วน 1.0** ตาจึงไม่รู้ว่าอะไรคือ
      ใจความ แบรนด์อยู่ที่ header อยู่แล้ว การพูดชื่อซ้ำที่ hero คือการแย่งเวที
      ตอนนี้พาดหัวชนะชัด **1.92 เท่าของแบรนด์** (วัด cap ต่อ cap ไม่ใช่กล่องต่อกล่อง)
- [x] แลกมาด้วยการ**ขยาย brand ที่ header**: cap 17 → **21px**, mark 29 → **36px**
      (เดิมก่อน Phase 21 คือ cap 14.6 / mark 28 → รวมสองรอบโตขึ้น 44% / 29%)
- [x] ปุ่ม GitHub ใส่ **GitHub mark** (`<symbol id="gh">`, nominative use — เป็น
      เครื่องหมายของคนอื่นชิ้นเดียวบนหน้านี้) และปุ่ม "Read the source" ที่ hero ด้วย
- [x] nav แคบลงไม่พอเพราะ brand โตขึ้น + ปุ่มมี icon → **ไม่ทิ้งลิงก์ repo**
      แต่ให้ยุบเหลือ icon ที่ ≤560px (51.6px) และซ่อนที่ ≤390px โดยยังมี
      `aria-label` ให้ screen reader อ่านชื่อได้ตลอด
- ตรวจ 375 / 480 / 880 / 1280px: nav พอดีทุกช่วง ไม่มี overflow icon วาดจริงทั้งสองปุ่ม

## ทิศทาง: git-native ไม่ใช่ sync engine (ตัดสินใจก่อน Phase 10)

> โจทย์ที่ตามมาคือ "vault ของทั้งทีมที่ค้นรวมกันได้จาก server กลาง" — Obsidian ตั้งใจไม่แตะเรื่องนี้ (Sync = อุปกรณ์ของ
> ตัวเอง, Publish = ทางเดียว, ไม่มี server-side index/API/ACL)

หลักที่ยึด: **อย่าเขียน sync protocol เอง** vault ของเราคือ git repo อยู่แล้ว git
แก้ conflict resolution / history / offline / auth ให้ครบแล้ว

- repo = source of truth (ไม่เปลี่ยนหลักการเดิม `.md` เป็นแหล่งเดียว)
- server กลาง = index + search + graph + API ข้าม vault **ไม่ใช่ file server**;
  ingest จาก git (clone/pull หรือ CI push ตอน merge)
- "เอากลับมาอัปเดท" = แก้ `.md` → commit → push แล้ว server เห็นเอง
  ไม่ต้องมี write path ของ samong เอง
- ACL ยืมจาก git host: ใครเข้า repo ได้ก็ค้น vault นั้นได้ — อย่าสร้าง permission
  system ของตัวเอง (จะใหญ่กว่า search engine ทั้งตัว)
- **local-first ต้องไม่หาย**: vault ที่ไม่ใช่ git repo (Obsidian แท้ๆ) ต้องใช้ได้
  เหมือนเดิมทุกอย่าง git เป็นแค่ transport เมื่อมี ไม่ใช่ requirement

## สิ่งที่ *ไม่ทำ* ในแผนนี้ (จดกันหลงทาง)

- ❌ Sync ข้ามเครื่อง / บัญชีผู้ใช้ — อยู่นอกขอบเขตของ core
- ❌ AI features (สรุปโน้ต, ถามตอบกับ vault) — รอ core นิ่งก่อน
- ❌ Mobile app
- ❌ Plugin system — อย่าเพิ่ง overengineer

## Phase 22 — CI แดงมา 3 push แล้วไม่มีใครดู (เพิ่มหลัง Phase 21)

หลัง push ครั้งแรกไปดู `gh run list` แล้วเจอว่า **CI ล้มติดกันตั้งแต่ Phase 19**
ทั้งที่ในเครื่อง 138 tests ผ่านทุกครั้ง — สองสาเหตุ ทั้งคู่เป็นความผิดของเทสต์เอง
ไม่ใช่ของโค้ด และทั้งคู่เป็นชนิด "ผ่านในเครื่องแต่ล้มบน CI"

**1. เทสต์แย่ง lock ของ registry จริงของผู้ใช้**
`Error: opening registry ~/.config/samong/registry.redb / Database already open`
— `tests/{phase1,phase3,phase10,phase13,integration}.rs` เรียก CLI โดย
**ไม่ตั้ง `SAMONG_CONFIG_DIR`** จึงไปเปิด registry ตัวจริง redb ล็อกแบบ exclusive
และ cargo รันเทสต์ขนานกัน → ชนกัน ในเครื่องรอดเพราะจังหวะไม่ทับ ไม่ใช่เพราะถูก
- อันตรายกว่าที่เห็น: `cargo test` **แก้ registry ของคนที่รันได้** เทสต์ที่
  add/remove vault จะไปยุ่งกับของจริง
- แก้: helper ตั้ง `SAMONG_CONFIG_DIR` เป็น `<vault>/.samong-test-config`
  ต่อการเรียกหนึ่งครั้ง — แยกต่อเทสต์เอง ไม่ต้องส่งอะไรเพิ่ม และเป็น dot-dir
  จึงไม่ถูกนับเป็นโน้ต (`phase2/4/5/7/12` ทำถูกอยู่แล้ว 5 ไฟล์ที่เหลือไม่ทำ)
- **พิสูจน์**: จับ mtime+size ของ registry จริงก่อน/หลัง `cargo test` → ไม่ขยับ

**2. assert เวลานาฬิกาบน CI**
`incremental (1.30s) must be clearly faster than full (2.36s)` — เร็วกว่าจริง
1.8 เท่า แต่ assert ขอ 2 เท่า เพราะ process startup + tandivy commit เป็นต้นทุน
คงที่ที่ไฟล์เดียวหารไม่ลง **ตัวตัดสินคือ scheduler ของ runner ไม่ใช่โค้ดเรา**
- แก้: เลิก assert เวลา ไปยืนยัน*งานที่ทำ* แทน ซึ่งคือสิ่งที่ฟีเจอร์สัญญาไว้จริง
  และ deterministic — reindex รอบสองต้องได้ `reindexed 0 note(s)` (เดินครบ 1000
  ไฟล์แล้ว hash ปฏิเสธทุกไฟล์) ถ้า pre-filter พังจะกลายเป็น 1000 แล้วล้มด้วย
  เหตุผลที่ถูก · เปลี่ยนชื่อเทสต์เป็น `incremental_reindex_touches_only_the_changed_note`
- **บทเรียน**: assert เวลานาฬิกาไม่ควรอยู่ใน CI ถ้าอยากวัด perf ให้ทำเป็น bench

**ที่ยังค้าง**: tag `v0.3.0` บน remote ชี้ `9883269` (Phase 20) แต่ main ไปถึง
Phase 21 แล้ว → release ที่ปล่อยไปสร้างจากคอมมิตเก่ากว่าที่หน้าเว็บโฆษณาอยู่
**ไม่ย้าย tag ที่ปล่อยแล้ว** — ค่อยออก v0.3.1 เมื่อ CI เขียว

## Phase 23 — กราฟกับ tree ที่มองด้วยตาแล้วอ่านไม่ออก (เพิ่มหลัง Phase 22)

สองข้อนี้เจอจาก screenshot เท่านั้น ทุกตัวเลขก่อนหน้าถูกหมด

**1. กราฟ: reference 425 กลบโน้ตจริง 5**
เห็นเป็นทุ่งวงแหวนเหมือนกันหมด กระจายสม่ำเสมอ แยกไม่ออกว่าโน้ตของเราอยู่ไหน
- [x] **ซ่อน reference เป็นค่าเริ่มต้น** แผนที่ตอบคำถาม "เรารู้อะไร" เอกสารของคนอื่น
      เป็นแหล่งค้นหา ไม่ใช่ความรู้ของเรา
- [x] **แต่กรองแบบหยาบไม่ได้** — ลองแล้วเหลือ 11 node 0 เส้น เพราะข้อมูลจริงคือ
      14 เส้น: **12 เส้น ref→missing และ 2 เส้น own→ref** ตัดหมดแล้วเส้นหายเกลี้ยง
      และ missing 6 อันลอยเปล่าๆ — กรองถูกแต่ภาพใช้ไม่ได้
- [x] เปลี่ยนเป็น **"โน้ตของเรา บวกหนึ่งก้าว"**: เก็บโน้ตเราทั้งหมด แล้วเก็บสิ่งที่
      โน้ตเราแตะตรงๆ หน้าเอกสารที่เราอ้างจริงเป็นส่วนหนึ่งของแผนที่ · missing
      มีความหมายเฉพาะเมื่ออยู่ข้างโน้ตที่เอ่ยถึงมัน จึงมาด้วยเมื่อโน้ตนั้นอยู่
      snapshot seed ไว้ก่อน loop เพื่อให้เป็นก้าวเดียวจริง ไม่ใช่ flood fill
      → **7 node 2 เส้น** (โน้ตเรา 5 + หน้า Next.js ที่ `PROJECT_OVERVIEW.md` อ้าง 2)
- [x] เมื่อเปิด reference ให้มัน**ถอยเป็นพื้น**: alpha 0.45 และรัศมีเล็กลง
      (2.8 + degree*0.3 เทียบกับ 4 + degree*0.75) — สองช่องทางพูดเรื่องเดียวกัน
      เพราะช่องทางเดียวไม่พอที่อัตราส่วน 85:1 · match/select ยังชนะเสมอ
- [x] **legend เป็นตัวควบคุมเอง** ไม่มีที่อื่นให้ไปหาปุ่มที่แปลว่าสิ่งที่ legend
      บอกอยู่แล้ว แสดงจำนวนที่ซ่อน (423 = reference ที่ไม่อยู่บนแผนที่ ไม่ใช่
      node ทุกชนิด) พร้อม `aria-pressed`
- [x] **จัดกลุ่มด้วยโฟลเดอร์ที่ลึกสุด ไม่ใช่ระดับบนสุด** — ของเดิมใช้ segment แรก
      ซึ่งเป็น `node_modules` สำหรับ reference ทุกไฟล์ กลุ่มจึงยุบเป็นหนึ่งเดียว
      และ clustering พูดอะไรไม่ได้เลย · anchor เฉพาะ 10 กลุ่มที่ใหญ่สุด
      ที่เหลือปล่อยไว้กลางจอ (ring ของ anchor หลายสิบอันคือ ring ไม่ใช่โครงสร้าง)

**2. note tree: บันไดโฟลเดอร์ลูกเดียว 5 ชั้น**
- [x] `compressBranch` ยุบสายที่ไม่มีอะไรนอกจากลูกคนเดียว →
      `node_modules/next/dist/docs` เป็น **แถวเดียวที่ indent 8px** (เดิม 5 แถว
      และ label ถูกตัดเหลือ `0…`) · root ไม่ถูกยุบเข้ากับลูก
- [x] `initialCollapsed` ต้องคิดจาก tree ที่ยุบแล้ว ไม่ใช่ tree ดิบ — path
      เปลี่ยนหลังยุบ ถ้าใช้ของดิบจะไป key collapse state กับแถวที่ไม่มีอยู่

**3. release.yml: `macos-13` ถูกปลดระวาง งาน x86_64-macos ค้างตลอดกาล**
v0.3.0 รอ **5 ชม. 51 นาที** แล้วไม่เคยได้ archive นี้ — เปลี่ยนเป็น cross-compile
`x86_64-apple-darwin` จาก runner arm64 ซึ่งไม่ต้องใช้ image พิเศษเพราะ macOS
มี SDK ทั้งสอง slice · เพิ่ม `rust_target` ใน matrix และ path
`target/<triple>/release` ใน Package

**Acceptance**: default 7 node 2 เส้น · กด legend → 436 node 14 เส้น · กดกลับ →
7/2 · badge 423 · tree แถวเดียวสำหรับ vendored docs · 138 tests + clippy + fmt ผ่าน

## Phase 24 — จัดอันดับด้วย degree (เพิ่มหลัง Phase 23)

**จ่ายหนี้ที่ค้างไว้ครึ่งแรก** ที่ค้างมาตั้งแต่ Phase 19 — "โน้ตที่หลาย
โน้ตลิงก์มามักเป็นสิ่งที่คุณหา" ข้อมูลมีอยู่แล้วในกราฟ ไม่ต้องพึ่ง AI

- [x] `Graph::degrees()` นับลิงก์ที่แตะแต่ละโน้ต **ทั้งสองทาง** ใน read
      transaction เดียว — target เป็นข้อความดิบใน `[[...]]` จึงต้อง resolve ผ่าน
      ตาราง TITLES · ถ้าเรียก `keys_for_title` ต่อ edge จะเปิด transaction ต่อ
      edge และฟังก์ชันนี้รันทุกครั้งที่ค้น
- [x] target ที่ไม่ชี้ถึงโน้ตไหน (หรือชี้ข้าม vault) **นับให้ต้นทางเท่านั้น**
      เพราะไม่มีอะไรถูกเชื่อม
- [x] `search::query_ranked()` — ดึง candidate มา **3 เท่า** ของที่ขอ คูณคะแนน
      BM25 ด้วย boost แล้วเรียงใหม่และตัด · ถ้า re-rank แค่ผลที่จะคืนอยู่แล้ว
      จะสลับลำดับได้แต่**ดึงโน้ตที่ BM25 ตัดออกไปนอกเส้นกลับมาไม่ได้** ซึ่งเป็น
      เคสที่คุ้มค่าที่สุด
- [x] เพดาน pool = `MAX_LIMIT` เดิม **ไม่สร้างประตูหลัง** ให้ตัวเองดึงเกินที่
      caller ถูกจำกัด → ผลข้างเคียงคือคนขอ 100 จะไม่ได้ overfetch (แต่ขอ 100
      ก็คือขอดูทั้งหมดอยู่แล้ว)

**สูตร boost** `1 + 0.25 * ln(1+degree)/ln(13)` เพดาน **+25%**
- **ลอการิทึม** ลิงก์แรกๆ มีน้ำหนักมากสุด โน้ตที่มี 50 ลิงก์ไม่กลบโน้ตที่มี 5
- **อิ่มตัวที่ degree 12** ซึ่งเป็น**ตัวเลขเดียวกับที่กราฟหยุดขยายรัศมี node**
  → node ที่ดูเหมือน hub ก็ติดอันดับเหมือน hub UI กับ ranking พูดตรงกัน
- **25% ตั้งใจให้เล็ก** ความเชื่อมโยงเป็น*คำใบ้ ไม่ใช่ข้อโต้แย้ง* โน้ตที่ตรงคำ
  ห่างกันเกิน 25% จะไม่ถูกแซงเพราะความนิยม — นี่คือเส้นที่กันไม่ให้ search
  กลายเป็นการประกวดความนิยม ซึ่งจะแย่กว่าของเดิม
- ตัดสินเสมอด้วย key เพื่อให้ลำดับเหมือนกันทุกเครื่องทุกครั้ง

- [x] `ops::search_vault()` เป็น**ทางเข้าเดียว**ของทั้ง CLI / HTTP / MCP —
      คำค้นเดียวกันจึงได้ลำดับเดียวกันทุกช่องทาง และ `search.rs` ยังไม่รู้จักกราฟ
- [x] กราฟเปิดไม่ได้ (index เก่า) → **ตกลงไปใช้ relevance เปล่า ไม่ล้มคำค้น**
      เพราะนั่นเป็นปัญหาของ index ไม่ใช่ของการค้นหา

**เทสต์ 9 ตัว** — ที่สำคัญคือคู่ที่กันสองทาง:
`connectedness_decides_between_equally_relevant_notes` (โน้ตที่ถูกตัดออกนอกเส้น
ถูกดึงกลับมาอันดับ 1) และ `relevance_still_outranks_connectedness` (degree 500
ยังแพ้โน้ตที่ตรงคำกว่า) · ทดสอบผ่าน **CLI จริง** ใน `tests/phase24.rs` ด้วย
เพราะ unit test บน `search` จะผ่านแม้ CLI ยังเรียกฟังก์ชันที่ไม่ ranked

**Acceptance**: 147 tests + clippy + fmt ผ่าน · README ทั้งสองภาษาอธิบายเพดาน 25%

## Phase 25 — ค้นด้วยความหมาย: local + optional (เพิ่มหลัง Phase 24)

**ข้อสุดท้ายที่ค้างไว้** ช่องว่างจริงคือ ค้นด้วยคำหาเจอเฉพาะโน้ตที่ใช้
คำเดียวกับที่พิมพ์ ถ้าจำคำที่เคยเขียนไม่ได้ก็หาไม่เจอ

- [x] **`semantic` feature ปิดเป็นค่าเริ่มต้น และไบนารีที่ปล่อยไม่มีมัน** —
      ไม่ใช่ความกลัวโค้ด แต่เป็นความซื่อสัตย์เรื่องสิ่งที่ต้องแลก: ลาก ONNX Runtime
      เข้ามา และ `embed` ครั้งแรกโหลดโมเดล ~120 MB จาก Hugging Face
      **โน้ตกับคำค้นยังไม่ออกจากเครื่อง** แต่ "ไฟล์เดียว ไม่ต้องโหลดอะไร" จะไม่จริง
      และคำสัญญานั้นคือเหตุผลที่คนเลือกเราแทนคลาวด์
- [x] **โมเดล multilingual โดยเจตนา** `multilingual-e5-small` 384 มิติ 100+ ภาษา
      — โปรเจกต์ที่ใกล้กันใช้ `BGE-small-en-v1.5` ที่อ่านอังกฤษเท่านั้น ถ้าเราใช้โมเดลอังกฤษ
      ก็เท่ากับยกจุดแข็งเรื่องไทยให้เขาในจุดที่สำคัญที่สุด
- [x] E5 ต้องมี prefix `query:` / `passage:` ถ้าไม่ใส่ ประโยคเดียวกันในฐานะคำค้น
      กับในฐานะเนื้อหาจะ embed ไปที่จุดเดียวกัน และคุณภาพอันดับตกลง
- [x] **`vectors.redb` เป็นไฟล์แยก** ไม่ใช่ตารางใน graph — เปิด/ปิดฟีเจอร์จึงไม่เคย
      migrate หรือเสี่ยงกับ index ที่ทุกอย่างพึ่งพา ลบไฟล์แล้ว vault กลับเหมือนเดิม
- [x] ตราด้วย **content hash ตัวเดียวกับที่ reindexer ใช้** → embed รอบสองข้ามโน้ต
      ที่ไม่เปลี่ยน (การ embed เป็นสิ่งที่ช้าที่สุดในโปรแกรมนี้) และ store
      **ปฏิเสธ vector จากโมเดลอื่น** เพราะ cosine ข้ามโมเดลเป็นเลขที่ไม่มีความหมาย
- [x] ซอยโน้ตเป็นชิ้น ~900 ตัวอักษรตัดที่ย่อหน้า **งบตั้งจากไทย** (2-3 ตัวอักษร
      ต่อ token เทียบกับอังกฤษ 4-5) เพราะการตัดหางโน้ตไทยหายเงียบๆ คือความพังที่
      สังเกตยากที่สุด · ให้คะแนนตามชิ้นที่ดีที่สุด ไม่ใช่ค่าเฉลี่ย — เอกสารยาว
      ที่ตอบคำถามในหัวข้อเดียวคือคำตอบที่ดี การเฉลี่ยจะกลบมัน
- [x] **`samong embed` แยกจาก `reindex`** — reindex ต้องเร็วและทำงานออฟไลน์ได้
      การ embed ไม่ใช่ทั้งสองอย่าง ถ้ายัดรวมกัน ทุกครั้งที่เซฟจะคาดเดาไม่ได้
- [x] `samong doctor` บอกจำนวนโน้ตที่มี vector เพื่อแยก "ค้นด้วยความหมายไม่ช่วย"
      ออกจาก "ยังไม่ได้ embed" — โรคเดียวกับ vault ที่ index ได้ 4 ไฟล์จาก 90
- [x] build ที่ไม่มีฟีเจอร์ยังมีคำสั่ง `embed` อยู่ แต่บอกว่าขาดอะไร ทำไม
      และคำสั่งเดียวที่ได้มันมา — ไม่ใช่ "unknown subcommand" ที่ทำให้คนสงสัยว่าพิมพ์ผิด

**รวมสองอันดับด้วย RRF ไม่ใช่ถ่วงน้ำหนักคะแนน**: BM25 ไม่มีขอบเขต cosine อยู่
−1..1 การผสมเลขดิบต้อง calibrate และค่านั้นเลื่อนไปตาม vault การผสม*อันดับ*ไม่ต้อง
- `score` คืนค่าบนสเกล RRF **เสมอ** แม้มีอันดับเดียวให้ผสม เพราะ mcp เรียงข้าม
  vault ด้วยฟิลด์นี้ ถ้า vault หนึ่งตอบเป็นหน่วย BM25 อีก vault ตอบเป็นหน่วย fused
  การเรียงจะไร้ความหมาย · ผสมอันดับเดียวรักษาลำดับเดิมเป๊ะ
- **degree boost ยังคูณกับคะแนน lexical ไม่ใช่คะแนน fused** เพราะคะแนน fused
  ห่างกันแค่ 1-2% การบวก 25% จะกระโดดหลายอันดับแทนที่จะแค่ตัดสินเสมอ
  ความเชื่อมโยงได้กัดครั้งเดียว บนอันดับที่มันถูกปรับมาให้พอดี
- โน้ตที่อันดับความหมายไม่คืนมาเลยได้ 0 จากด้านนั้น **ไม่ใช่ถูกกดลง** เพราะมัน
  ก็ยังเป็นผลที่ตรงคำ

### ที่ยืนยันไม่ได้บนเครื่องนี้ (ต้องพูดให้ชัด)
`cargo check --features semantic` ผ่าน แต่ **`cargo test` link ไม่ได้**:
ONNX Runtime ที่ `ort` โหลดมาถูกคอมไพล์ด้วย MSVC STL ใหม่กว่าที่ toolchain
เครื่องนี้มี → `LNK1120: 40 unresolved externals` ทั้งหมดเป็น `__std_*_element`
(SIMD algorithm symbols ของ STL ใหม่) — เป็นปัญหาสภาพแวดล้อม ไม่ใช่ของโค้ด
- แก้ด้วยการเพิ่ม **job `semantic` ใน CI บน ubuntu** ซึ่ง link ได้สะอาด →
  compile + link + เทสต์ vector store และการซอยชิ้น ถูกยืนยันที่นั่น
- **การโหลดโมเดลยังไม่ถูกรันจริงเลย** 120 MB จาก Hugging Face จะทำให้ CI ช้าและ
  ขึ้นกับเน็ต และผมไม่โหลดลงเครื่องเจ้าของโดยไม่ถาม → **เส้นทาง embed จริงกับ
  คุณภาพอันดับ semantic ยังไม่มีใครเห็นผล**

**Acceptance**: default build 151 tests + clippy + fmt ผ่าน · ทั้งสอง feature
configuration compile ได้ · README สองภาษาอธิบายสิ่งที่ต้องแลกตรงๆ

### Phase 25 ต่อ — ลองจริงแล้วเจอสิ่งที่เทสต์มองไม่เห็น (2026-07-30)

ยืนยันไม่ได้บนเครื่อง Windows (MSVC STL) → ติดตั้ง Rust ใน **WSL Ubuntu** clone
repo ลง ext4 แล้ว build ที่นั่น link สะอาดใน 3 นาที

**1. hybrid search ที่ผมเขียนไว้ตอบคำถามที่ฟีเจอร์นี้มีอยู่เพื่อแก้ไม่ได้เลย**
ถาม `การจัดเส้นทางแบบไดนามิก` กับ vault ที่ embed แล้ว 430 โน้ต → **`no results`**
เพราะ fusion เริ่มจากผล lexical แล้วให้ semantic แค่จัดลำดับใหม่ BM25 หาไม่เจอ
= ไม่มีอะไรให้จัด **151 tests + clippy + CI 3 job ผ่านหมดตอนที่บั๊กนี้อยู่ในโค้ด**
- แก้: `search::hits_for_keys()` ดึงโน้ตที่ semantic เจอแต่ BM25 ไม่เจอจาก index
  ด้วย key ตรงๆ แล้ว `search_vault` รวมเป็น **union** ก่อน RRF
- หลังแก้: ได้ `.../02-dynamic-routes.md` ซึ่ง**ไม่มีตัวอักษรร่วมกับคำค้นแม้ตัวเดียว**
  — สิ่งที่โมเดลอังกฤษล้วนทำไม่ได้ และเป็นเหตุผลที่เลือก multilingual

**2. ตัวเลขที่ผมอ้างผิด** README บอก ~120 MB โดยไม่ได้วัด ของจริง **465 MB**
(ONNX float32 470 MB + tokenizer 17 MB) — แก้แล้วและระบุว่าวัดจริง
- ถ้าจะลดในอนาคต: fastembed มี quantized variant สำหรับบางโมเดล แต่
  `MultilingualE5Small` ใช้ `onnx/model.onnx` ที่เป็น fp32

**3. reference notes เป็น opt-in** วัดได้ว่า embed 430 โน้ตใช้ **11m25s** และ 425
โน้ตเป็นเอกสาร Next.js คิดเป็น 95% ของเวลา — เหตุผลเดียวกับที่กราฟซ่อน reference
ใน Phase 23

| | เวลา | ผล |
|---|---|---|
| embed ทั้งหมด (เดิม) | **11m 25s** | 430 โน้ต |
| `samong embed` (ใหม่) | **26s** | 5 โน้ต + ข้าม 425 |
| รันซ้ำ (hash-gated) | **1.7s** | `430 already current` |

- ปิดแล้ว**ไม่ลบ** vector ที่เคยเขียน เพราะโน้ตอ้างอิงยังอยู่ใน scope และการทิ้ง
  งาน 11 นาทีเพราะ flag ขยับก็หยาบคายในแบบของมันเอง
- `doctor` นับ gap เทียบ **project notes เท่านั้น** ไม่งั้นจะเตือนค้างถาวรทุก vault
  ที่ปกติ ซึ่งเป็นคำเตือนที่คนเรียนรู้ที่จะเมิน แล้วมันจะกลบคำเตือนจริง

**ผลที่ดีที่สุด ได้จาก default ไม่ต้อง `--reference`**: ถาม `โลโก้ใช้สีอะไรได้`
→ เจอ **`brand/BRAND-ASSETS.md`** ซึ่งเป็นโน้ต*ภาษาอังกฤษ* และคำค้นภาษาอังกฤษ
`"what colours is the logo allowed to use"` **หาไม่เจอ** — ข้ามภาษาได้จริงบนโน้ตของเราเอง

**ที่ต้องซื่อสัตย์**: semantic เพิ่ม noise ด้วย RRF แบบไม่มี threshold ยอมให้ผล
อันดับ 1 ของฝ่าย semantic เข้ามาเสมอแม้ cosine ไม่สูง (เคยเห็น
`04-community/index.md` ขึ้นอันดับ 2 ของคำถามเรื่องโลโก้) · ถ้าจะแก้ต้องมีเกณฑ์
ความคล้ายขั้นต่ำ ซึ่งเป็นค่าเฉพาะโมเดลและเดามั่วไม่ได้ ต้องวัดจากข้อมูลจริงก่อน

## Phase 26 — หน้าบ้านที่ GitHub เห็น (เพิ่มหลัง Phase 25)

**เจ้าของทักว่า "README ที่ GitHub ยังไม่อัปเดต รวมทั้งหน้าจอ" — ถูกทั้งสองอย่าง**

**1. screenshot เป็นของยุค "Banyan"** ภาพแรกที่คนเห็นบน GitHub แสดงชื่อเก่า
โลโก้ต้นไทรเก่า สีเขียวเก่า layout สามคอลัมน์ที่ลบไปตั้งแต่ Phase 16 UI ภาษาไทย
(Phase 20 เปลี่ยนเป็นอังกฤษ) และ `Ctrl K` command palette ที่ไม่มีอยู่แล้ว
- [x] ถ่ายใหม่ด้วย **headless Edge** (`--screenshot` + `--virtual-time-budget`)
      สคริปต์ไว้ที่ `docs/screenshots.ps1` เพื่อให้ถ่ายซ้ำได้ตอน UI เปลี่ยน —
      สาเหตุที่มันเก่าคือไม่มีใครถ่ายซ้ำได้ง่ายๆ
- [x] **vault ตัวอย่างที่สคริปต์ไว้** `docs/demo-vault.sh` 18 โน้ตที่ลิงก์กันจริง
      มีโฟลเดอร์ และมีโน้ตไทย 2 ไฟล์ — เพราะ docmind มีโน้ตโปรเจกต์ 5 ไฟล์
      กราฟจึงเป็นจุดเล็กในผืนว่าง ถ่ายออกมาไม่ได้เรื่อง
- [x] ลบ `docs/editor-*.png` ยุค Banyan และเปลี่ยนภาพเปิดเป็นกราฟ ซึ่งเป็น
      home surface จริงตั้งแต่ Phase 16

**2. กราฟกินพื้นที่แค่ ~15% ของ workspace ตอนโหลด** — เห็นครั้งแรกจากการถ่ายภาพ
กล้องอยู่ที่ 1:1 รอบจุดกำเนิด vault 18 โน้ตจึงเป็นจุดเล็กกลางจอว่าง
- [x] auto-fit: คำนวณ bbox ของ node แล้วตั้ง camera ให้พอดี **ทุก tick จนกว่า
      ผู้ใช้จะ pan/zoom/ลากเอง** แล้วหยุดถาวร
- **ลองผิดสองรอบ ซึ่งเป็นเหตุผลที่ต้องดูภาพจริง**: (ก) clamp `k` ไว้ที่ `1x`
      ด้วยเหตุผลว่า "vault เล็กไม่ควรถูกขยาย" ผลคือไม่ fit อะไรเลยเพราะ cloud
      เล็กกว่า viewport อยู่แล้ว → เปลี่ยนเป็น `0.2..2.5` (ข) fit ครั้งเดียวตอน
      `simulation.on("end")` ไปวัดตอน layout ยังหุบตัวอยู่ กล้องจึงล็อกกับขนาด
      ที่กราฟไม่มีแล้ว **ภาพออกมาเล็กกว่าเดิม** → fit ทุก tick
- ผลพลอยได้: `k > 1.15` ทำให้ label ขึ้นเอง README จึงอ่านชื่อโน้ตได้

**3. `doctor` ในเว็บไม่รู้เรื่อง embeddings** CLI รายงานแล้วแต่หน้าจอไม่
- [x] `DoctorResponse.embeddings` เป็น **nullable** ไม่ใช่ศูนย์ เพื่อให้ UI แยก
      "build นี้ทำ semantic ไม่ได้" ออกจาก "ทำได้แต่ยังไม่ embed" — สองสถานะที่
      คำตอบต่างกันคนละเรื่อง · VaultHealth แสดงโมเดล จำนวน และช่องว่าง
      โดยนับ gap เทียบ project notes เท่านั้น

**4. README ที่ค้าง**
- [x] ตาราง CLI **ไม่มี `samong embed`** ทั้งสองภาษา
- [x] ตัวอย่าง checksum ยังเป็น `v0.3.0` → `v0.3.2`
- [x] Roadmap ยังบอกว่า "เปิด public" และ "ปุ่มเพิ่ม vault ในหน้าเว็บ" เป็นแผน
      ทั้งที่ทำเสร็จแล้ว → เขียนใหม่ให้เป็นสิ่งที่เหลือจริง (similarity floor,
      โมเดลเล็กลง, server กลางที่ index จาก git)

**5. ป้องกันแบรนด์ตอนคน clone** — `brand.html` ประกาศว่าโลโก้ไม่อยู่ใต้ Apache-2.0
แต่ `LICENSE` ที่รากครอบทั้งต้นไม้ และใน `site/brand/` ไม่มีอะไรบอกเลย
- [x] `site/brand/LICENSE` วางข้อยกเว้นไว้**ข้างไฟล์** วิธีเดียวกับ Rust/Kubernetes
      — Apache-2.0 §6 ไม่ให้สิทธิ์เครื่องหมายการค้า แต่ §2 ให้สิทธิ์ลิขสิทธิ์เหนือ
      artwork อย่างกว้าง คนที่ clone ไปไม่มีทางเดาได้ · ระบุสิ่งที่ทำได้เลย
      (nominative use) และที่ทำไม่ได้ · ไม่แก้ตัวบท LICENSE เพราะจะทำให้
      license scanner สับสน ชี้จาก `NOTICE` ที่ §4(d) บังคับให้เดินทางไปด้วย

**6. `workflow_dispatch` ใน release.yml** — เดิม tag push เป็นทางเดียว job ที่ล้ม
จึงต้องเผา version ใหม่ ซึ่งเกิดกับ v0.3.0 และ v0.3.1 จริง
- [x] `env.TAG: ${{ inputs.tag || github.ref_name }}` เป็นแหล่งเดียว ใช้ทั้งใน
      `checkout ref` (ไม่งั้น manual run จะ build default branch แล้วติดป้ายชื่อ
      tag ที่ไม่ได้ build มา), ชื่อ archive, และ `tag_name` ของ upload

**ที่ยังไม่แก้**: label บนกราฟทับกันในบริเวณหนาแน่น เห็นชัดในภาพที่ถ่าย —
ต้องมี label placement หรือซ่อนเมื่อชนกัน เป็นงานของตัวเอง


## Phase 27 — vault ที่ส่งต่อให้คนอื่นได้ (เพิ่มหลัง Phase 26)

**ที่มา**: คำถามว่า "ถ้ามีคนได้ vault ของเราไป เขาจะรู้อะไรบ้าง" นำไปสู่การทดลอง
จริงที่พบสองอย่าง แล้วต่อด้วยทิศทางที่ vault จะถูกส่งต่อหรือขายกันเอง

**สิ่งที่ทดลองแล้วพบ (ไม่ใช่การเดา)**
1. index **เก็บสำเนาข้อความทุกโน้ตแบบเต็ม** — `search.rs` เรียก `.set_stored()`
   ทั้ง title และ body (จำเป็นสำหรับ snippet) grep ไม่เจอเพราะ LZ4 **compress
   ไม่ใช่ encrypt**
2. **ชื่อโน้ตที่ลบแล้วยังค้างใน `graph.redb`** — ลบไฟล์ ลบลิงก์ที่ชี้หามัน แล้ว
   reindex · `samong broken` บอกถูกว่าไม่มีลิงก์ค้าง แต่ bytes ยังอยู่ เพราะ redb
   เป็น copy-on-write: page ที่ปล่อยแล้วไม่ได้ถูกเขียนศูนย์ทับ

→ **คนที่เตรียม vault ขายโดยลบโน้ตส่วนตัวออกก่อน จะส่งชื่อโน้ตที่ลบไปด้วย**
นี่เป็นพฤติกรรมปัจจุบัน ไม่ใช่ความเสี่ยงเชิงทฤษฎี

- [x] **`samong pack <dir>`** — เป็น **whitelist ไม่ใช่ copy-แล้ว-ลบ** เพราะความ
      พังที่กันคือ "ส่งสิ่งที่ไม่รู้ว่ามีไปด้วย" และ blacklist กันได้แค่สิ่งที่
      มีคนนึกออกแล้วเท่านั้น · ออกเฉพาะ `.md` ที่อยู่ใน scope กับ `samong.toml`
- [x] **บังคับให้มี `license`** ก่อน pack — การเผยแพร่โดยไม่บอกว่าคนอื่นทำอะไรได้
      คือความผิดพลาดที่ถอยกลับยากที่สุด จึงหยุดที่คำสั่ง ไม่ใช่ที่การรีวิว
- [x] **ตัด reference notes ออกโดยค่าเริ่มต้น** — เป็นเอกสารของคนอื่นและ license
      จำนวนมากห้ามแจกต่อ · `--include-reference` เปิดได้แต่เตือนตรงๆ
- [x] ข้อจำกัดที่**บอกไว้ ไม่ซ่อน**: ไฟล์แนบ (รูป, PDF) ไม่ใช่ `.md` จึงไม่ถูกคัดลอก

**manifest ที่นอนอยู่เฉยๆ มาตั้งแต่ Phase 10 ถูกปลุกแล้ว**
`[vault] description/version/license/source` มีมาตั้งแต่ Phase 10 ในฐานะ forward
compatibility แต่ `grep` แล้วไม่มีโค้ดไหนอ่านเลยนอกจาก `name`
- [x] `samong doctor` แสดงทั้งสี่ฟิลด์ · `/api/vaults/{vault}/doctor` คืน
      `manifest` · VaultHealth ในเว็บแสดงเมื่อมีค่า
- [x] เตือนเรื่อง license **เฉพาะ vault ที่ประกาศ `source` แล้ว** — คือ vault ที่
      ตั้งใจให้เดินทาง · vault ส่วนตัวไม่ต้องโดนบ่น

**Acceptance**: 158 tests (เพิ่ม 7) + clippy + fmt ผ่าน · เทสต์ที่สำคัญที่สุดคือ
`a_deleted_notes_title_does_not_reach_the_packed_copy` ซึ่ง assert precondition ว่า
ชื่อโน้ตยังอยู่ใน index จริงก่อน แล้วจึงพิสูจน์ว่ามันไม่หลุดเข้าไฟล์ที่ pack ออกมา

## Phase 28 — ติดตั้ง vault ของคนอื่น (เพิ่มหลัง Phase 27)

`samong vault install <git-url>` และ `samong vault update [name]`

**ตัดสินใจว่าของที่ติดตั้งมาเป็น reference notes ไม่ใช่ vault ที่ลงทะเบียนแยก**
- ใช้กลไกที่สร้างไว้ตอน Phase 13 สำหรับเอกสารของ dependency ซ้ำ ไม่ประดิษฐ์
  vault ชนิดที่สอง — และเป็นการตัดสินใจเดียวกัน: หนึ่งโปรเจกต์ หนึ่งสมอง
- **read-only ไม่ใช่การตกแต่ง**: การแก้จะถูก `update` ลบทิ้งรอบหน้า และเนื้อหา
  ไม่ใช่ของผู้อ่านที่จะแก้ · `reject_reference_write` กันทุกทางเขียนอยู่แล้ว
- ผลคือของที่ซื้อมาอยู่ใน **กราฟเดียวกัน การค้นเดียวกัน และ `[[link]]` เดียวกัน**
  กับโน้ตที่เขียนเอง เทสต์พิสูจน์ทั้งสามอย่าง

**เรียก `git` CLI ไม่ฝัง git library — เพราะเรื่อง authentication**
vault ที่ขายอยู่ใน private repo การเข้าถึงคือ SSH agent, credential helper,
hardware key, SSO device flow, 2FA — ทุกอย่างที่ผู้ใช้ **ตั้งค่าให้ `git` ไว้แล้ว**
ถ้าฝัง library เราต้องเขียนใหม่ให้แย่กว่า และคนแรกที่เจอ config ที่เราไม่ได้คิดถึง
คือคนที่จ่ายเงินแล้วเข้าไม่ถึงของ · ต้นทุนคือ git ต้องมีในเครื่อง กับข้อความที่
ชัดเจนเมื่อไม่มี

**`.gitignore` อัตโนมัติ — ค่าเริ่มต้นต้องไม่ใช่อันที่ทำให้คนเดือดร้อน**
ถ้าไม่ทำ ผู้ซื้อจะ commit โน้ตของคนอื่นขึ้น repo ตัวเอง ซึ่งสำหรับ vault ที่ซื้อมา
คือการละเมิด license ที่เขาไม่ได้เลือกทำ · เขียนเหตุผลกำกับไว้ในไฟล์ด้วย ไม่งั้น
มีคนลบบรรทัดนั้นทิ้งเพราะไม่รู้ว่ามันมีไว้ทำไม

**แก้ `samong.toml` ด้วย `toml_edit` ไม่ใช่ `toml`**
เป็น dependency ใหม่ตัวเดียวของ phase นี้ · `samong.toml` เป็นไฟล์ที่คนเขียนเอง
และใส่คอมเมนต์ การ parse แล้ว serialize กลับด้วย `toml` จะคืนไฟล์ที่คอมเมนต์และ
ลำดับหายหมด — config ที่คนแก้ด้วยมือต้องรอดจากการที่เราแก้มัน · มีเทสต์ยืนยันว่า
คอมเมนต์ยังอยู่

**provenance ไม่ต้องเก็บที่ไหนเลย** — `installed()` หา include root ที่เป็น git
checkout เอา · checkout คือบันทึกที่มาของตัวมันเอง registry entry มีแต่จะ drift
ออกจากความจริง จึงไม่แตะ schema ของ registry

- [x] บอก description / version / license **ตอนติดตั้ง** ไม่ใช่ฝังในไฟล์ที่ผู้ซื้อ
      อาจไม่เคยเปิด
- [x] `update` ที่ vault หนึ่งเข้าไม่ได้ (subscription หมดอายุ) **ไม่หยุดตัวอื่น** —
      การหมดอายุเป็นเรื่องปกติ ไม่ใช่ข้อยกเว้น
- [x] URL ที่ชี้ไปนอก vault ถูกปฏิเสธ ไม่ใช่ sanitize — การ sanitize เชิญชวนให้
      เถียงกันว่ามันครบหรือยัง

**เทสต์จับสิ่งที่ผมคิดผิดสองข้อ**: `name_from_url` กับ URL ที่ไม่มี path คืนชื่อ
host ซึ่งเป็นชื่อที่แย่แต่ไม่อันตราย (แก้ความคาดหวังในเทสต์ ไม่ใช่แก้โค้ด) และ
`toml::Value` ใน toml 1.x parse ค่าเดี่ยว ไม่ใช่ทั้งเอกสาร ต้องใช้ `from_str::<Table>`

**Acceptance**: 169 tests (เพิ่ม 11) + clippy + fmt ผ่าน · phase28 รันกับ git จริง

## Phase 29 — พิสูจน์ว่า vault เป็นของคนที่บอกว่าเป็นคนทำ และบอกที่มาตรงที่คนอ่าน

`samong vault verify` + ที่มาของโน้ตในผลค้นหาทุกช่องทาง

**ผมแนะนำ `git tag -s` ไว้ในรอบก่อน — ผิด**
`vault update` เดินตาม branch ผู้อ่านจึงรับ commit ที่อยู่ระหว่าง tag และ
ลายเซ็นบน tag ไม่ได้พูดถึง commit ที่เพิ่ง pull มาเลย · สิ่งที่ถูกคือ **เซ็น commit**
(`commit.gpgsign true`) ทุกอัปเดตจึงมีเจ้าของในตัวมันเอง

**ไม่ทำ checksum manifest — และนั่นคือการตัดสินใจ ไม่ใช่การข้าม**
vault ที่ติดตั้งแล้วคือ git checkout ทุกไบต์อยู่ใน commit hash ซึ่งเป็น Merkle tree
ที่ git ตรวจซ้ำทุกครั้งอยู่แล้ว · เขียน `SHA256SUMS` วางข้างเนื้อหาคือพูดสิ่งเดิม
ให้**อ่อนลง**: คนที่แก้โน้ตได้ก็แก้ไฟล์ checksum ที่วางอยู่ข้างๆ ได้ · digest ที่ไม่มี
ลายเซ็นครอบไม่ใช่มาตรการความปลอดภัย มันคือสำเนาที่สองของสิ่งที่เราสงสัย
· สิ่งที่ขาดจริงคือ **authenticity** ไม่ใช่ integrity

**pin กุญแจตอน install แบบ SSH pin host key**
การตรวจลายเซ็นมีความหมายก็ต่อเมื่อผู้อ่านรู้ว่าควรเจอกุญแจไหน และไม่มี registry
ไหนบอกได้ — ครั้งแรกคนที่ให้ URL มาคือผู้มีอำนาจ · เก็บ pin ไว้ใน `git config`
ของ clone เอง เป็นการตัดสินใจเดียวกับ Phase 28: **checkout คือบันทึกที่มาของตัวมันเอง**
บันทึกที่เก็บไว้ที่อื่นมีแต่จะ drift ออกจากสิ่งที่มันอธิบาย

- [x] ตรวจ**ก่อน merge** ไม่ใช่หลัง pull — คำเตือนที่พิมพ์ตอนเนื้อหาลงดิสก์และเข้า
      index ไปแล้ว คือรายงาน ไม่ใช่ทางเลือก · เทสต์ยืนยันว่า commit ที่ถูกปฏิเสธ
      ไม่โผล่ใน working tree
- [x] **หยุดเซ็น = เปลี่ยนกุญแจ** ไม่งั้นวิธีโจมตีที่ถูกที่สุดคือเลิกเซ็นเฉยๆ
- [x] **ไฟล์ untracked นับเป็นความเปลี่ยนแปลง** — `.md` ที่ใครหย่อนใส่ vault ที่ติดตั้งไว้
      จะถูก index และโผล่ในผลค้นหาโดยถูกเครดิตให้ผู้ขาย
- [x] vault ที่ไม่ได้เซ็น **ผ่าน** โดยปริยาย — วันนี้แทบทุก vault ในโลกไม่ได้เซ็น
      การตรวจที่ fail ทุกครั้งคือการตรวจที่ไม่มีใครรัน · `--require-signature` มีไว้
      ให้คนที่ตัดสินใจเป็นอย่างอื่น

**ที่มาต้องเดินทางมากับผลลัพธ์ ไม่ใช่ให้ไปเปิดดูทีหลัง**
ผู้ติดตั้งเป็นคนเดียวที่รู้ว่า `vendor/h/` คืออะไร และเขาไม่ใช่คนเดียวที่อ่านผลค้นหา ·
ติดที่ `ops::search_vault` ที่เดียว เพราะเป็นทางผ่านของทั้ง CLI/HTTP/MCP —
attribution ที่สาม interface ต้องจำเองคือ attribution ที่จะมีสักตัวลืม · **ช่วงเวลาที่
อันตรายจริงไม่ใช่ตอนค้น แต่คือตอนที่ใครสักคนก็อปย่อหน้าออกจากผลลัพธ์ไปไว้ในโน้ต
ตัวเอง** หลังจากนั้นไม่มีอะไรจำได้อีกว่ามันมาจากไหน
- ไม่ระบุ license → เขียนว่า `licence not stated` ไม่ใช่เว้นว่าง มันคือคำตอบ ไม่ใช่ช่องว่าง
- MCP สำคัญที่สุด: agent คือสิ่งที่มีโอกาสมากที่สุดที่จะยกเนื้อหาที่ซื้อมาไปวางในโน้ตผู้ใช้

**Acceptance**: 188 tests (เพิ่ม 19) + clippy + fmt ผ่าน · เทสต์ ssh-signed รันจริง
ไม่ skip · ตรวจ badge ในเบราว์เซอร์ด้วย geometry (ไม่ล้นกล่อง ไม่ทับ path บรรทัดเดียว)

## Phase 30 — ดับเบิลคลิกแล้วเปิดได้

`samong-app` — launcher ที่ไม่ต้องพิมพ์อะไรเลย

**สมมติฐานเรื่อง terminal เป็นการเลือกกลุ่มผู้ใช้ไปแล้ว โดยที่เราไม่ได้เลือก**
คนที่โน้ตต้องอยู่ในเครื่องมากที่สุด — ทนาย หมอ นักวิจัย คนที่ถือความลับของคนอื่น —
คือคนที่**ไม่พิมพ์ `samong vault add`** · ประตูเดียวที่เป็น command line เท่ากับ
ตัดสินใจแล้วว่า product นี้เพื่อใคร · phase นี้คือ first run ทั้งอันที่ไม่มีอะไรให้ตอบ

**สามอย่างที่ launcher ตัดสินใจให้**
1. ถ้า Samong เปิดอยู่แล้ว → **พาไปที่หน้าต่างเดิม** ไม่เปิด server ตัวที่สอง
   สองตัวจะ index vault เดียวกันและแย่ง redb lock กัน
2. ถ้าพอร์ตถูกยึดโดย**คนอื่น** (3000 คือพอร์ตที่มีคนแย่งที่สุดในวงการ) → เลื่อนขึ้น
   ไม่ใช่ fail · ตรวจว่าเป็น Samong จริงด้วย `/api/vaults` ที่ต้องตอบ JSON array —
   เปิดเบราว์เซอร์ไปที่แอปคนอื่นแย่กว่าขึ้น error
3. ถ้าไม่มี vault → **สร้างให้ พร้อมโน้ตข้างใน** first run ที่ลงเอยที่หน้าจอเปล่า
   ไม่ได้อธิบายอะไรเลย

**สองโน้ต ไม่ใช่หนึ่ง และลิงก์ถึงกัน**
สิ่งแรกที่คนใหม่เห็นคือกราฟ · โน้ตเดียววาดจุดเดียวซึ่งไม่ได้สาธิตอะไร สองโน้ต
กับหนึ่งเส้นวาด**ตัวความคิดของโปรแกรม** และโน้ตที่สองไปถึงได้ด้วยการกดลิงก์
ซึ่งเป็น interaction เดียวที่ควรเรียน · ไม่ทับไฟล์ที่มีอยู่ เพราะโฟลเดอร์นั้นอาจ
เป็นของเขาอยู่แล้ว

**default folder ไม่ใช่ folder picker**
picker ต้องใช้ native dialog ซึ่งลาก GTK มาบน Linux และต้องถูกทั้งสาม platform
ก่อนจะมีใครได้เห็นโน้ตแรก · และ first run ไม่ใช่จังหวะที่จะถามอยู่ดี — คนที่เพิ่ง
ดับเบิลคลิกโปรแกรมที่ไม่รู้จักไม่มีข้อมูลจะตอบว่า "โน้ตคุณควรอยู่ที่ไหน"

**windows subsystem = 2 ไม่ใช่ 3**
console binary ที่เปิดจาก Explorer จะโผล่หน้าต่างดำที่อยู่ค้าง และปิดมันคือ
ฆ่าโปรแกรม · หน้าต่างนั้นคือสัญญาณที่ชัดที่สุดว่า "นี่ไม่ใช่แอปจริง" · ราคาที่จ่าย
คือไม่มีที่พิมพ์อะไรได้เลย จึงต้องมี `launcher.log` ที่**เปิดขึ้นมาให้ดูเองเมื่อพัง** —
เงียบไปเลยจะไปตกกับคนที่หาสาเหตุเองไม่ได้ที่สุด

**"เปิดได้" ต้องมาพร้อม "ปิดได้"** → `POST /api/shutdown` + ปุ่ม `⏻`
server ที่เปิดจาก launcher ไม่มี terminal ให้กด Ctrl+C ถ้าไม่ทำ ทางเดียวคือ
Task Manager · ใช้ `with_graceful_shutdown` เพื่อให้เบราว์เซอร์**ได้คำตอบก่อน
socket ปิด** ไม่งั้นหน้าเว็บรายงาน network error สำหรับ request ที่ทำงานสำเร็จ ·
`notify_one` ไม่ใช่ `notify_waiters` เพราะเก็บ permit ไว้ได้ถ้า serve loop ยังไม่รอ

**บั๊กที่เจอเพราะเปิดใช้จริง: parser ของ `[[ ]]` กินข้ามบรรทัด**
โน้ตต้อนรับอธิบายเรื่อง wikilink จึงมี `` `[[` `` เปล่าๆ อยู่บรรทัดหนึ่ง และลิงก์จริง
อยู่อีกสองบรรทัดถัดไป · pattern เดิม match ตั้งแต่ `[[` ตัวแรกไปถึง `]]` ตัวท้าย
แล้ว**วาด node ที่ label เป็นย่อหน้า** — เป็นสิ่งแรกที่ผู้ใช้ใหม่จะเห็นบนแผนที่ ·
แก้เป็นห้ามข้าม newline ซึ่งเป็นกฎเดียวกับ Obsidian (สำคัญกว่าเหตุผลของเรา:
โน้ตต้องอ่านได้ทั้งสองที่ ลิงก์ที่ resolve ที่หนึ่งแต่ไม่ resolve อีกที่แย่กว่าไม่มีลิงก์)
· `rewrite_wikilinks` แก้ตามด้วย ไม่งั้น rename จะพลาดบางอันและทำอันอื่นเสีย

**Acceptance**: 199 tests (เพิ่ม 11) + clippy + fmt ผ่าน · เทสต์ยิง binary จริง
โดยไม่มี argument เลย ด้วย HOME ปลอม · ยืนยัน PE subsystem = 2 จาก header จริง
(ไม่ใช่เชื่อว่า `cfg_attr` ทำงาน) · รันเองครบวง: สร้าง vault → กราฟ 2 node 2 edge
ไม่มี node ขยะ → กดปุ่ม `⏻` → process ออก → พอร์ตปิด

**หนี้ที่ยอมรับ**: ไม่มี icon (brand เป็น SVG ต้องมี rasteriser ใน CI + build script
สำหรับ Windows resource) · ไม่ได้ code-sign ทั้ง `.app` และ `.exe` ผู้ใช้ครั้งแรก
ต้องคลิกขวา → Open บน macOS · **นี่คือกำแพงที่เหลืออยู่สูงที่สุดสำหรับกลุ่มที่
launcher นี้มีไว้เพื่อเขา และมันแก้ด้วยเงิน ไม่ใช่ด้วยโค้ด**
