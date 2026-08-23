// Every icon the UI uses, re-exported from one place. Components import from
// here rather than reaching into lucide directly, so the icon vocabulary stays
// consistent and swapping a set is a single edit.
export {
  ArrowLeft,
  ArrowUpRight,
  Bookmark,
  BookmarkCheck,
  Brain,
  Briefcase,
  Calendar,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  Cloud,
  Code2,
  Cpu,
  Database,
  Eye,
  Flame,
  Gift,
  Home,
  ListChecks,
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
  PanelsTopLeft,
  Plus,
  RefreshCw,
  RotateCcw,
  Rows3,
  Search,
  Server,
  Settings,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Star,
  Sun,
  Tag,
  Target,
  TrendingUp,
  Wrench,
  X,
} from "lucide-svelte";

import type { ScrapedItem } from "./api";
import {
  Brain,
  Briefcase,
  Calendar,
  Cloud,
  Code2,
  Cpu,
  Database,
  Newspaper,
  PanelsTopLeft,
  Server,
  ShieldCheck,
  Smartphone,
  Wrench,
} from "lucide-svelte";

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

/**
 * The icon for a skill catalog category. Keyed by the category names Rust
 * hands over in `list_skills`, with a neutral fallback so adding a category on
 * the Rust side never renders a hole.
 */
export function skillCategoryIcon(category: string): IconComponent {
  switch (category) {
    case "Languages":
      return Code2;
    case "Frontend":
      return PanelsTopLeft;
    case "Mobile":
      return Smartphone;
    case "Backend & APIs":
      return Server;
    case "Data & Databases":
      return Database;
    case "AI / ML":
      return Brain;
    case "Cloud & DevOps":
      return Cloud;
    case "Systems & Low-level":
      return Cpu;
    case "Security":
      return ShieldCheck;
    default:
      return Wrench;
  }
}
