import { useCallback, useMemo, useState } from 'react';

/** Picking rows of a list (sources, extensions, addons) to turn on or off together. */
export const useBulkSelection = (ids: string[]) => {
  const [selecting, setSelecting] = useState(false);
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
  // Rows that left the list (filtered out, removed) aren't acted on.
  const selected = useMemo(() => ids.filter((id) => picked.has(id)), [ids, picked]);
  const toggle = useCallback(
    (id: string) =>
      setPicked((current) => {
        const next = new Set(current);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      }),
    [],
  );
  const allSelected = ids.length > 0 && selected.length === ids.length;
  return {
    selecting,
    selected,
    isSelected: (id: string) => picked.has(id),
    toggle,
    allSelected,
    toggleAll: () => setPicked(allSelected ? new Set() : new Set(ids)),
    start: () => setSelecting(true),
    stop: () => {
      setSelecting(false);
      setPicked(new Set());
    },
  };
};

export type BulkSelection = ReturnType<typeof useBulkSelection>;
