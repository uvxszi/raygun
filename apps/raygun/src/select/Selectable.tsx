import { Check } from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { FC, MouseEvent, PointerEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useBackLayer } from '../shell/backLayers';
import { HOLD_MS, ScopeContext, useInSelectionScope } from './scopeContext';
import { pickedKey, useSelectionStore } from './selectionStore';
import type { Picked } from './selectionStore';
import { SelectionBar } from './SelectionBar';

/** How far a finger may move during the hold before it counts as a scroll, px. */
const HOLD_SLOP = 10;

/**
 * Results inside it can be picked (search, a source's page). It hosts the selection's bar, and
 * the selection goes when it closes, on Back and on Escape.
 */
export const SelectionScope: FC<{ children: ReactNode }> = ({ children }) => {
  const selecting = useSelectionStore((state) => state.items.length > 0);
  const clear = useSelectionStore((state) => state.clear);
  useBackLayer(selecting, clear);
  useEffect(() => clear, [clear]);
  return (
    <ScopeContext.Provider value>
      {children}
      {selecting && <SelectionBar />}
    </ScopeContext.Provider>
  );
};

type Props = {
  picked: Picked;
  children: ReactNode;
  className?: string;
  /** The box sits over a cover's corner (grids) or over the artwork at the row's start. */
  layout?: 'cover' | 'row';
  /** Shown as it is, not pickable (a library cover among results). */
  disabled?: boolean;
};

/**
 * A result that can be picked: a long press (touch), Ctrl/Cmd/Shift-click or its box picks it,
 * and while anything is picked a tap adds or takes out instead of opening. Outside a scope it's
 * just its children.
 */
export const Selectable: FC<Props> = (props) =>
  useInSelectionScope() && !props.disabled ? <SelectableItem {...props} /> : <>{props.children}</>;

const SelectableItem: FC<Props> = ({ picked, children, className = '', layout = 'cover' }) => {
  const { t } = useTranslation();
  const key = pickedKey(picked);
  const selected = useSelectionStore((state) => state.keys.has(key));
  const selecting = useSelectionStore((state) => state.items.length > 0);
  const toggle = useSelectionStore((state) => state.toggle);
  const hold = useRef<{ timer: ReturnType<typeof setTimeout>; x: number; y: number } | null>(null);
  // The hold picked it: the click (and the touch's context menu) that follow are swallowed.
  const held = useRef(false);
  const touch = useRef(false);

  const cancelHold = () => {
    if (hold.current) clearTimeout(hold.current.timer);
    hold.current = null;
  };
  useEffect(() => cancelHold, []);

  const pick = () => {
    held.current = true;
    toggle(picked);
    navigator.vibrate?.(10);
  };

  const onPointerDown = (event: PointerEvent) => {
    touch.current = event.pointerType !== 'mouse';
    held.current = false;
    if (!touch.current) return;
    cancelHold();
    hold.current = {
      timer: setTimeout(() => {
        hold.current = null;
        pick();
      }, HOLD_MS),
      x: event.clientX,
      y: event.clientY,
    };
  };
  const onPointerMove = (event: PointerEvent) => {
    const start = hold.current;
    if (start && Math.hypot(event.clientX - start.x, event.clientY - start.y) > HOLD_SLOP) {
      cancelHold();
    }
  };
  const onClickCapture = (event: MouseEvent) => {
    const modified = event.ctrlKey || event.metaKey || event.shiftKey;
    if (held.current) {
      held.current = false;
    } else if (selecting || modified) {
      toggle(picked);
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  };
  // A finger's long press is the selection's, not the card's menu (Android sends both).
  const onContextMenuCapture = (event: MouseEvent) => {
    if (!touch.current) return;
    event.preventDefault();
    event.stopPropagation();
    if (!held.current) {
      cancelHold();
      pick();
    }
  };

  const box = (
    <button
      type="button"
      role="checkbox"
      aria-checked={selected}
      aria-label={t('search.select.pick', { title: pickedTitle(picked) })}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        toggle(picked);
      }}
      className={`z-10 grid size-6 shrink-0 place-items-center rounded-full border-2 shadow transition-opacity ${
        selected
          ? 'border-primary bg-primary text-background'
          : 'border-white/90 bg-black/40 text-transparent'
      } ${
        selecting || selected
          ? 'opacity-100'
          : 'pointer-events-none opacity-0 group-hover/pick:pointer-events-auto group-hover/pick:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100'
      } ${layout === 'cover' ? 'absolute top-1.5 right-1.5' : 'absolute top-1/2 left-2 -translate-y-1/2'}`}
    >
      <Check aria-hidden size={14} strokeWidth={3} />
    </button>
  );

  return (
    <div
      data-selected={selected || undefined}
      data-testid="selectable"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={cancelHold}
      onPointerCancel={cancelHold}
      onPointerLeave={cancelHold}
      onClickCapture={onClickCapture}
      onContextMenuCapture={onContextMenuCapture}
      className={`group/pick relative rounded-xl transition-colors [-webkit-touch-callout:none] ${
        selected ? 'bg-primary/15 ring-primary ring-2' : ''
      } ${className}`}
    >
      {children}
      {box}
    </div>
  );
};

const pickedTitle = (picked: Picked) =>
  picked.kind === 'song' ? picked.track.title : picked.title;
