/**
 * Putting text on the clipboard from inside a webview.
 *
 * `navigator.clipboard` is only defined in a secure context, and Tauri serves
 * the Android build over `http://tauri.localhost`, which is not one — so the
 * modern API is missing on exactly the platform with no keyboard shortcut to
 * fall back on. The deprecated `execCommand` path still works there, hence
 * both, and hence the boolean: a caller that cannot copy has to say so rather
 * than flash "Copied" over nothing.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch (e) {
    console.warn("clipboard write refused, falling back", e);
  }

  try {
    const scratch = document.createElement("textarea");
    scratch.value = text;
    // Off-screen rather than hidden: a `display: none` textarea has no
    // selection to copy.
    scratch.style.position = "fixed";
    scratch.style.top = "-9999px";
    scratch.setAttribute("readonly", "");
    document.body.appendChild(scratch);
    scratch.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(scratch);
    return ok;
  } catch (e) {
    console.warn("could not copy to the clipboard", e);
    return false;
  }
}
