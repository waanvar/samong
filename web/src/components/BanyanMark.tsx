/** Original banyan-tree mark: a canopy over hanging aerial roots. */
export function BanyanMark({ size = 22 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M4 9a8 5.5 0 0 1 16 0 8 5.5 0 0 1 -16 0z" fill="currentColor" opacity="0.18" />
      <path d="M4 9a8 5.5 0 0 1 16 0 8 5.5 0 0 1 -16 0z" />
      <path d="M12 14.5V21" />
      <path d="M8 13.5v4.2" />
      <path d="M16 13.5v4.2" />
      <path d="M5.5 12v2.6" />
      <path d="M18.5 12v2.6" />
    </svg>
  );
}
