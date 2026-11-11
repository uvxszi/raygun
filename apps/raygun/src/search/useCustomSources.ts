import { useMemo, useState } from 'react';

import type { Medium } from '../manga/medium';

/** A source Custom sources' panel offers; its extension gives its tile's icon and group. */
export type SourceOption = { id: string; name: string; adult?: boolean; extensionId?: string };

const customKey = (medium: Medium) => `raygun.search.custom.${medium}`;
const chosenKey = (medium: Medium) => `raygun.search.sources.${medium}`;

const readChosen = (medium: Medium): string[] => {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(chosenKey(medium)) ?? '[]');
    return Array.isArray(saved) ? saved.filter((id): id is string => typeof id === 'string') : [];
  } catch {
    return [];
  }
};

/** Saved on or off; before there was a switch, sources picked with chips meant it was on. */
const readCustom = (medium: Medium): boolean => {
  try {
    const saved = localStorage.getItem(customKey(medium));
    if (saved !== null) return saved === '1';
  } catch {
    // A per-device convenience.
  }
  return readChosen(medium).length > 0;
};

const remember = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // A per-device convenience.
  }
};

/**
 * Which of a kind's sources a search uses, remembered per device and kind: every one while Custom
 * sources is off; while it's on, the ones switched on (sources removed since don't count).
 * Turning it on with none chosen starts with every source on, so nothing is left out until
 * switched off; the choice is kept while it's off.
 */
export const useCustomSources = (
  medium: Medium,
  available: readonly SourceOption[] | undefined,
) => {
  const [custom, setCustomState] = useState(() => readCustom(medium));
  const [chosen, setChosenState] = useState(() => readChosen(medium));
  const ids = useMemo(() => available?.map((source) => source.id), [available]);
  // Until the sources are listed, the saved choice stands as it is. The same list while nothing
  // changes, so what follows it (the panel's tiles, the search) isn't redrawn for nothing.
  const picked = useMemo(() => {
    if (!ids) return chosen;
    const listed = new Set(ids);
    return chosen.filter((id) => listed.has(id));
  }, [ids, chosen]);
  const setChosen = (next: string[]) => {
    setChosenState(next);
    remember(chosenKey(medium), JSON.stringify(next));
  };
  const setOn = (on: boolean) => {
    setCustomState(on);
    remember(customKey(medium), on ? '1' : '0');
  };
  return {
    custom,
    /** The sources switched on. */
    picked,
    setCustom: (on: boolean) => {
      setOn(on);
      if (on && picked.length === 0 && ids) setChosen(ids);
    },
    set: (id: string, on: boolean) => {
      const others = picked.filter((other) => other !== id);
      setChosen(on ? [...others, id] : others);
    },
    /** Exactly these sources on (the panel's All, None and Invert, a group's switch). */
    choose: (next: readonly string[]) => setChosen([...next]),
    /** One source alone (a row's More, a source menu's "Search only this source"). */
    only: (id: string) => {
      setOn(true);
      setChosen([id]);
    },
  };
};
