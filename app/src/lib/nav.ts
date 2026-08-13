import { Bookmark, Briefcase, Home, Settings } from "./icons";

/** The app's four destinations, shared by the top bar and the mobile tab bar
 *  so the two can never drift apart. */
export const NAV = [
  { href: "/", label: "Feed", icon: Home },
  { href: "/internships", label: "Jobs", icon: Briefcase },
  { href: "/saved", label: "Saved", icon: Bookmark },
  { href: "/settings", label: "Settings", icon: Settings },
] as const;
