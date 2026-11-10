import { useCallback, useEffect, useRef, useState } from 'react';

import { useBackLayer } from '../shell/backLayers';
import type { PickState } from './usePickPress';

/**
 * A list's own picks, by id (the library's titles, shows, episodes): a hold or Shift-click adds
 * everything between the last pick and this one (Mihon), Ctrl-click and the circles toggle, and
 * Select all or Invert work on what `order` shows now. Picks outlive a filter or a tab change;
 * Back and Escape end picking.
 */
export const usePickList = (order: readonly string[]) => {
  const [picked, setPicked] = useState<ReadonlySet<string>>(() => new Set());
  const last = useRef<string | null>(null);
  const shown = useRef(order);
  useEffect(() => {
    shown.current = order;
  });

  const clear = useCallback(() => {
    setPicked(new Set());
    last.current = null;
  }, []);
  const selecting = picked.size > 0;
  useBackLayer(selecting, clear);

  const toggle = (id: string) => {
    last.current = id;
    setPicked((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };
  const extend = (id: string) => {
    const from = last.current;
    const list = shown.current;
    last.current = id;
    setPicked((current) => {
      const next = new Set(current);
      const start = from !== null && current.has(from) ? list.indexOf(from) : -1;
      const end = list.indexOf(id);
      if (start >= 0 && end >= 0) {
        const [low, high] = start < end ? [start, end] : [end, start];
        for (const item of list.slice(low, high + 1)) next.add(item);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return {
    picked,
    selecting,
    has: (id: string) => picked.has(id),
    toggle,
    extend,
    selectAll: () => setPicked((current) => new Set([...current, ...shown.current])),
    invert: () =>
      setPicked((current) => {
        const next = new Set(current);
        for (const id of shown.current) {
          if (next.has(id)) next.delete(id);
          else next.add(id);
        }
        return next;
      }),
    clear,
    /** What an item of the list needs to be picked. */
    state: (id: string): PickState => ({
      selected: picked.has(id),
      selecting,
      toggle: () => toggle(id),
      extend: () => extend(id),
    }),
  };
};

export type PickList = ReturnType<typeof usePickList>;
