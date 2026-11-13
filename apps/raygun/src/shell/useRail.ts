import { commands } from '@raygun/bindings';
import type { QuickAccessNames, RailOrder, RailPin, RailPrefs } from '@raygun/bindings';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { QUICK_ACCESS_KEY, samePin, withRailDefaults } from './railPrefs';
import type { Page, PinTarget } from './railPrefs';

export {
  FIXED_PINS,
  pageKey,
  playlistPin,
  QUICK_ACCESS_KEY,
  RAIL_DEFAULTS,
  RAIL_ORDERS,
  RAIL_SHOWN,
  samePin,
  withRailDefaults,
} from './railPrefs';
export type { PinTarget, QuickAccessPrefs } from './railPrefs';

const now = () => Math.floor(Date.now() / 1000);

const pinOf = (target: PinTarget): RailPin => ({
  kind: target.kind,
  to: target.to,
  search: target.search,
  name: target.name,
  artworkUrl: target.artworkUrl,
  pinnedAt: now(),
  playedAt: 0,
  openedAt: 0,
  auto: false,
});

/**
 * Quick access's settings and pins (`Settings::rail`): pins stay on top in the user's order, and
 * the audio library lists itself around them (Rust's `quick_access`). Every change is shown at
 * once and saved; Rust then says quick access changed.
 */
export const useRail = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const client = useQueryClient();
  const stored = settings.data?.rail;
  const loaded = Boolean(settings.data);

  return useMemo(() => {
    const prefs = withRailDefaults(stored);
    // Once saved, quick access is listed again (Rust says so too): a page pinned from its top bar
    // isn't in the list shown yet.
    const save = (next: RailPrefs) =>
      update.mutate(
        { key: 'rail', value: next },
        { onSettled: () => void client.invalidateQueries({ queryKey: QUICK_ACCESS_KEY }) },
      );
    const isPinned = (target: Page) => prefs.pins.some((existing) => samePin(existing, target));
    return {
      prefs,
      loaded,
      isPinned,
      /** Pinned at the end of the pins. */
      pin: (target: PinTarget) => {
        if (isPinned(target)) return;
        save({ ...prefs, pins: [...prefs.pins, pinOf(target)] });
      },
      unpin: (target: Page) =>
        save({ ...prefs, pins: prefs.pins.filter((existing) => !samePin(existing, target)) }),
      /** Pins pages at once, after the other pins, or takes their pins away (a selection's Pin). */
      setPinned: (targets: PinTarget[], pinned: boolean) => {
        if (!pinned) {
          const pins = prefs.pins.filter(
            (existing) => !targets.some((target) => samePin(existing, target)),
          );
          if (pins.length !== prefs.pins.length) save({ ...prefs, pins });
          return;
        }
        const fresh = targets.filter(
          (target, index) =>
            !isPinned(target) && targets.findIndex((other) => samePin(other, target)) === index,
        );
        if (fresh.length > 0) save({ ...prefs, pins: [...prefs.pins, ...fresh.map(pinOf)] });
      },
      /** Moves a pin before another (or to the end of the pins). */
      move: (moved: Page, before: Page | null) => {
        const pinned = prefs.pins.find((existing) => samePin(existing, moved));
        if (!pinned) return;
        const rest = prefs.pins.filter((existing) => !samePin(existing, moved));
        const at = before ? rest.findIndex((existing) => samePin(existing, before)) : -1;
        rest.splice(at < 0 ? rest.length : at, 0, pinned);
        save({ ...prefs, pins: rest });
      },
      setOrder: (order: RailOrder) => save({ ...prefs, order }),
      /** The phone's drawer: names beside the pins, or icons only. */
      setDrawerExpanded: (drawerExpanded: boolean) => save({ ...prefs, drawerExpanded }),
      /** Changes some of quick access's settings. */
      set: (patch: Partial<RailPrefs>) => save({ ...prefs, ...patch }),
    };
    // `update` is stable enough: a new mutation object each render would loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stored, loaded, client]);
};

/** The fixed blocks' names in this language, which Rust sorts by. */
export const useQuickAccessNames = (): QuickAccessNames => {
  const { t } = useTranslation();
  return {
    favorites: t('collections.favorites'),
    downloaded: t('library.view.downloaded'),
    localFiles: t('localFiles.title'),
  };
};

/**
 * What quick access lists now (Rust's `quick_access_list`): the pins first when they stay on top,
 * then the audio library in the chosen order, at most `shown` of it. Rust says when it changed
 * (`QuickAccessChangedEvent`, in `useLiveInvalidation`).
 */
export const useQuickAccess = (enabled = true) => {
  const names = useQuickAccessNames();
  return useQuery({
    queryKey: [...QUICK_ACCESS_KEY, names],
    queryFn: () => unwrapResult(commands.quickAccessList(names)),
    enabled,
    // A new language or a refresh keeps the list on screen until the answer comes.
    placeholderData: keepPreviousData,
  });
};
