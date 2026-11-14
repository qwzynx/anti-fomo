import type { ApplicationStatus } from "./api";

/**
 * The stages an application moves through, in order, with how each reads.
 * Rust stores the lowercase value (`db::APPLICATION_STATUSES`); the two lists
 * must name the same four.
 */
export const APPLICATION_STAGES: {
  value: ApplicationStatus;
  label: string;
  /** Filled-chip classes when this stage is the current one. */
  activeClass: string;
  /** A quieter badge, for list rows. */
  badgeClass: string;
}[] = [
  {
    value: "applied",
    label: "Applied",
    activeClass: "bg-brand text-brand-fg",
    badgeClass: "bg-brand-soft text-brand-soft-fg",
  },
  {
    value: "interviewing",
    label: "Interviewing",
    activeClass: "bg-event text-event-soft",
    badgeClass: "bg-event-soft text-event",
  },
  {
    value: "offer",
    label: "Offer",
    activeClass: "bg-job text-job-soft",
    badgeClass: "bg-job-soft text-job",
  },
  {
    value: "rejected",
    label: "Closed",
    activeClass: "bg-muted text-surface",
    badgeClass: "bg-line-soft text-muted",
  },
];

export function stageLabel(status: ApplicationStatus | null): string {
  return APPLICATION_STAGES.find((s) => s.value === status)?.label ?? "Saved";
}

export function stageBadgeClass(status: ApplicationStatus | null): string {
  return APPLICATION_STAGES.find((s) => s.value === status)?.badgeClass ?? "bg-line-soft text-subtle";
}
