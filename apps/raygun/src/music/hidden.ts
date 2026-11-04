import { commands } from '@raygun/bindings';
import type { HiddenItem, HiddenPrefs, Settings } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import type { TFunction } from 'i18next';
import { Eye, CircleMinus, UserRoundX } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { SETTINGS_KEY } from '../settings/useSettings';
import type { MenuItem } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { creditedArtists } from './artists';
import { trackKey } from './trackKey';

/**
 * Hidden artists and "Not interested" (Spotify's "Don't play this artist"): kept in Rust
 * (`Settings::hidden`), which leaves them out of mixes, charts, genre rows and Your music. Home's
 * rows check here too, so a change shows at once.
 */

/** A name as compared: case and runs of spaces don't count (as Rust's `hidden::fold`). */
const fold = (name: string) => name.trim().split(/\s+/).join(' ').toLowerCase();

/** A page's address as a hidden item's key (as Rust's `hidden::page_key`). */
export const pageKey = (to: string, search: Record<string, unknown> = {}) =>
  `${to}?${Object.keys(search)
    .filter((key) => search[key] !== undefined && search[key] !== null)
    .sort()
    .map((key) => `${key}=${String(search[key])}`)
    .join('&')}`;

/** A feed's key, for podcasts and books not opened yet. */
export const feedKey = (feedUrl: string) => `feed:${feedUrl}`;

/** What can be marked Not interested: everything but when it was. */
export type HideTarget = Omit<HiddenItem, 'hiddenAt'>;

/** A song as a Not interested target. */
export const songTarget = (song: {
  title: string;
  artist: string | null;
  artworkUrl?: string | null;
}): HideTarget => ({
  kind: 'song',
  key: trackKey(song.title, song.artist),
  name: song.title,
  subtitle: song.artist,
  artworkUrl: song.artworkUrl ?? null,
});

/** The words that join artists in a credit, as Rust's `credits_include` knows them. */
const SEPARATORS = [
  ', ',
  ' & ',
  ' feat. ',
  ' feat ',
  ' ft. ',
  ' ft ',
  ' featuring ',
  ' x ',
  ' and ',
];

/** Whether folded `credit` names `name` whole, between separators ("Simon & Garfunkel feat. X"). */
const namesWhole = (credit: string, name: string) => {
  for (let at = credit.indexOf(name); at >= 0; at = credit.indexOf(name, at + 1)) {
    const before = credit.slice(0, at);
    const after = credit.slice(at + name.length);
    if (
      (before === '' || SEPARATORS.some((separator) => before.endsWith(separator))) &&
      (after === '' || SEPARATORS.some((separator) => after.startsWith(separator)))
    ) {
      return true;
    }
  }
  return false;
};

export type HiddenFilter = {
  isEmpty: boolean;
  hidesArtist: (name: string | null | undefined) => boolean;
  /** A credit ("A, B & C feat. D") naming a hidden artist. */
  hidesCredit: (credit: string | null | undefined) => boolean;
  /** A song by a hidden artist, or marked Not interested. */
  hidesSong: (song: { title: string; artist: string | null }) => boolean;
  hidesItem: (key: string) => boolean;
};

/** What's hidden, ready to check songs and rows against (as Rust's `hidden::Hidden`). */
export const hiddenFilter = (prefs: HiddenPrefs | undefined): HiddenFilter => {
  const artists = new Set(
    (prefs?.artists ?? []).map((artist) => fold(artist.name)).filter(Boolean),
  );
  const compound = [...artists].filter((name) => creditedArtists(name).length > 1);
  const items = new Set((prefs?.items ?? []).map((item) => item.key));
  const hidesArtist = (name: string | null | undefined) =>
    Boolean(name) && artists.has(fold(name as string));
  const hidesCredit = (credit: string | null | undefined) => {
    if (!credit?.trim() || artists.size === 0) return false;
    const folded = fold(credit);
    return (
      artists.has(folded) ||
      creditedArtists(credit).some((part) => artists.has(fold(part))) ||
      compound.some((name) => namesWhole(folded, name))
    );
  };
  return {
    isEmpty: artists.size === 0 && items.size === 0,
    hidesArtist,
    hidesCredit,
    hidesSong: (song) =>
      hidesCredit(song.artist) || (items.size > 0 && items.has(trackKey(song.title, song.artist))),
    hidesItem: (key) => items.has(key),
  };
};

/**
 * What's hidden now, from the settings. Rows only follow the hidden part, so other settings
 * changing (the sidebar's pins, a panel's size) don't redraw every list.
 */
export const useHidden = (): HiddenFilter => {
  const hidden = useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => unwrapResult(commands.settingsGet()),
    select: (settings: Settings) => settings.hidden,
  }).data;
  return useMemo(() => hiddenFilter(hidden), [hidden]);
};

const now = () => Math.floor(Date.now() / 1000);

/** Answers that leave hidden things out, asked again after a change. */
const askAgain = (client: QueryClient) => {
  void client.invalidateQueries({ queryKey: ['music'] });
  void client.invalidateQueries({ queryKey: ['recentContexts'] });
};

/**
 * Hides and shows again, at once on screen (the settings in the cache change first), then saved
 * in Rust; each says so in a toast with Undo.
 */
export const useHideActions = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  return useMemo(() => {
    const save = (change: (prefs: HiddenPrefs) => HiddenPrefs, run: () => Promise<Settings>) => {
      client.setQueryData<Settings>(SETTINGS_KEY, (settings) =>
        settings
          ? { ...settings, hidden: change(settings.hidden ?? { artists: [], items: [] }) }
          : settings,
      );
      void run()
        .then((settings) => {
          client.setQueryData(SETTINGS_KEY, settings);
          askAgain(client);
        })
        .catch(() => {
          void client.invalidateQueries({ queryKey: SETTINGS_KEY });
          showToast(t('hidden.saveFailed'));
        });
    };
    const setArtist = (name: string, hidden: boolean) =>
      save(
        (prefs) => ({
          ...prefs,
          artists: [
            ...prefs.artists.filter((artist) => fold(artist.name) !== fold(name)),
            ...(hidden ? [{ name, hiddenAt: now() }] : []),
          ],
        }),
        () => unwrapResult(commands.hiddenArtistSet(name, hidden)),
      );
    const setItem = (item: HideTarget, hidden: boolean) => {
      const stamped: HiddenItem = { ...item, hiddenAt: now() };
      save(
        (prefs) => ({
          ...prefs,
          items: [
            ...prefs.items.filter((other) => other.key !== item.key),
            ...(hidden ? [stamped] : []),
          ],
        }),
        () => unwrapResult(commands.hiddenItemSet(stamped, hidden)),
      );
    };
    return {
      hideArtist: (name: string) => {
        setArtist(name, true);
        showToast(t('hidden.artistHidden', { name }), () => setArtist(name, false));
      },
      showArtist: (name: string) => {
        setArtist(name, false);
        showToast(t('hidden.artistShown', { name }), () => setArtist(name, true));
      },
      notInterested: (item: HideTarget) => {
        setItem(item, true);
        showToast(t('hidden.itemHidden', { name: item.name }), () => setItem(item, false));
      },
      interested: (item: HideTarget) => setItem(item, false),
    };
  }, [client, t]);
};

export type HideActions = ReturnType<typeof useHideActions>;

/** "Don't show <artist>", or "Show <artist> again" when they're hidden. */
export const hideArtistItem = (
  name: string,
  hidden: HiddenFilter,
  actions: HideActions,
  t: TFunction,
): MenuItem =>
  hidden.hidesArtist(name)
    ? {
        label: t('hidden.showArtist', { name }),
        icon: Eye,
        onSelect: () => actions.showArtist(name),
      }
    : {
        label: t('hidden.hideArtist', { name }),
        icon: UserRoundX,
        onSelect: () => actions.hideArtist(name),
      };

/** "Not interested": Home doesn't show it again. */
export const notInterestedItem = (
  target: HideTarget,
  actions: HideActions,
  t: TFunction,
  label = 'hidden.notInterested',
): MenuItem => ({
  label: t(label),
  icon: CircleMinus,
  onSelect: () => actions.notInterested(target),
});
