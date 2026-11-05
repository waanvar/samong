# Samong design system

The interface as it is built, written down so the next screen looks like it came
from the same hand as the last one — whoever adds it, and however long after.

Everything here is read off the code, not aspired to. Where this file and the
code disagree, the code shipped and this file is wrong; `tests/design_system.rs`
fails on the disagreements it can see (token values, vault hues, raw colours,
colour transitions, unguarded animation), so most drift is caught by the commit
that causes it rather than by someone noticing a screen looks off.

The sources: `web/src/styles.css` (every style the app has), `web/src/App.tsx`
(the frame), `web/src/components/` (the parts), `web/src/i18n.ts` (every string),
and `site/index.html` (the website, which shares the palette). The reasoning
behind most decisions is in `PLAN.md`, Phases 15, 16, 19 and 23.

## What the interface is

**The graph is the workspace.** A vault is a shape you learn to recognise, not a
folder listing. Searching is how you enter it: type, and everything that does not
match dims, so a query becomes a place. Reading a note is a state laid over that
map — `Esc` returns to it — not a different application.

**An instrument, not a document.** Luminous data on a deep neutral ground, the way
an instrument panel is. Dense, quiet, and fast to respond. Decoration has no job
here; every visual element carries information.

**Global, and unusually good at Thai.** Thai is a capability, not a theme. The
only place word segmentation shows in the visual language is the hairline at the
edge of a matched token in a search snippet — and that is information, not
ornament. No Thai motifs, no Thai-only copy, English as the default locale.

## Colour

Colour is assigned by meaning. A new screen picks the meaning first, and the
colour follows; it never introduces a colour.

| Meaning | Token | Used for | Never used for |
|---|---|---|---|
| Found / now | `--found`, `--found-edge`, `--found-wash` | Matched text, the active note in the tree, a toggle that is on, the focused search field's border, the lit node in the mark, the "o" in the wordmark | Decoration, headings, emphasis, primary buttons |
| Interactive | `--accent`, `--accent-ink`, `--accent-wash` | Primary button, focus ring, selected search hit, magnitude bars | Links between notes |
| A reference to a note | `--link`, `--link-wash` | Link chips, wikilinks in rendered Markdown, `.link-btn` | Generic buttons |
| Status | `--ok`, `--warn`, `--danger` | Tags, an ambiguous link, unsaved changes, a destructive hover | Anything that is not a status |
| Vault identity | `--v1` … `--v4` | Graph nodes and legend dots, one hue per vault in fixed order | Folders, note types, categories |
| Ground | `--void` < `--surface` < `--surface-2` < `--surface-3` | Canvas and page < panels < controls < hover | — |
| Text | `--ink` > `--ink-2` > `--ink-mute` | Primary > secondary > hints, paths, counts | — |
| Rules | `--line`, `--line-strong` | Dividers between regions; borders on controls and floating surfaces | — |

Rules that follow from the table:

- **`--found` is spent on one meaning.** "This is what you were looking for", or
  "this is the one active now". If a new screen wants to make something stand
  out that is neither, it uses weight, size or position, not `--found`.
- **State before colour.** A read-only note is a hollow node, a missing target is
  a dashed one; neither got a new hue. When something new needs to be told apart,
  reach for fill, outline, dash, weight or opacity first. The categorical set was
  cut from six hues to four by the validator, and the design improved for it.
- **Vault hues are never cycled.** A fifth vault is drawn in a neutral grey
  (`NEUTRAL_HUE` in `GraphCanvas.tsx`) rather than reusing a hue that already
  means another vault. The legend is always shown, so colour is never the only
  signal.
- **Washes are mixed, not picked.** A tint is `color-mix(in oklab, <token> N%,
  transparent)`, with N between 12% and 20% in the existing styles (34% for
  `--found-wash` in light, which needs it to show on a pale ground).
- **Contrast is AA in both themes**, measured for every text/background pair
  when the palette was set (`PLAN.md`, Phase 15). A new pairing is measured, not
  assumed: the `kbd` element failed at 3.81:1 in `--ink-mute` and passes at 7.93:1
  in `--ink-2`.

### Tokens

Defined once in `:root` (dark, the default) and overridden under
`:root[data-theme="light"]`. A dash in the light column means the dark value
applies to both. `tests/design_system.rs` compares this table with
`web/src/styles.css` cell by cell.

| Token | Dark | Light | Role |
|---|---|---|---|
| `--void` | `#0d1014` | `#eceef2` | Page and canvas ground |
| `--surface` | `#14181e` | `#f8f9fb` | Panels: header, rail, detail, sheets |
| `--surface-2` | `#1b2028` | `#e6e9ef` | Controls: buttons, fields, code |
| `--surface-3` | `#242a34` | `#d8dce4` | Hover on a control |
| `--ink` | `#e8ecf2` | `#12161d` | Primary text |
| `--ink-2` | `#b7bfcc` | `#3a4250` | Secondary text, button labels |
| `--ink-mute` | `#8792a3` | `#646d7d` | Hints, paths, counts, placeholders |
| `--line` | `#262c36` | `#dcdfe7` | Dividers between regions |
| `--line-strong` | `#39414e` | `#bcc2cd` | Borders on controls and floating surfaces |
| `--found` | `#d8f24a` | `#b9d61f` | Found / now |
| `--found-edge` | `#b6d423` | `#7f9410` | Edges and hairlines of found / now |
| `--found-wash` | `color-mix(in oklab, var(--found) 20%, transparent)` | `color-mix(in oklab, var(--found) 34%, transparent)` | Background of found / now |
| `--accent` | `#6f8dff` | `#3b4fd0` | Interactive, primary, focus ring |
| `--accent-ink` | `#0d1014` | `#ffffff` | Text on `--accent` |
| `--accent-wash` | `color-mix(in oklab, var(--accent) 16%, transparent)` | — | Selected row |
| `--link` | `#8fa3ff` | `#3843b4` | A reference to a note |
| `--link-wash` | `color-mix(in oklab, var(--link) 14%, transparent)` | — | Background of a link chip |
| `--danger` | `#e88378` | `#b3352c` | Destructive |
| `--ok` | `#6fce9d` | `#2c7a52` | Healthy, present |
| `--warn` | `#e0b063` | `#8f5c0d` | Ambiguous, unsaved, missing |
| `--v1` | `#4a7fd4` | — | First vault |
| `--v2` | `#c47b16` | — | Second vault |
| `--v3` | `#a35fc4` | — | Third vault |
| `--v4` | `#0f9b7e` | — | Fourth vault |
| `--font-display` | `"Bai Jamjuree", "IBM Plex Sans Thai", system-ui, sans-serif` | — | Titles, section labels, big numbers |
| `--font-ui` | `"IBM Plex Sans Thai", system-ui, -apple-system, sans-serif` | — | Everything else |
| `--font-mono` | `"IBM Plex Mono", ui-monospace, monospace` | — | Paths, keys, counts, codes |
| `--shadow-2` | `0 24px 60px -20px rgba(0, 0, 0, 0.75)` | `0 20px 52px -22px rgba(18, 22, 29, 0.32)` | Floating surfaces only |
| `--radius` | `8px` | — | Buttons, fields, panels, cards |
| `--radius-sm` | `5px` | — | Rows, small chips, `kbd`, focus ring |
| `--leading-body` | `1.75` | — | Body text; Thai sets vowels above and below the line |
| `--leading-tight` | `1.3` | — | Headings |

A pill is `border-radius: 999px`: search field, chips, badges, tags, the graph
legend and the stage note. There is no third radius.

## Type

Three families, three roles, all OFL-1.1, imported from `@fontsource` in
`web/src/main.tsx` and served by the binary itself — no request leaves the
machine for a font. Only the weights imported there exist: display 500, 600,
700; UI 400, 500, 600; mono 400. A new weight is a new download for everyone, so
use one of these.

- **Bai Jamjuree** (display): note titles, sheet titles, section labels, big
  numbers. Semi-condensed because Thai titles run long and must fit on a line.
- **IBM Plex Sans Thai** (UI): everything people read. Its vertical metrics give
  Thai vowels room.
- **IBM Plex Mono**: anything that is an identifier, not prose — paths, keys,
  counts, licence codes, the language switch. If a person would copy it rather
  than read it, it is mono.

| Use | Size | Weight | Family |
|---|---|---|---|
| Markdown `h1` | 24px | 600 | display |
| Big number (`.stat-num`) | 22px | 700 | display, tabular |
| Detail title, Markdown `h2` | 19px | 600 | display |
| Sheet title, reader title, Markdown `h3` | 16px | 600 | display |
| Editor title | 15px | 600 | display |
| Body, search input | 14px | 400 | UI |
| Buttons, tree rows, lists | 13px | 400 / 500 | UI |
| Meta, snippets, hints, chips | 12.5px | 400 | UI |
| Small meta, legend, tooltips | 11.5px | 400 | UI |
| Section label (`.side-label`, `.chip-label`) | 10.5px | 600, uppercase, 0.08–0.09em tracking | display |
| Badges, counts, `kbd` | 10–11px | 400 | mono |

Headings carry `letter-spacing: -0.015em` and `--leading-tight`. Numbers that
line up in a column get `font-variant-numeric: tabular-nums`. Prose is held to
`72ch` (`.preview`).

## Layout

One frame, no routes. Every screen is a region of it or a layer over it.

```
┌ .frame-top ─────────────────────────────────────────────────────────────┐
│ brand · vault switch · [ search ──────────── ] · status · buttons · EN ☀ ⏻ │
├ .rail ┬ .stage ─────────────────────────────────────────┬ .detail ──────┤
│ notes │ graph canvas                                    │ title, path,  │
│ tree  │ (legend bottom-left, scale bottom-right,        │ link chips,   │
│ 34px  │  stage note top-centre)                         │ preview       │
│ /250px│                                                 │ 380px         │
└───────┴─────────────────────────────────────────────────┴───────────────┘
  Layers over the frame: .reader (full screen, Esc) · .sheet (right, Esc)
```

- `.app` is a grid: `auto` header over a `minmax(0, 1fr)` body. The page itself
  never scrolls; regions scroll inside themselves.
- `.frame-body` columns: `auto minmax(0, 1fr) 380px` — rail, stage, detail.
- Breakpoints, all at the **end** of `styles.css` so they win (a media query
  placed before the rule it overrides once left the sidebar un-hideable on
  phones):
  - ≤ 1180px: detail 320px, open rail 210px.
  - ≤ 900px: the detail column is hidden; a selected note opens the reader
    instead of squeezing a third column beside the map.
  - ≤ 640px: the header wraps and search takes a full row; the rail overlays the
    stage; the graph scale is hidden.
- Use logical properties (`inline-size`, `padding-inline`, `inset-block-start`,
  `border-block-end`, `text-align: start`), as the existing styles do.
- Spacing is not tokenised. The values in use: control gaps 5–10px; header
  padding `9px 12px`; panel padding 16–18px horizontally; section rhythm 12–14px.
  Match the nearest existing region rather than inventing a value.

### Brand in the app

The brand appears in the header only (`SamongMark` 22px + `SamongWordmark` cap
13px) and at display size on the onboarding and stopped screens (45px + cap
26px). The mark is always 1.726 × the wordmark's cap height. Both are geometry;
neither depends on a font. No other screen shows the brand.

## Components

Reuse these by class name. A new screen that needs something not on this list
should first check whether it is one of these in another arrangement.

| Class | What it is | Variants |
|---|---|---|
| `.btn` | Every button | `.primary` (one per view at most), `.lg`, `.danger` (red only on hover), `.on` (a toggle that is on — takes `--found`), `.icon`, `.icon.lang` |
| `kbd` | A key hint inside a button | — |
| `.search-field` | Pill input with icon, meta and clear | focus turns the border `--found-edge` |
| `.search-hit` | A result row: `.hit-head` (title, path, badge) over `.snippet` | `.sel`, `.create` |
| `.chip` | A link to a note | `.dangling` (dashed, mute), `.ambiguous` (warn), `.cross` (outlined) |
| `.chip-row` + `.chip-label` | A labelled row of chips | — |
| `.side-label` | Uppercase section label | `.ref` adds a top rule |
| `.source-badge`, `.ref-badge` | Mono pill for provenance or read-only; never truncated | — |
| `.tag` | Mono status pill | `.ok`, `.warn` |
| `.stat-row` > `.stat` | Three big numbers with labels | — |
| `.health-list` | `dl` of label / value pairs | — |
| `.plain-list` | Unstyled list of rows | `.nested` |
| `.link-btn` | A text button that opens a note | — |
| `.bar-list` | Magnitude: name, bar, labelled value on every row, one hue | — |
| `.empty-hint` | Muted line for empty, loading and error states | — |
| `.path` | A path or key: mono, muted | — |
| `.status-flash` | Transient message in the header, via `flash()` in `App.tsx` | — |
| `.stage-note` | Pill over the graph stating the current filter | — |
| `.tree-list` | Rows in the rail | `.tree-folder`, `.tree-note.active` |
| `.mode-switch` | Segmented control | `button.on` |

Icons are Unicode glyphs (`☀ ☾ ⏻ ‹ ›`), not an icon font or SVG set.
Confirmation of a destructive action uses `window.confirm`; there is no modal
component, and a new screen should not introduce one to replace it.

## Motion

- **Only layout and transform animate. Colour never transitions.** A transition
  on a token-valued colour cannot interpolate when the theme flips the token: the
  computed colour strands on the old theme, which once left the search field dark
  in light mode at 1.11:1 contrast. Hover feedback is instant.
- Durations 130–220ms; entrances use `cubic-bezier(0.2, 0.7, 0.3, 1)`; keyframes
  declare only `from`.
- Every `animation` sits inside `@media (prefers-reduced-motion: no-preference)`.
  With reduced motion the element simply appears.

## Words

- Every visible string goes through `t()` from `web/src/i18n.ts`. Keys are
  declared in `en` first; `th` is type-checked against it, so a missing
  translation is a build error. Counts use `{ one, other }`; Thai may collapse a
  plural to one string.
- Labels describe what happens, plainly: "Vault health", "Quit", "Add vault".
  The same understated voice as the README. Errors say what failed and include
  the reason: `t("save.failed", { error })`.
- Icon-only buttons get an `aria-label` from `t()`.

## Adding a screen or a menu

Most additions are one of four shapes. Pick the shape before writing anything.

1. **A tool or report about the vault** (like *Vault health*): a sheet.
   - Opened by a `.btn` in `.frame-top`, placed with the existing text buttons,
     before the `EN ☀ ⏻` cluster, which stays last.
   - Markup: `.sheet-backdrop` (click closes) > `aside.sheet[role="dialog"]`
     with an `aria-label` > `header.sheet-head` (`h2` title + a close `.btn`) >
     `.sheet-body`. `Esc` closes. Loading and error states are `.empty-hint`.
   - `web/src/components/VaultHealth.tsx` is the reference implementation; copy
     its structure.
2. **Something about the selected note**: a section in `DetailPanel`, shaped like
   `.detail-links` — padding `12px 18px`, a bottom rule, a `.chip-label` and its
   content.
3. **A focused, full-screen state** on one note: the `.reader` layer, with a
   `.reader-bar` and a back `.btn` showing `<kbd>Esc</kbd>`.
4. **A new way to filter or see the map**: a control in the graph legend or the
   header, a `.btn.on` when active, and a `.stage-note` saying what is shown.

Then, for any of them:

- Colours from the tokens only, chosen by meaning from the table above.
- Styles go in `web/src/styles.css` under a `/* ---------- name ---------- */`
  section, before the responsive block. No CSS framework, no CSS-in-JS, no
  component library, no new font.
- Strings in `en` and `th`.
- Check it at 1180, 900, 640 and 375px wide, in both themes (`?theme=light`) and
  both languages (`?lang=th` — Thai runs longer and needs the leading).
- `cd web && npm run build`, then `cargo test --all --locked`. The UI is embedded
  at compile time, so building only the crate keeps serving the old interface.

## Will not be accepted

- A colour literal outside the token blocks, or a new token added for one screen.
- `--found` used for anything other than found / now.
- A colour transition, or an animation outside the reduced-motion guard.
- A fifth vault hue, or hues cycled.
- A route, a page, or a second layout beside the frame.
- A UI library, icon set or CSS framework. The whole bundle ships inside the
  binary, and every kilobyte is downloaded by everyone.
- 3D. It was considered for the graph and rejected: hundreds of kilobytes for
  something that does not help anyone find a note, and keyboard and screen
  reader access lost with it.

## The website

`site/index.html` is a separate page with the same palette, dark only. Shared
tokens keep the same names and values as the app's dark theme (the test checks
this), and the vault hues are reused so the page and the product show the same
graph. Differences that are deliberate: 16px body text, a `62ch` measure, a
focus ring in `--found` rather than `--accent`, and the type tokens named
`--display`, `--body`, `--mono`. `site/brand.html` is the brand guide and the
authority on the mark and the lockup.
