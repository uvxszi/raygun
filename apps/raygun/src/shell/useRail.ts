import { events } from '@raygun/bindings';
import type { RailOrder, RailPin, RailPrefs } from '@raygun/bindings';
import { useRouterState } from '@tanstack/react-router';
import { useEffect, useMemo, useRef } from 'react';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

export type PinTarget = Pick<RailPin, 'kind' | 'to' | 'search' | 'name' | 'artworkUrl'>;

const now = () => Math.floor(Date.now() / 1000);

/** Two pins are the same page: same route and same search parameters. */
export const samePin = (a: Pick<RailPin, 'to' | 'search'>, b: Pick<RailPin, 'to' | 'search'>) =>
  a.to === b.to &&
  Object.keys({ ...a.search, ...b.search }).every((key) => a.search[key] === b.search[key]);

const pin = (target: PinTarget): RailPin => ({
  ...target,
  pinnedAt: now(),
  playedAt: 0,
  openedAt: 0,
});

/** The fixed pins the first run starts with (M46.29). */
const FIXED: PinTarget[] = [
  { kind: 'favorites', to: '/library', search: { view: 'favorites' }, name: '', artworkUrl: null },
  {
    kind: 'downloaded',
    to: '/library',
    search: { view: 'downloaded' },
    name: '',
    artworkUrl: null,
  },
  { kind: 'localFiles', to: '/local-files', search: {}, name: '', artworkUrl: null },
];

export const playlistPin = (playlist: { id: string; name: string; artworkUrls: string[] }) => ({
  kind: 'playlist',
  to: '/library/playlist',
  search: { id: playlist.id },
  name: playlist.name,
  artworkUrl: playlist.artworkUrls[0] ?? null,
});

/** Pins in the chosen order: as arranged, or by when each was last played from or opened. */
export const orderedPins = (prefs: RailPrefs): RailPin[] => {
  if (prefs.order === 'custom') return prefs.pins;
  const key = prefs.order === 'played' ? 'playedAt' : 'openedAt';
  return [...prefs.pins].sort((a, b) => b[key] - a[key]);
};

/**
 * The desktop sidebar's pins (M46.29), kept in Settings' `rail`. The first run pins Favorites,
 * Downloaded, Local files and every playlist, so nothing that was there disappears.
 */
export const useRail = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const prefs: RailPrefs = settings.data?.rail ?? { pins: [], order: 'custom', seeded: false };
  const loaded = Boolean(settings.data);

  return useMemo(() => {
    const save = (next: RailPrefs) => update.mutate({ key: 'rail', value: next });
    const isPinned = (target: Pick<RailPin, 'to' | 'search'>) =>
      prefs.pins.some((existing) => samePin(existing, target));
    return {
      prefs,
      loaded,
      pins: orderedPins(prefs),
      isPinned,
      /** The first run's pins: Favorites, Downloaded, Local files and every playlist. */
      seed: (playlists: { id: string; name: string; artworkUrls: string[] }[]) =>
        save({
          ...prefs,
          seeded: true,
          pins: [...FIXED.map(pin), ...playlists.map((list) => pin(playlistPin(list)))],
        }),
      pin: (target: PinTarget) => {
        if (isPinned(target)) return;
        save({ ...prefs, pins: [...prefs.pins, pin(target)] });
      },
      unpin: (target: Pick<RailPin, 'to' | 'search'>) =>
        save({ ...prefs, pins: prefs.pins.filter((existing) => !samePin(existing, target)) }),
      /** Drops several pins at once (pages that are gone). */
      drop: (gone: RailPin[]) =>
        save({
          ...prefs,
          pins: prefs.pins.filter((existing) => !gone.some((pin) => samePin(pin, existing))),
        }),
      /** Moves a pin before another (or to the end); the order becomes custom. */
      move: (moved: RailPin, before: RailPin | null) => {
        const rest = orderedPins(prefs).filter((existing) => !samePin(existing, moved));
        const at = before ? rest.findIndex((existing) => samePin(existing, before)) : -1;
        rest.splice(at < 0 ? rest.length : at, 0, moved);
        save({ ...prefs, order: 'custom', pins: rest });
      },
      setOrder: (order: RailOrder) => save({ ...prefs, order }),
      /** Stamps when a pinned page was opened or played from. */
      touch: (target: Pick<RailPin, 'to' | 'search'>, what: 'played' | 'opened') => {
        const found = prefs.pins.find((existing) => samePin(existing, target));
        if (!found) return;
        const field = what === 'played' ? 'playedAt' : 'openedAt';
        if (now() - found[field] < 30) return;
        save({
          ...prefs,
          pins: prefs.pins.map((existing) =>
            samePin(existing, target) ? { ...existing, [field]: now() } : existing,
          ),
        });
      },
    };
    // `update` is stable enough: a new mutation object each render would loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefs, loaded]);
};

/** The page on screen as a pin's route and search (string values only). */
const here = (location: { pathname: string; search: unknown }) => ({
  to: location.pathname,
  search: Object.fromEntries(
    Object.entries((location.search ?? {}) as Record<string, unknown>)
      .filter(([, value]) => typeof value === 'string' || typeof value === 'number')
      .map(([key, value]) => [key, String(value)]),
  ),
});

/**
 * Stamps pins (M46.29): opening a pinned page sets when it was opened, and playing while on it
 * (a new queue) when it was last played from. Mounted once by the shell.
 */
export const useRailStamps = (enabled: boolean) => {
  const rail = useRail();
  const location = useRouterState({ select: (state) => state.location });
  const latest = useRef({ rail, location });
  useEffect(() => {
    latest.current = { rail, location };
  });
  const pageKey = JSON.stringify(here(location));

  useEffect(() => {
    if (enabled) latest.current.rail.touch(JSON.parse(pageKey), 'opened');
  }, [pageKey, enabled]);

  useEffect(() => {
    if (!enabled) return;
    let first: string | null = null;
    const unlisten = events.queueEvent.listen((event) => {
      // A new first song is a new queue: played from the page on screen.
      const head = event.payload.items[0]?.id ?? null;
      if (head === first) return;
      first = head;
      if (head) latest.current.rail.touch(here(latest.current.location), 'played');
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [enabled]);
};
