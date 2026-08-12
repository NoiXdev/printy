import type { JSX } from "react";

interface LogoProps {
  /** Pixel size of the square mark. Defaults to 28. */
  size?: number;
  /** Render the "Printy" wordmark next to the mark. Defaults to false. */
  wordmark?: boolean;
  /** Color of the wordmark text. Defaults to currentColor (inherits). */
  wordmarkColor?: string;
  className?: string;
}

/**
 * The Printy brand mark: a teal folder with a coral sheet emerging from it and
 * a mint confirmation badge. Rendered as crisp inline SVG so it scales to any
 * size. The folder carries a literal cream rim rather than a themed one, so it
 * stays legible on the teal sidebar and on the dark-theme page alike. Purely
 * presentational — `aria-hidden` on the mark; pass a wordmark for a labelled
 * lockup.
 */
export default function Logo({
  size = 28,
  wordmark = false,
  wordmarkColor = "currentColor",
  className,
}: LogoProps): JSX.Element {
  const mark = (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      aria-hidden="true"
      role="img"
    >
      {/* coral sheet emerging from the folder, tilted out of the pocket */}
      <rect
        x="18"
        y="4"
        width="16"
        height="20"
        rx="2"
        fill="var(--coral)"
        transform="rotate(7 26 14)"
      />
      <rect
        x="21"
        y="10"
        width="9"
        height="2"
        rx="1"
        fill="#fff5f2"
        transform="rotate(7 26 14)"
      />
      <rect
        x="21"
        y="15"
        width="6"
        height="2"
        rx="1"
        fill="#fff5f2"
        transform="rotate(7 26 14)"
      />
      {/* teal folder body, in front of the sheet */}
      <path
        d="M4 16h11l3 4h23a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H4a3 3 0 0 1-3-3V19a3 3 0 0 1 3-3z"
        fill="var(--teal)"
        stroke="#f4f1ea"
        strokeWidth="1.5"
      />
      {/* mint confirmation badge */}
      <circle cx="36" cy="35" r="6" fill="var(--mint)" />
      <path
        d="M33.4 35l2 2 3-3.6"
        stroke="var(--teal)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
    </svg>
  );

  if (!wordmark) return className ? <span className={className}>{mark}</span> : mark;

  return (
    <span
      className={className}
      style={{ display: "inline-flex", alignItems: "center", gap: size * 0.32 }}
    >
      {mark}
      <span
        style={{
          fontFamily: "var(--font-display)",
          fontWeight: 800,
          fontSize: size * 0.72,
          letterSpacing: "-0.01em",
          color: wordmarkColor,
          lineHeight: 1,
        }}
      >
        Printy
      </span>
    </span>
  );
}
