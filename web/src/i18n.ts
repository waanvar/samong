import { useMemo, useSyncExternalStore } from "react";

/**
 * Interface language.
 *
 * Samong's one unfair advantage is that it segments Thai, so Thai has to be a
 * first-class language here — but the product is for anyone, and an English
 * default is what makes it openable by someone who arrives from the website.
 * English is therefore the source of truth: every key is declared in `en` first,
 * and `th` is type-checked against it, so a missing translation is a build
 * error rather than a blank label someone discovers in production.
 *
 * No i18n library. Two locales and ~70 strings do not need a plural-rule engine
 * or a message parser, and the whole UI ships embedded inside a Rust binary
 * where every kilobyte is a kilobyte someone downloads.
 */
export type Locale = "en" | "th";

export const LOCALES: { code: Locale; label: string }[] = [
  { code: "en", label: "English" },
  { code: "th", label: "ไทย" },
];

/**
 * A message that changes with a count. English distinguishes one from many;
 * Thai does not, so a Thai translation is allowed to collapse a `Plural` back
 * into a single string — that is a correct translation, not a missing one.
 */
type Plural = { one: string; other: string };
type Message = string | Plural;

const en = {
  // ---- chrome ----
  "vault.select": "Choose vault",
  "vault.add": "+ Add vault…",
  "vault.addFirst": "Add your first vault",
  "allVaults": "All vaults",
  "allVaults.title": "Show every vault in one map",
  "health.open": "Vault health",
  "theme.toggle": "Switch theme",
  "lang.toggle": "Switch language",
  "rail.notes": "Notes",

  // ---- first run ----
  "onboard.body":
    "Point Samong at a notes folder or a project root. It maps what you already " +
    "know, skipping node_modules and anything .gitignore excludes.",

  // ---- prompts ----
  "prompt.vaultPath":
    "Vault folder (full path)\nPoint it at a project root — Samong skips " +
    "node_modules and anything .gitignore excludes",
  "prompt.vaultName": "Vault name (used in [[name/note]])",

  // ---- status flashes ----
  "saved": "Saved",
  "saved.outOfScope": "Saved — this file is outside scope, so search will not find it",
  "save.failed": "Could not save: {error}",
  "open.failed": "Could not open note: {error}",
  "create.done": "Created “{title}”",
  "create.failed": "Could not create note: {error}",
  "delete.confirm": "Delete note “{key}”?",
  "delete.done": "Deleted “{key}”",
  "delete.dangling": {
    one: "Deleted “{key}” — 1 note still links to it",
    other: "Deleted “{key}” — {count} notes still link to it",
  },
  "delete.failed": "Could not delete: {error}",
  "vault.added": "Added vault “{name}”",
  "vault.addFailed": "Could not add vault: {error}",
  "connect.failed": "Cannot reach the server: {error}",

  // ---- search ----
  "search.placeholder": "Search what you already wrote",
  "search.aria": "Search notes",
  "search.clear": "Clear search",
  "search.count": { one: "{count} result", other: "{count} results" },
  "search.none": "No notes match “{query}”",
  "search.noneCreate": " — create it with the button below",
  "search.create": "Create note “{query}”",

  // ---- graph ----
  "graph.aria":
    "Knowledge map, {nodes} notes, {edges} links — use the search field or the " +
    "list on the left to open a note from the keyboard",
  "graph.missing": "No note here yet",
  "graph.links": { one: "{count} link", other: "{count} links" },
  "graph.readOnlySuffix": " · read-only",
  "graph.legend.own": "Your notes",
  "graph.legend.reference": "Read-only",
  "graph.legend.referenceToggle": "Show or hide read-only notes on the map",
  "graph.legend.missing": "Not created yet",
  "stage.matched": {
    one: "Highlighting the one note that matches — press Esc in the search field to see them all",
    other:
      "Highlighting {count} notes that match — press Esc in the search field to see them all",
  },

  // ---- selected note ----
  "detail.empty": "Pick a note from the map, or search above",
  "detail.emptyHint":
    "Each circle is one file · its size is how many links it has · hollow means read-only",
  "detail.readOnly": "Read-only",
  "detail.readOnly.title": "Comes from scope.include — not editable",
  "detail.expand": "Open full screen",
  "detail.delete": "Delete",
  "detail.outgoing": "Out",
  "detail.incoming": "In",
  "detail.noLinks":
    "This note is not connected to anything yet — add a [[link]] to join it to the map",
  "link.missing.title": "No note here yet — click to create it",
  "link.ambiguous.title": "This title matches {count} files: {keys}",
  "chip.missing": "missing",
  "chip.files": "{count} files",

  // ---- reader ----
  "reader.unsaved": "Unsaved",
  "reader.back": "Back to the map",

  // ---- editor ----
  "editor.mode": "Edit mode",
  "editor.mode.edit": "Write",
  "editor.mode.split": "Both",
  "editor.mode.preview": "Read",
  "editor.content": "Note content",
  "editor.suggest": "Link to a note — Enter to accept",

  // ---- note tree ----
  "tree.empty":
    "This vault has no notes yet — type a title in the search field and create it from there",
  "tree.own": "Project notes",
  "tree.reference": "Reference (read-only)",

  // ---- vault health ----
  "health.title": "Vault health",
  "health.aria": "Health of vault {vault}",
  "health.close": "Close",
  "health.error": "Could not read the report: {error}",
  "health.loading": "Checking…",
  "health.stat.project": "Project notes",
  "health.stat.reference": "Reference",
  "health.stat.skipped": "Skipped",
  "health.folder": "Folder",
  "health.scannedFrom": "Scanned from",
  "health.gitignore.on": "respected",
  "health.gitignore.off": "disabled in samong.toml",
  "health.includeRoots": "Reference sources (scope.include)",
  "health.present": "found",
  "health.absent": "not on this machine",
  "health.absentHint":
    "A source is usually missing because its dependency is not installed — the " +
    "reference notes from it come back on their own once it is",
  "health.skippedTitle": "Markdown files not counted as notes",
  "health.skippedDependency": {
    one: "1 file sits inside a dependency directory — to learn from a set of docs, add its path to scope.include",
    other:
      "{count} files sit inside dependency directories — to learn from a set of docs, add its path to scope.include",
  },
  "health.about": "About this vault",
  "health.version": "Content version",
  "health.license": "Licence",
  "health.source": "Source",
  "health.semantic": "Semantic search",
  "health.semantic.none": "Nothing embedded yet — run `samong embed` to search by meaning",
  "health.semantic.count": { one: "{count} note embedded", other: "{count} notes embedded" },
  "health.semantic.missingProject": {
    one: "1 of your notes has no vector — run `samong embed` to catch up",
    other: "{count} of your notes have no vector — run `samong embed` to catch up",
  },
  "health.semantic.missingReference": {
    one: "1 reference note is not embedded, which is the default; it is still searchable by words",
    other:
      "{count} reference notes are not embedded, which is the default; they are still searchable by words",
  },
  "health.ambiguousTitle": "Ambiguous note titles",
  "health.noAmbiguous":
    "No duplicate titles among your project notes — every [[link]] points to exactly one file",
  "health.referenceOnly": {
    one: "1 more title collides only among reference notes, which is normal for vendored docs and needs no fix",
    other:
      "{count} more titles collide only among reference notes, which is normal for vendored docs and needs no fix",
  },
} satisfies Record<string, Message>;

export type MessageKey = keyof typeof en;

const th: Record<MessageKey, Message> = {
  "vault.select": "เลือก vault",
  "vault.add": "+ เพิ่ม vault…",
  "vault.addFirst": "เพิ่ม vault แรก",
  "allVaults": "ทุก vault",
  "allVaults.title": "รวมทุก vault ในแผนที่เดียว",
  "health.open": "สภาพ vault",
  "theme.toggle": "สลับธีม",
  "lang.toggle": "สลับภาษา",
  "rail.notes": "รายการโน้ต",

  "onboard.body":
    "ชี้ไปที่โฟลเดอร์โน้ตหรือ root ของโปรเจกต์ แล้ว Samong จะทำแผนที่ความรู้ให้ " +
    "โดยข้าม node_modules และไฟล์ที่ .gitignore กันไว้เอง",

  "prompt.vaultPath":
    "โฟลเดอร์ของ vault (พาธเต็ม)\nชี้ที่ root ของโปรเจกต์ได้เลย — Samong ข้าม " +
    "node_modules และไฟล์ที่ gitignore ให้เอง",
  "prompt.vaultName": "ชื่อ vault (ใช้ใน [[ชื่อ/โน้ต]])",

  "saved": "บันทึกแล้ว",
  "saved.outOfScope": "บันทึกแล้ว — ไฟล์นี้อยู่นอก scope จึงค้นไม่เจอ",
  "save.failed": "บันทึกไม่สำเร็จ: {error}",
  "open.failed": "เปิดโน้ตไม่สำเร็จ: {error}",
  "create.done": "สร้าง “{title}” แล้ว",
  "create.failed": "สร้างโน้ตไม่สำเร็จ: {error}",
  "delete.confirm": "ลบโน้ต “{key}” ?",
  "delete.done": "ลบ “{key}” แล้ว",
  "delete.dangling": "ลบ “{key}” แล้ว (ยังมี {count} โน้ตลิงก์มา)",
  "delete.failed": "ลบไม่สำเร็จ: {error}",
  "vault.added": "เพิ่ม vault “{name}” แล้ว",
  "vault.addFailed": "เพิ่ม vault ไม่สำเร็จ: {error}",
  "connect.failed": "เชื่อมต่อ server ไม่ได้: {error}",

  "search.placeholder": "ค้นความรู้ที่เคยบันทึกไว้",
  "search.aria": "ค้นหาโน้ต",
  "search.clear": "ล้างคำค้น",
  "search.count": "{count} ผล",
  "search.none": "ไม่พบโน้ตที่ตรงกับ “{query}”",
  "search.noneCreate": " — สร้างใหม่ได้จากปุ่มด้านล่าง",
  "search.create": "สร้างโน้ต “{query}”",

  "graph.aria":
    "แผนที่ความรู้ {nodes} โน้ต {edges} ลิงก์ — ใช้ช่องค้นหาหรือรายการด้านซ้ายเพื่อเปิดโน้ตด้วยคีย์บอร์ด",
  "graph.missing": "ยังไม่มีโน้ตนี้",
  "graph.links": "{count} ลิงก์",
  "graph.readOnlySuffix": " · อ่านเท่านั้น",
  "graph.legend.own": "โน้ตของคุณ",
  "graph.legend.reference": "อ่านเท่านั้น",
  "graph.legend.referenceToggle": "แสดงหรือซ่อนโน้ตอ่านเท่านั้นบนแผนที่",
  "graph.legend.missing": "ยังไม่มีโน้ต",
  "stage.matched": "เน้นเฉพาะ {count} โน้ตที่ตรงกับคำค้น — กด Esc ในช่องค้นเพื่อกลับมาดูทั้งหมด",

  "detail.empty": "เลือกโน้ตจากแผนที่ หรือค้นหาด้านบน",
  "detail.emptyHint": "แต่ละวงคือโน้ตหนึ่งไฟล์ · ขนาดวงบอกจำนวนลิงก์ · วงกลวงคือโน้ตอ่านเท่านั้น",
  "detail.readOnly": "อ่านเท่านั้น",
  "detail.readOnly.title": "มาจาก scope.include — แก้ไม่ได้",
  "detail.expand": "เปิดอ่านเต็มจอ",
  "detail.delete": "ลบ",
  "detail.outgoing": "ออกไป",
  "detail.incoming": "เข้ามา",
  "detail.noLinks": "โน้ตนี้ยังไม่เชื่อมกับใคร — ใส่ [[ลิงก์]] เพื่อต่อเข้าแผนที่",
  "link.missing.title": "ยังไม่มีโน้ตนี้ — กดเพื่อสร้าง",
  "link.ambiguous.title": "ชื่อนี้ตรงกับ {count} ไฟล์: {keys}",
  "chip.missing": "ยังไม่มี",
  "chip.files": "{count} ไฟล์",

  "reader.unsaved": "ยังไม่บันทึก",
  "reader.back": "กลับไปที่แผนที่",

  "editor.mode": "โหมดแก้ไข",
  "editor.mode.edit": "เขียน",
  "editor.mode.split": "คู่กัน",
  "editor.mode.preview": "อ่าน",
  "editor.content": "เนื้อหาโน้ต",
  "editor.suggest": "ลิงก์ไปยังโน้ต — Enter เพื่อเลือก",

  "tree.empty": "vault นี้ยังไม่มีโน้ต — พิมพ์ชื่อในช่องค้นหาแล้วสร้างจากที่นั่น",
  "tree.own": "โน้ตของโปรเจกต์",
  "tree.reference": "อ้างอิง (อ่านเท่านั้น)",

  "health.title": "สภาพของ vault",
  "health.aria": "สภาพ vault {vault}",
  "health.close": "ปิด",
  "health.error": "อ่านรายงานไม่ได้: {error}",
  "health.loading": "กำลังตรวจ…",
  "health.stat.project": "โน้ตของโปรเจกต์",
  "health.stat.reference": "อ้างอิง",
  "health.stat.skipped": "ข้ามไป",
  "health.folder": "โฟลเดอร์",
  "health.scannedFrom": "สแกนจาก",
  "health.gitignore.on": "เคารพ",
  "health.gitignore.off": "ปิดไว้ใน samong.toml",
  "health.includeRoots": "แหล่งอ้างอิง (scope.include)",
  "health.present": "พบ",
  "health.absent": "ไม่มีในเครื่องนี้",
  "health.absentHint":
    "แหล่งที่ไม่พบมักเป็นเพราะยังไม่ติดตั้ง dependency — โน้ตอ้างอิงจากที่นั่นจะกลับมาเองเมื่อติดตั้งแล้ว",
  "health.skippedTitle": "ไฟล์ .md ที่ไม่ถูกนับเป็นโน้ต",
  "health.skippedDependency":
    "{count} ไฟล์อยู่ในโฟลเดอร์ dependency — ถ้าอยากเรียนรู้จากเอกสารชุดไหน เพิ่ม path นั้นใน scope.include",
  "health.about": "เกี่ยวกับ vault นี้",
  "health.version": "เวอร์ชันเนื้อหา",
  "health.license": "สัญญาอนุญาต",
  "health.source": "แหล่งที่มา",
  "health.semantic": "ค้นด้วยความหมาย",
  "health.semantic.none": "ยังไม่ได้ embed อะไรเลย — สั่ง `samong embed` เพื่อค้นด้วยความหมาย",
  "health.semantic.count": "embed แล้ว {count} โน้ต",
  "health.semantic.missingProject":
    "โน้ตของคุณ {count} ไฟล์ยังไม่มี vector — สั่ง `samong embed` เพื่อตามให้ทัน",
  "health.semantic.missingReference":
    "โน้ตอ้างอิง {count} ไฟล์ไม่ได้ embed ซึ่งเป็นค่าเริ่มต้น · ยังค้นด้วยคำได้ปกติ",
  "health.ambiguousTitle": "ชื่อโน้ตที่กำกวม",
  "health.noAmbiguous": "ไม่มีชื่อซ้ำในโน้ตของโปรเจกต์ — ทุก [[ลิงก์]] ชี้ได้ที่เดียว",
  "health.referenceOnly":
    "อีก {count} ชื่อซ้ำกันเฉพาะในโน้ตอ้างอิง ซึ่งปกติสำหรับเอกสารที่มาพร้อม dependency ไม่ต้องแก้",
};

const dictionaries: Record<Locale, Record<MessageKey, Message>> = { en, th };

export type Params = Record<string, string | number>;

function pick(message: Message, params?: Params): string {
  if (typeof message === "string") return message;
  return Number(params?.count) === 1 ? message.one : message.other;
}

function fill(template: string, params?: Params): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

function translate(locale: Locale, key: MessageKey, params?: Params): string {
  const message = dictionaries[locale][key] ?? en[key];
  return fill(pick(message, params), params);
}

export type TFunction = (key: MessageKey, params?: Params) => string;

const STORAGE_KEY = "samong-lang";

/**
 * `?lang=` wins so a bug report can be reproduced in the reporter's language,
 * then the saved choice, then what the browser asks for. Anything unrecognised
 * lands on English rather than on the developer's own language.
 */
function detect(): Locale {
  const fromUrl = new URLSearchParams(location.search).get("lang");
  const saved = localStorage.getItem(STORAGE_KEY);
  for (const candidate of [fromUrl, saved, ...navigator.languages]) {
    const tag = candidate?.toLowerCase();
    if (!tag) continue;
    for (const { code } of LOCALES) {
      if (tag === code || tag.startsWith(`${code}-`)) return code;
    }
  }
  return "en";
}

let current: Locale = detect();
const listeners = new Set<() => void>();

// The bootstrap script in index.html sets `lang` before first paint from the
// same three sources; this is the authority, and corrects it if they disagree.
document.documentElement.lang = current;

export function locale(): Locale {
  return current;
}

/** Persisted, unlike `?lang=` — an explicit click is a preference. */
export function setLocale(next: Locale): void {
  if (next === current) return;
  current = next;
  localStorage.setItem(STORAGE_KEY, next);
  document.documentElement.lang = next;
  for (const notify of listeners) notify();
}

function subscribe(notify: () => void): () => void {
  listeners.add(notify);
  return () => {
    listeners.delete(notify);
  };
}

export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, locale, locale);
}

/**
 * Returns a fresh function per locale, so `t` can sit in a hook dependency
 * list and the value actually changes when the language does.
 */
export function useT(): TFunction {
  const active = useLocale();
  return useMemo<TFunction>(() => (key, params) => translate(active, key, params), [active]);
}
