import {
  useEffect,
  useId,
  useRef,
  useState,
  type FocusEvent,
  type JSX,
  type KeyboardEvent,
} from "react";

export interface FolderMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** Styles the item as destructive (e.g. delete) and rules its own colour. */
  destructive?: boolean;
  /** Renders a visual separator directly above this item. */
  separated?: boolean;
}

export interface FolderMenuProps {
  /** Accessible name of the trigger button, e.g. "Aktionen für Scanner". */
  label: string;
  items: FolderMenuItem[];
  /** Controlled so a right-click on the card can open the same menu. */
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * The folder card's "..." overflow menu. Follows the WAI-ARIA menu-button
 * pattern: opening focuses the first enabled item, arrow keys move a roving
 * selection (skipping disabled items and wrapping at the ends), Enter/Space
 * activates the focused item, and Escape -- or activating an item -- closes
 * the menu and returns focus to the trigger button.
 *
 * `open`/`onOpenChange` are controlled by the parent `FolderCard` rather than
 * owned here, because the card's right-click handler needs to be able to
 * open the very same menu the "..." button opens.
 */
export default function FolderMenu({
  label,
  items,
  open,
  onOpenChange,
}: FolderMenuProps): JSX.Element {
  const [activeIndex, setActiveIndex] = useState(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const menuId = useId();

  const enabledIndexes = items.reduce<number[]>((acc, item, i) => {
    if (!item.disabled) acc.push(i);
    return acc;
  }, []);

  // Opening always starts the roving selection at the first enabled item.
  useEffect(() => {
    if (open) setActiveIndex(enabledIndexes[0] ?? 0);
    // Item composition does not change while a menu this app renders is
    // open; only the open transition itself matters here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Keeps DOM focus in sync with the roving `activeIndex`, both on open and
  // as arrow keys move it.
  useEffect(() => {
    if (open) itemRefs.current[activeIndex]?.focus();
  }, [open, activeIndex]);

  // Click (or tap) outside either the trigger or the menu closes it, without
  // stealing focus back -- the user's click already moved it somewhere.
  useEffect(() => {
    if (!open) return;
    function handleMouseDown(e: MouseEvent): void {
      const target = e.target as Node;
      if (menuRef.current?.contains(target) || triggerRef.current?.contains(target)) return;
      onOpenChange(false);
    }
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [open, onOpenChange]);

  function close(refocusTrigger: boolean): void {
    onOpenChange(false);
    if (refocusTrigger) triggerRef.current?.focus();
  }

  function selectItem(index: number): void {
    const item = items[index];
    if (!item || item.disabled) return;
    item.onSelect();
    close(true);
  }

  function step(direction: 1 | -1): void {
    if (enabledIndexes.length === 0) return;
    const pos = enabledIndexes.indexOf(activeIndex);
    const nextPos = (pos + direction + enabledIndexes.length) % enabledIndexes.length;
    setActiveIndex(enabledIndexes[nextPos]);
  }

  function handleMenuKeyDown(e: KeyboardEvent<HTMLUListElement>): void {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        step(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        step(-1);
        break;
      case "Home":
        e.preventDefault();
        if (enabledIndexes.length > 0) setActiveIndex(enabledIndexes[0]);
        break;
      case "End":
        e.preventDefault();
        if (enabledIndexes.length > 0) setActiveIndex(enabledIndexes[enabledIndexes.length - 1]);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        selectItem(activeIndex);
        break;
      case "Escape":
        e.preventDefault();
        close(true);
        break;
    }
  }

  // Tabbing (or otherwise moving focus) out of the menu closes it -- a menu
  // must never become a keyboard trap.
  function handleMenuBlur(e: FocusEvent<HTMLUListElement>): void {
    const next = e.relatedTarget as Node | null;
    if (next && (menuRef.current?.contains(next) || triggerRef.current?.contains(next))) return;
    onOpenChange(false);
  }

  return (
    <div className="folder-menu-wrap">
      <button
        type="button"
        className="menu-trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={label}
        ref={triggerRef}
        onClick={() => onOpenChange(!open)}
      >
        <span aria-hidden="true">⋯</span>
      </button>
      {open && (
        <ul
          role="menu"
          id={menuId}
          ref={menuRef}
          className="folder-menu"
          aria-label={label}
          onKeyDown={handleMenuKeyDown}
          onBlur={handleMenuBlur}
        >
          {items.map((item, i) => (
            <li role="none" key={item.key}>
              {item.separated && <hr className="folder-menu-separator" role="none" />}
              <button
                type="button"
                role="menuitem"
                ref={(el) => {
                  itemRefs.current[i] = el;
                }}
                tabIndex={i === activeIndex ? 0 : -1}
                disabled={item.disabled}
                className={`folder-menu-item${item.destructive ? " destructive" : ""}`}
                onClick={() => selectItem(i)}
              >
                {item.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
