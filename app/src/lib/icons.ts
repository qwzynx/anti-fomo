// Every icon the UI uses, re-exported from one place. Components import from
// here rather than reaching into lucide directly, so the icon vocabulary stays
// consistent and swapping a set is a single edit.
export {
  ArrowLeft,
  ArrowUpRight,
  Bookmark,
  BookmarkCheck,
  Briefcase,
  Calendar,
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Eye,
  Flame,
  Home,
  Inbox,
  LayoutGrid,
  LayoutList,
  ListFilter,
  Loader,
  LoaderCircle,
  MapPin,
  Monitor,
  Moon,
  Newspaper,
  RefreshCw,
  RotateCcw,
  Rows3,
  Search,
  Settings,
  Sparkles,
  Star,
  Sun,
  Tag,
  TrendingUp,
  X,
} from "lucide-svelte";

import type { ScrapedItem } from "./api";
import { Briefcase, Calendar, Newspaper } from "lucide-svelte";

/**
 * The type of a lucide icon component. Taken from a real icon rather than
 * written out as `Component<IconProps>`: lucide-svelte still declares its icons
 * as `SvelteComponentTyped` classes, which do not satisfy Svelte 5's
 * `Component` signature, so a hand-written type fails to typecheck.
 */
export type IconComponent = typeof Briefcase;

/** The icon that stands for an item's kind, used on badges and empty states. */
export function iconForType(type: ScrapedItem["item_type"]) {
  switch (type) {
    case "Internship":
    case "Job":
      return Briefcase;
    case "Event":
      return Calendar;
    default:
      return Newspaper;
  }
}
