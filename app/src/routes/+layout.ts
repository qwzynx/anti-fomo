// Tauri has no Node server — the app ships as a prerendered static SPA shell
// and every route renders client-side inside the webview.
export const ssr = false;
export const prerender = true;
