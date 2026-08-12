import { useEffect, useId, useMemo, useRef, useState, type JSX } from "react";

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
}

interface Props<T extends string | number> {
  /** Forwarded to the trigger button so an external <label htmlFor> can name it. */
  id?: string;
  value: T | null;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  placeholder?: string;
  ariaLabel?: string;
  /**
   * Greys the control out and blocks opening or changing it — e.g. duplex and
   * colour mode when the selected printer's driver does not support them.
   * The control stays mounted and reachable (a real `disabled` button, not a
   * removed one), so assistive technology still announces it as disabled
   * rather than the setting silently vanishing.
   */
  disabled?: boolean;
}

/** House searchable-select combobox, ported from tabsy's SearchSelect. */
export default function SearchSelect<T extends string | number>({
  id,
  value,
  onChange,
  options,
  placeholder = "Auswählen …",
  ariaLabel,
  disabled = false,
}: Props<T>): JSX.Element {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selected = options.find((o) => o.value === value) ?? null;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q === "" ? options : options.filter((o) => o.label.toLowerCase().includes(q));
  }, [options, query]);

  useEffect(() => {
    if (!open) return;
    function onDocClick(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  useEffect(() => {
    setActive(0);
  }, [query, open]);

  // A control that becomes disabled while open (the capability probe can
  // resolve after the user already opened it) must close immediately.
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  function choose(opt: SelectOption<T>): void {
    onChange(opt.value);
    setOpen(false);
    setQuery("");
  }

  function onKeyDown(e: React.KeyboardEvent): void {
    if (disabled) return;
    if (e.key === "Escape") {
      setOpen(false);
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActive((a) => Math.min(a + 1, filtered.length - 1));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const opt = filtered[active];
      if (opt) choose(opt);
      return;
    }
  }

  return (
    <div className="ss" ref={rootRef}>
      <button
        type="button"
        id={id}
        className="ss-control input"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => {
          if (disabled) return;
          setOpen((o) => !o);
        }}
      >
        <span className={selected ? "" : "ss-placeholder"}>
          {selected ? selected.label : placeholder}
        </span>
        <span className="ss-caret" aria-hidden="true">
          ▾
        </span>
      </button>
      {open && !disabled && (
        <div className="ss-pop">
          <input
            className="input ss-search"
            autoFocus
            placeholder="Suchen …"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            role="combobox"
            aria-controls={listId}
            aria-expanded={true}
          />
          <ul className="ss-list" id={listId} role="listbox">
            {filtered.length === 0 && <li className="ss-empty">Keine Treffer</li>}
            {filtered.map((opt, i) => (
              <li
                key={String(opt.value)}
                role="option"
                aria-selected={opt.value === value}
                className={
                  "ss-option" +
                  (i === active ? " active" : "") +
                  (opt.value === value ? " selected" : "")
                }
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  choose(opt);
                }}
              >
                {opt.label}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
