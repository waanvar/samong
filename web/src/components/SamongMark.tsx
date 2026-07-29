/**
 * The Samong mark: a small network with exactly one node lit.
 *
 * That is the product's one act — you ask, and the thing you needed is the one
 * that lights up. The lit node is the only element here that wears the accent
 * colour, which is the same rule the interface follows: `--found` means "this is
 * what you were looking for" and nothing else.
 *
 * Four circles and four lines, so it survives a 16px favicon, a one-colour
 * screen print, and an embroidery needle. Identical geometry to
 * `site/brand/samong-mark.svg` — the product and the brand are one object.
 */
export function SamongMark({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" aria-hidden="true">
      <g stroke="var(--ink-mute)" strokeWidth="1.5" strokeLinecap="round">
        <path d="M9 9.5 22.5 7" />
        <path d="M9 9.5 20.5 20.5" />
        <path d="M8.5 22.5 20 21.5" />
        <path d="M9 9.5 8.5 22.5" />
      </g>
      <circle cx="9" cy="9.5" r="3.4" fill="var(--ink)" />
      <circle cx="22.5" cy="7" r="2.3" fill="var(--ink-mute)" />
      <circle cx="8.5" cy="22.5" r="2.3" fill="var(--ink-mute)" />
      <circle cx="21.5" cy="21.5" r="5.2" fill="var(--found)" />
    </svg>
  );
}
