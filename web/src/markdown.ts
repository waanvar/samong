import { marked } from "marked";
import DOMPurify from "dompurify";

const WIKILINK = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;

/** Render markdown to sanitized HTML, turning [[wikilinks]] into
 *  clickable spans carrying their target in a data attribute. */
export function renderMarkdown(source: string): string {
  const withLinks = source.replace(WIKILINK, (_m, target: string, alias?: string) => {
    const label = (alias ?? target).trim();
    const t = target.trim().replace(/"/g, "&quot;");
    return `<a class="wikilink" data-target="${t}">${label}</a>`;
  });
  const html = marked.parse(withLinks, { async: false, gfm: true, breaks: true });
  return DOMPurify.sanitize(html, { ADD_ATTR: ["data-target"] });
}

export interface OutlineItem {
  level: number;
  text: string;
  line: number;
}

/** Headings of a markdown document, for the outline panel. */
export function outline(source: string): OutlineItem[] {
  const items: OutlineItem[] = [];
  source.split("\n").forEach((line, index) => {
    const m = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (m) items.push({ level: m[1].length, text: m[2], line: index });
  });
  return items;
}
