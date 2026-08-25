import { Bookmark, Briefcase, FileText, Home, Settings } from "./icons";

/**
 * Where you can go. Three destinations, not four: Settings is a tool you reach
 * for and come back from, not a place you spend time, so it sits with the
 * other tools in the top bar rather than competing with the feed for a slot.
 *
 * The top bar renders `NAV`; the phone tab bar renders `TABS`, which adds
 * Settings back because a phone has no top bar to hide it in.
 */
export const NAV = [
  { href: "/", label: "Today", icon: Home },
  { href: "/internships", label: "Roles", icon: Briefcase },
  { href: "/saved", label: "Saved", icon: Bookmark },
] as const;

/**
 * Tools, not destinations. The résumé builder is the same shape as Settings —
 * somewhere you go to change something and then come back from — so it joins
 * the top bar's tool cluster rather than taking a fourth slot from the feed.
 */
export const RESUME = { href: "/resume", label: "Résumé", icon: FileText } as const;

export const SETTINGS = { href: "/settings", label: "Settings", icon: Settings } as const;

export const TOOLS = [RESUME, SETTINGS] as const;

/** A phone has no top bar to hide the tools in, so they come back as tabs. */
export const TABS = [...NAV, ...TOOLS] as const;
