# ใช้ Samong เป็นมันสมองของ AI agent 🧠

Samong ถูกออกแบบให้เป็น "ความจำถาวร" ของ AI coding agent — agent ค้นความรู้เก่า
ก่อนทำงาน แล้วบันทึกสิ่งที่เรียนรู้ใหม่กลับเข้า vault วนเป็นวงจร

## ทางที่ 1: MCP (แนะนำ)

`samong-mcp` เป็น MCP server บน stdio — เครื่องมือที่รองรับ MCP จะเห็น Samong เป็น
tools ในตัว

### อ่านสองบรรทัดนี้ก่อน จะประหยัดเวลามาก

**MCP เป็นความสามารถของ "ไคลเอนต์" ไม่ใช่ของ "โมเดล"** — คำถามที่ถูกไม่ใช่
"GPT ต่อ MCP ได้ไหม" แต่คือ "โปรแกรมที่ผมใช้คุยกับ GPT ต่อ MCP ได้ไหม" โมเดล
เดียวกันจึงต่อได้หรือไม่ได้ ขึ้นกับว่าคุณเรียกใช้มันผ่านอะไร

**`samong-mcp` รันบนเครื่องคุณผ่าน stdio** ⇒ ต่อได้เฉพาะไคลเอนต์ที่รันบนเครื่อง
เดียวกัน · **แอปบนเว็บล้วนๆ ต่อไม่ได้** (ChatGPT บนเว็บ, Gemini บนเว็บ, Grok ใน X)
เพราะมันอยู่บนเซิร์ฟเวอร์ของเขา ไปเรียกโปรเซสในเครื่องคุณไม่ได้ ตัวเชื่อมแบบ
connector ของพวกนั้นต้องการ MCP server ที่เป็น URL สาธารณะ ซึ่งขัดกับสิ่งที่
Samong เป็น — โน้ตไม่ออกจากเครื่อง

### ตารางสรุป

| ไคลเอนต์ | โมเดล | คำสั่งเพิ่ม | ไฟล์ตั้งค่า |
|---|---|---|---|
| Claude Code | Claude | `claude mcp add --scope user samong -- samong-mcp` | `.mcp.json` |
| Claude Desktop | Claude | ติดตั้งไฟล์ `.mcpb` จากหน้า release | `claude_desktop_config.json` |
| Codex CLI / ChatGPT desktop | GPT | `codex mcp add samong -- samong-mcp` | `~/.codex/config.toml` |
| Gemini CLI | Gemini | `gemini mcp add samong samong-mcp` | `~/.gemini/settings.json` |
| Qwen Code | Qwen | `qwen mcp add samong samong-mcp` | `~/.qwen/settings.json` |
| Kimi Code CLI | Kimi | `kimi mcp add samong -- samong-mcp` | จัดการผ่าน `kimi mcp` |
| Grok Build | Grok | `grok mcp add samong -- samong-mcp` | `~/.grok/config.toml` |
| GLM (Z.ai) | GLM | ใช้ผ่านไคลเอนต์อื่น — ดูด้านล่าง | ของไคลเอนต์นั้น |
| DeepSeek | DeepSeek | ใช้ผ่านไคลเอนต์อื่น — ดูด้านล่าง | ของไคลเอนต์นั้น |

> ถ้ายังไม่ได้ `cargo install` ให้ใส่ path เต็มของไบนารีแทนคำว่า `samong-mcp` เช่น
> `C:\path\to\samong\target\release\samong-mcp.exe`

### รูปแบบไฟล์ตั้งค่า

ไคลเอนต์ตระกูล JSON — **Claude Code, Gemini CLI, Qwen Code** ใช้โครงเดียวกันเป๊ะ
ต่างกันแค่ที่อยู่ของไฟล์:

```json
{
  "mcpServers": {
    "samong": { "command": "samong-mcp" }
  }
}
```

ไคลเอนต์ตระกูล TOML — **Codex CLI และ Grok Build**:

```toml
[mcp_servers.samong]
command = "samong-mcp"
```

> Codex ใช้คีย์ `mcp_servers` (ขีดล่าง) ไม่ใช่ `mcpServers` แบบ JSON — ตัวสะกดนี้
> พลาดกันบ่อยและมันจะเงียบ ไม่ฟ้องว่าตั้งค่าผิด

### GLM, DeepSeek และโมเดลที่ไม่มีไคลเอนต์ของตัวเอง

สองตัวนี้เป็น **โมเดล** ไม่ใช่ไคลเอนต์ MCP — คุณเรียกใช้มันผ่านโปรแกรมอื่น และ
**การตั้งค่า MCP เป็นของโปรแกรมนั้น ไม่ใช่ของโมเดล**

- **GLM Coding Plan ของ Z.ai** ออกแบบมาให้เสียบเข้ากับไคลเอนต์ที่มีอยู่แล้ว เช่น
  Claude Code, Cline, OpenCode ⇒ ตั้ง Samong ตามไคลเอนต์นั้นตามปกติ แล้ว GLM จะ
  เห็น tools ของ Samong เหมือนกัน
- **DeepSeek** ไม่มี CLI ของตัวเองที่เป็น MCP client ⇒ ใช้ผ่านไคลเอนต์ที่ตั้ง
  โมเดลเองได้และรองรับ MCP เช่น Cline, Continue, OpenCode, Zed

หลักเดียวกันใช้กับโมเดลอื่นๆ ที่ยังไม่มีในตาราง: **หาไคลเอนต์ที่รองรับ MCP ก่อน
แล้วค่อยเลือกโมเดล**

### ตรวจว่าต่อติดจริง

อย่าเชื่อว่าตั้งค่าแล้วแปลว่าใช้ได้ — ถามมันตรงๆ:

```
list_vaults ของ samong คืนอะไรบ้าง
```

ถ้าไคลเอนต์ตอบเป็นรายชื่อ vault แปลว่าต่อติด ถ้าบอกว่าไม่รู้จัก tool นี้ แปลว่า
ยังไม่ติด — ส่วนใหญ่เพราะ `samong-mcp` ไม่ได้อยู่ใน `PATH` ของโปรเซสที่ไคลเอนต์
รันขึ้นมา ซึ่งมักไม่ใช่ `PATH` เดียวกับเทอร์มินัลของคุณ ใส่ path เต็มแล้วหายไป

### Tools ที่ agent ได้

| Tool | หน้าที่ |
|---|---|
| `list_vaults` | รายชื่อ vault ทั้งหมด |
| `list_notes` | รายชื่อโน้ตใน vault เป็น **path** (มี `[reference]` ต่อท้ายถ้าเป็นโน้ตอ่านอย่างเดียว) |
| `read_note` | อ่านเนื้อหา markdown — อ้างด้วย **path** เช่น `docs/API.md` |
| `save_note` | สร้าง/แก้โน้ต (ใส่ `[[ลิงก์]]` เชื่อมความรู้ได้); ปฏิเสธถ้าเป็น reference note จาก `scope.include` เพราะไฟล์เป็นของ dependency เขียนไปก็หายตอน install ใหม่ |
| `search_notes` | ค้น full-text — ภาษาไทยตัดคำให้ ค้นกลางประโยคเจอ; `limit` คุมจำนวนผล (default 8 = ประหยัด token, **นับรวมทุก vault** ไม่ใช่ต่อ vault) |
| `get_links` | ดูความเชื่อมโยงของโน้ต (forward/backlinks/ข้าม vault) — อ้างด้วย path |

> **โน้ตอ้างด้วย path ไม่ใช่ title** เพราะ vault เดียวมีไฟล์ชื่อ `README.md` ได้หลายไฟล์
> ให้เอา path จาก `list_notes` หรือ `search_notes` มาใช้ต่อ อย่าเดาเอง

**ตั้งใจไม่มี tool ลบโน้ต** — มันสมองของ agent ควรสะสมความรู้ ไม่ควรลบเองได้
การลบเป็นเรื่องของมนุษย์ผ่าน CLI หรือ Web UI

## ทางที่ 2: CLI (ไม่ต้องตั้งค่าอะไร)

Agent ที่รันคำสั่ง shell ได้ ใช้ Samong ได้ทันที:

```sh
samong search --all-vaults "jwt refresh token"   # ค้นก่อนเริ่มงาน
samong new "บทเรียน: redb lock"                   # บันทึกความรู้ใหม่
samong links "สถาปัตยกรรม auth"                   # ดูความเชื่อมโยง
```

## Recipe: วางวงจรความรู้ใน CLAUDE.md

คัดลอกบล็อกนี้ลง `CLAUDE.md` ของโปรเจกต์คุณ (ปรับชื่อ vault ตามจริง):

```markdown
## Knowledge base (Samong)

มันสมองถาวรของโปรเจกต์นี้อยู่ใน Samong vault ชื่อ `my-project`

**ก่อนเริ่มงานชิ้นใหญ่**: ค้นความรู้เดิมก่อนเสมอ
- MCP: เรียก `search_notes` ด้วยหัวข้อที่เกี่ยวข้อง (ไทย/อังกฤษได้ทั้งคู่)
- ผลลัพธ์คือ `vault/path` — เอา path นั้นส่งให้ `read_note` ต่อได้ตรงๆ

**หลังตัดสินใจสำคัญหรือแก้ปัญหายาก**: บันทึกกลับเข้า vault ด้วย `save_note`
- ตั้ง path สั้นกระชับ เช่น `การตัดสินใจ: เลือก redb แทน sled.md`
  (จัดกลุ่มด้วยโฟลเดอร์ได้ เช่น `decisions/redb.md`)
- ในเนื้อหาใส่ [[ลิงก์]] ไปโน้ตที่เกี่ยวข้อง เพื่อให้กราฟความรู้เชื่อมกัน
- บันทึก: บริบท ณ ตอนนั้น, ทางเลือกที่พิจารณา, เหตุผลที่เลือก, ข้อควรระวัง

**ห้าม**: ลบหรือเขียนทับโน้ตเดิมโดยไม่อ่านก่อน — ถ้าข้อมูลเก่าผิด
ให้เขียนโน้ตใหม่ที่อ้างถึงของเดิมแล้วอธิบายว่าอะไรเปลี่ยน
```

## ตัวอย่างวงจรที่เกิดขึ้นจริง

1. คุณสั่ง: "เพิ่ม rate limiting ให้ API"
2. Agent เรียก `search_notes("rate limiting")` → เจอ
   `my-project/decisions/middleware stack.md` ที่เคยบันทึกไว้
3. Agent อ่านแล้วรู้ว่าโปรเจกต์นี้ใช้ tower layers → เขียนโค้ดถูกแนวตั้งแต่แรก
4. เสร็จงาน agent เรียก `save_note` ที่ `decisions/rate limiting.md` บันทึกว่า
   "ใช้ governor crate เพราะ..." พร้อมลิงก์ [[middleware stack]]
5. เดือนหน้า agent ตัวไหนก็ตาม (หรือคุณเอง) ค้นเจอความรู้นี้ทันที
