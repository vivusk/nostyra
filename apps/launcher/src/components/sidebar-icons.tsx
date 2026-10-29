// Inline stroke icons from the approved mockup; decorative (aria-hidden), colored by currentColor.
import logoMarkUrl from "../assets/nostyra-logo-mark.svg";

/** Nostyra logo mark (gamepad pill: D-pad + face-button X) from the shared SVG file;
 *  the app icon source draws the same mark on the app background. */
export function NostyraLogoMark() {
  return <img src={logoMarkUrl} width={28} height={28} alt="" aria-hidden="true" />;
}

const strokeProps = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  "aria-hidden": true,
} as const;

export function HomeIcon() {
  return (
    <svg {...strokeProps}>
      <path d="M3 11l9-7 9 7" />
      <path d="M5 10v10h14V10" />
    </svg>
  );
}

export function LibraryIcon() {
  return (
    <svg {...strokeProps}>
      <rect x="3" y="3" width="7" height="7" rx="1.5" />
      <rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" />
      <rect x="14" y="14" width="7" height="7" rx="1.5" />
    </svg>
  );
}

export function SettingsIcon() {
  return (
    <svg {...strokeProps}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" />
    </svg>
  );
}
