/**
 * Lightweight host-OS sniff for the frontend. There is no `@tauri-apps/plugin-os`
 * dependency in this app, and pulling one in just to pick a file-dialog filter
 * extension would be a lot of dependency for one boolean, so this reads the
 * WebView's own user agent instead (WebView2 on Windows reports "Windows").
 */
export function isWindows(): boolean {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}
