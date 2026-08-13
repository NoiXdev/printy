/**
 * Keeps at most one folder-card menu open at a time. Each `FolderCard` owns
 * its own open/closed state; when it opens, it registers its own closer
 * here, which closes whichever other card's menu was previously open. No
 * global React state or context is needed for this -- a single module-level
 * slot is enough because only one menu can ever legitimately be open.
 */

type Closer = () => void;

let activeClose: Closer | null = null;

/** Call when a menu opens. Closes any other menu that was open. */
export function openExclusive(close: Closer): void {
  if (activeClose && activeClose !== close) activeClose();
  activeClose = close;
}

/** Call when a menu closes (including on unmount) to release the slot. */
export function closeExclusive(close: Closer): void {
  if (activeClose === close) activeClose = null;
}
