import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { createPortal } from 'react-dom';

/** Space kept between the popover and the window's edges, and from its button. */
const MARGIN = 8;

type Place = { top: number; left: number; maxHeight: number };

/** Open popovers, oldest first: a choice inside a popover opens another above it. */
const open: HTMLElement[] = [];
/** Whether `node` is in a popover opened after `panel` (one it opened). */
const inLaterPopover = (panel: HTMLElement, node: Node) =>
  open.slice(open.indexOf(panel) + 1).some((later) => later.contains(node));

/**
 * Where a popover of `width` goes beside `anchor`: under it when it fits (or there's more room
 * there), else above; lined up with its left edge, or its right edge near the window's right.
 */
const placeBeside = (anchor: DOMRect, width: number, height: number): Place => {
  const below = window.innerHeight - anchor.bottom - MARGIN * 2;
  const above = anchor.top - MARGIN * 2;
  const under = height <= below || below >= above;
  const room = Math.max(under ? below : above, 120);
  const shown = Math.min(height, room);
  const top = under ? anchor.bottom + MARGIN : anchor.top - MARGIN - shown;
  const leftAligned = anchor.left;
  const rightAligned = anchor.right - width;
  const left = leftAligned + width + MARGIN <= window.innerWidth ? leftAligned : rightAligned;
  return {
    top: Math.max(MARGIN, top),
    left: Math.min(Math.max(MARGIN, left), window.innerWidth - width - MARGIN),
    maxHeight: room,
  };
};

type PopoverProps = {
  /** The button that opened it: the popover sits beside it, and pressing it again closes it. */
  anchor: HTMLElement;
  label: string;
  onClose: () => void;
  children: ReactNode;
  /** In px. */
  width?: number;
};

/**
 * A small panel beside the button that opened it (M46.7), for menus and choices on wide windows:
 * a click anywhere else, Escape or the button again closes it; nothing behind it is dimmed.
 */
export const Popover: FC<PopoverProps> = ({ anchor, label, onClose, children, width = 320 }) => {
  const panel = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<Place | null>(null);

  // Follows the button as the page scrolls or the window changes size.
  useLayoutEffect(() => {
    let frame = 0;
    const follow = () => {
      const box = anchor.getBoundingClientRect();
      const height = panel.current?.scrollHeight ?? 0;
      const next = placeBeside(box, width, height);
      setPlace((current) =>
        current &&
        current.top === next.top &&
        current.left === next.left &&
        current.maxHeight === next.maxHeight
          ? current
          : next,
      );
      frame = requestAnimationFrame(follow);
    };
    follow();
    return () => cancelAnimationFrame(frame);
  }, [anchor, width]);

  useEffect(() => {
    const self = panel.current;
    if (!self) return;
    open.push(self);
    return () => {
      open.splice(open.indexOf(self), 1);
    };
  }, []);

  useEffect(() => {
    // Pointer down outside closes; on the button, its own click does (so it toggles).
    const outside = (event: PointerEvent) => {
      const self = panel.current;
      const target = event.target as Node | null;
      if (!self || !target || self.contains(target) || anchor.contains(target)) return;
      if (inLaterPopover(self, target)) return;
      onClose();
    };
    // Escape closes the newest popover only, and nothing behind it (a sheet it sits in).
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || open.at(-1) !== panel.current) return;
      // A search field with text empties first (TextField); the next Escape closes.
      const field = event.target;
      if (field instanceof HTMLInputElement && field.value !== '' && field.dataset.clearsOnEscape) {
        return;
      }
      event.stopPropagation();
      onClose();
      anchor.focus();
    };
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('keydown', escape, true);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('keydown', escape, true);
    };
  }, [anchor, onClose]);

  useEffect(() => {
    panel.current?.focus({ preventScroll: true });
  }, []);

  return createPortal(
    <div
      ref={panel}
      role="dialog"
      aria-label={label}
      tabIndex={-1}
      data-testid="popover"
      style={{
        top: place?.top ?? -9999,
        left: place?.left ?? -9999,
        width,
        maxHeight: place?.maxHeight,
      }}
      className="bg-surface motion-pop fixed z-[70] flex flex-col overflow-y-auto rounded-2xl p-2 shadow-2xl ring-1 shadow-black/40 ring-white/5 outline-none"
    >
      {children}
    </div>,
    document.body,
  );
};
