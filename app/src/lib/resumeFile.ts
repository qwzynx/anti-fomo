import { save } from "@tauri-apps/plugin-dialog";
import { writeFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { openPath } from "@tauri-apps/plugin-opener";

import * as api from "./api";
import type { ResumeTheme } from "./api";

/**
 * Putting a file where the user asked for it.
 *
 * Rust renders the PDF and hands back raw bytes; the destination is chosen
 * here. That split is on purpose and is the portable one: on Android the save
 * dialog returns a `content://` URI rather than a filesystem path, which
 * `std::fs::write` in Rust could not open at all — the fs plugin can. Desktop
 * gets a native save dialog out of the same code.
 *
 * Every failure is *reported*, never swallowed. `ItemDetail.svelte` documents
 * why in the same words: the opener plugin's scope check turned every Apply
 * button into a silent no-op that only complained to a console nobody on a
 * phone can read, and that is exactly how it went unnoticed. A missing
 * `dialog:` or `fs:` permission would fail the same way here.
 */

export type SaveOutcome =
  | { ok: true; path: string; opened: boolean }
  | { ok: false; cancelled: true }
  | { ok: false; cancelled: false; message: string };

function reason(e: unknown): string {
  const message = e instanceof Error ? e.message : String(e);
  // The one failure worth naming precisely, because the fix is a capability
  // entry rather than anything the user did.
  if (/not allowed|forbidden|permission|scope/i.test(message)) {
    return `The app is not permitted to write that file (${message}). This is a packaging problem, not something you did.`;
  }
  return message;
}

/** Renders the résumé and writes it wherever the user points the dialog. */
export async function saveResumePdf(opts: {
  id?: string;
  url?: string;
  theme?: ResumeTheme;
  filename: string;
  /** Hand the saved file to the system viewer afterwards. */
  reveal?: boolean;
}): Promise<SaveOutcome> {
  let bytes: ArrayBuffer;
  try {
    bytes = await api.renderResumePdf({ id: opts.id, url: opts.url, theme: opts.theme });
  } catch (e) {
    console.error("could not render the résumé", e);
    return { ok: false, cancelled: false, message: reason(e) };
  }

  let path: string | null;
  try {
    path = await save({
      defaultPath: opts.filename,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
  } catch (e) {
    console.error("could not open the save dialog", e);
    return { ok: false, cancelled: false, message: reason(e) };
  }
  if (!path) return { ok: false, cancelled: true };

  try {
    await writeFile(path, new Uint8Array(bytes));
  } catch (e) {
    console.error("could not write the résumé", e);
    return { ok: false, cancelled: false, message: reason(e) };
  }

  // Opening it is a convenience, so a refusal here is not a failed save — the
  // file is on disk either way and saying otherwise would be a lie.
  let opened = false;
  if (opts.reveal) {
    try {
      await openPath(path);
      opened = true;
    } catch (e) {
      console.warn("saved, but could not open the file", e);
    }
  }
  return { ok: true, path, opened };
}

/** Writes the résumé as a `resume.json`, for backup or another tool. */
export async function saveResumeJson(id: string | undefined, name: string): Promise<SaveOutcome> {
  let json: string;
  try {
    json = await api.exportJsonResume(id);
  } catch (e) {
    return { ok: false, cancelled: false, message: reason(e) };
  }

  let path: string | null;
  try {
    path = await save({
      defaultPath: `${name || "resume"}.json`,
      filters: [{ name: "JSON Resume", extensions: ["json"] }],
    });
  } catch (e) {
    return { ok: false, cancelled: false, message: reason(e) };
  }
  if (!path) return { ok: false, cancelled: true };

  try {
    await writeTextFile(path, json);
    return { ok: true, path, opened: false };
  } catch (e) {
    return { ok: false, cancelled: false, message: reason(e) };
  }
}

/** A short "Saved to …" line, or the reason it did not happen. */
export function outcomeMessage(outcome: SaveOutcome): string | null {
  if (outcome.ok) {
    const file = outcome.path.split(/[/\\]/).pop() || outcome.path;
    return `Saved ${file}`;
  }
  return outcome.cancelled ? null : outcome.message;
}
