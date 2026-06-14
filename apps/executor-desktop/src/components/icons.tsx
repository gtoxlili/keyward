// Minimal stroke icons (currentColor). No icon-library dependency.
type P = { size?: number };
const s = (n = 18) => ({
  width: n,
  height: n,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export const IconDashboard = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M3 13a9 9 0 0 1 18 0" />
    <path d="M12 13l4-3" />
    <circle cx="12" cy="13" r="1.4" fill="currentColor" stroke="none" />
    <path d="M3 19h18" />
  </svg>
);

export const IconPair = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M9 7H6a4 4 0 0 0 0 8h3" />
    <path d="M15 7h3a4 4 0 0 1 0 8h-3" />
    <path d="M8.5 11h7" />
  </svg>
);

export const IconKey = ({ size }: P) => (
  <svg {...s(size)}>
    <circle cx="8" cy="8" r="3.2" />
    <path d="M10.3 10.3 20 20" />
    <path d="M17 17l2-2M14.5 14.5l2-2" />
  </svg>
);

export const IconShield = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M12 3l7 3v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6z" />
    <path d="M9.2 12l1.9 1.9 3.7-3.7" />
  </svg>
);

export const IconGear = ({ size }: P) => (
  <svg {...s(size)}>
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2" />
  </svg>
);

export const IconArrow = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M5 12h13" />
    <path d="M13 7l5 5-5 5" />
  </svg>
);

export const IconCopy = ({ size }: P) => (
  <svg {...s(size)}>
    <rect x="9" y="9" width="11" height="11" rx="2" />
    <path d="M5 15V5a2 2 0 0 1 2-2h8" />
  </svg>
);

export const IconCheck = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M5 12.5l4.5 4.5L19 6.5" />
  </svg>
);

export const IconSun = ({ size }: P) => (
  <svg {...s(size)}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19" />
  </svg>
);

export const IconMoon = ({ size }: P) => (
  <svg {...s(size)}>
    <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />
  </svg>
);
