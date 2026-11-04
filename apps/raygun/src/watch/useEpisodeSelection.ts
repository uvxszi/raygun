import { useState } from 'react';

/** Episodes picked on a title page (by their ids at the source), to download or export at once. */
export const useEpisodeSelection = () => {
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  return {
    selecting,
    selected,
    start: () => setSelecting(true),
    end: () => {
      setSelecting(false);
      setSelected(new Set());
    },
    toggle: (id: string) =>
      setSelected((current) => {
        const next = new Set(current);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      }),
    set: (ids: string[]) => setSelected(new Set(ids)),
  };
};

export type EpisodeSelection = ReturnType<typeof useEpisodeSelection>;
