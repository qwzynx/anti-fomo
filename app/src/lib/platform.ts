/**
 * The modifier a shortcut hint should name. This ships on Linux, Windows,
 * macOS and Android, and "⌘K" on a Linux laptop is a hint for a key that is
 * not there.
 */
export const isMac =
  typeof navigator !== "undefined" && /mac|iphone|ipad/i.test(navigator.userAgent);

export const modKey = isMac ? "⌘" : "Ctrl ";
