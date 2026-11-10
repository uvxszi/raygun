import type { QuickAccessItem, RailPin } from '@raygun/bindings';
import { useNavigate, useRouter } from '@tanstack/react-router';
import { ArrowDown, ArrowUp, Pin, PinOff, Trash2 } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { useDeletePlaylist, usePlaylists } from '../collections/useCollections';
import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import type { FixedTile } from '../library/useFixedTileMenu';
import { useFixedTileMenuItems } from '../library/useFixedTileMenu';
import { useFollowActions } from '../music/follows';
import { useSetSubscribed } from '../podcasts/usePodcasts';
import { useWebsites } from '../websites/useWebsites';
import type { QuickMenuItem } from './QuickAccessMenu';
import { showToast } from './toastStore';
import { pageKey, useQuickAccess, useRail } from './useRail';

const FIXED_KINDS: string[] = ['favorites', 'downloaded', 'localFiles'];

/**
 * Rust's list with the pins as they are now: a pin made, dropped or moved a moment ago shows at
 * once, before Rust lists again. With `pinnedFirst`, the pins in their order and the rest apart;
 * without, everything in Rust's order.
 */
const arrangeShown = (items: QuickAccessItem[], pins: RailPin[], pinnedFirst: boolean) => {
  const pinKeys = pins.map(pageKey);
  const pinned = new Set(pinKeys);
  const now = items.flatMap((item) => {
    const isPinned = pinned.has(pageKey(item));
    // Unpinned, a page outside the library leaves.
    if (!isPinned && item.pinned && !item.library) return [];
    return [{ ...item, pinned: isPinned }];
  });
  if (!pinnedFirst) return { top: [] as QuickAccessItem[], rest: now };
  const byKey = new Map(now.map((item) => [pageKey(item), item]));
  const top = [...new Set(pinKeys)].flatMap((key) => {
    const item = byKey.get(key);
    return item ? [item] : [];
  });
  return { top, rest: now.filter((item) => !item.pinned) };
};

/** Menus with the parts of other menus: no separators at the ends or twice in a row. */
const tidy = (items: QuickMenuItem[]) => {
  const kept: QuickMenuItem[] = [];
  for (const item of items) {
    const last = kept.at(-1);
    const after = !last || last.kind === 'separator' || last.kind === 'heading';
    if (item.kind === 'separator' && after) continue;
    kept.push(item);
  }
  while (kept.at(-1)?.kind === 'separator') kept.pop();
  return kept;
};

/** Takes an item out of the library: a playlist is deleted, an artist or a show unfollowed. */
const useRemoveFromLibrary = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const navigate = useNavigate();
  const deletePlaylist = useDeletePlaylist();
  const { unfollow } = useFollowActions();
  const setSubscribed = useSetSubscribed();
  return (item: QuickAccessItem, name: string) => {
    const done = { onSuccess: () => showToast(t('menu.removedFromLibrary', { name })) };
    const id = item.search.id ?? '';
    if (item.kind === 'playlist') {
      const here = router.state.location;
      const open =
        here.pathname === '/library/playlist' && (here.search as { id?: string }).id === id;
      deletePlaylist.mutate(id, {
        onSuccess: () => {
          done.onSuccess();
          if (open) void navigate({ to: '/library', search: { view: 'playlists' } as never });
        },
      });
    } else if (item.kind === 'artist') {
      unfollow.mutate(item.search.name ?? item.name, done);
    } else if (item.kind === 'podcast' || item.kind === 'audiobook') {
      setSubscribed.mutate({ id, subscribed: false }, done);
    }
  };
};

/**
 * Quick access as it shows (M46.29): the desktop sidebar, the phone's drawer and Home's Quick
 * access row list the same items in the same order. `top` are the pins kept on top (in the
 * order they're dragged into), `rest` the audio library by itself (Rust's `quick_access`). Each
 * item has its name now (a playlist's or a site's own) and its menu: Pin or Unpin, moving a pin,
 * the item's own options and Remove from library for what can leave it. `shown` false leaves the
 * list unasked (a closed drawer).
 */
export const useQuickAccessItems = (shown = true) => {
  const { t } = useTranslation();
  const rail = useRail();
  const quick = useQuickAccess(shown);
  const playlists = usePlaylists();
  const playlistMenuItems = usePlaylistMenuItems();
  const fixedMenuItems = useFixedTileMenuItems();
  const removeFromLibrary = useRemoveFromLibrary();
  const websites = useWebsites();

  const pinnedFirst = rail.prefs.pinnedFirst;
  const items = quick.data?.items;
  const { top, rest } = useMemo(
    () => arrangeShown(items ?? [], rail.prefs.pins, pinnedFirst),
    [items, rail.prefs.pins, pinnedFirst],
  );
  const playlistById = useMemo(
    () => new Map((playlists.data ?? []).map((list) => [list.id, list])),
    [playlists.data],
  );
  const playlistOf = (item: QuickAccessItem) =>
    item.kind === 'playlist' ? playlistById.get(item.search.id ?? '') : undefined;
  const siteOf = (item: QuickAccessItem) =>
    item.kind === 'website'
      ? websites.data?.find((site) => site.id === item.search.site)
      : undefined;
  const nameOf = (item: QuickAccessItem) =>
    playlistOf(item)?.name ?? siteOf(item)?.name ?? item.name;

  const ownItemsOf = (item: QuickAccessItem): QuickMenuItem[] => {
    // Favorites', Downloaded's and Local files' own: play, open, their tools, sort.
    if (FIXED_KINDS.includes(item.kind)) {
      return [
        { kind: 'separator' },
        ...fixedMenuItems(item.kind as FixedTile, { pin: false }).slice(1),
      ];
    }
    // A playlist's own (M46.45), but its Delete: Remove from library is that.
    const playlist = playlistOf(item);
    if (!playlist) return [];
    const remove = t('collections.delete');
    return [
      { kind: 'separator' },
      ...playlistMenuItems(playlist, { pin: false })
        .slice(1)
        .filter(
          (entry) =>
            entry.kind === 'separator' || entry.kind === 'heading' || entry.label !== remove,
        ),
    ];
  };

  const menuItems = (item: QuickAccessItem): QuickMenuItem[] => {
    const name = nameOf(item);
    const at = top.findIndex((other) => pageKey(other) === pageKey(item));
    const moves: QuickMenuItem[] =
      item.pinned && at >= 0
        ? [
            ...(at > 0
              ? [
                  {
                    label: t('rail.moveUp'),
                    icon: ArrowUp,
                    onSelect: () => rail.move(item, top[at - 1] ?? null),
                  },
                ]
              : []),
            ...(at < top.length - 1
              ? [
                  {
                    label: t('rail.moveDown'),
                    icon: ArrowDown,
                    onSelect: () => rail.move(item, top[at + 2] ?? null),
                  },
                ]
              : []),
          ]
        : [];
    return tidy([
      { kind: 'heading', label: name },
      item.pinned
        ? { label: t('quickAccess.unpin'), icon: PinOff, onSelect: () => rail.unpin(item) }
        : {
            label: t('quickAccess.pin'),
            icon: Pin,
            onSelect: () =>
              rail.pin({
                kind: item.kind,
                to: item.to,
                search: item.search,
                name: item.name,
                artworkUrl: item.artworkUrl,
              }),
          },
      ...moves,
      ...ownItemsOf(item),
      ...(item.removable
        ? [
            { kind: 'separator' as const },
            {
              label: t('menu.removeFromLibrary'),
              icon: Trash2,
              danger: true,
              confirm: t(
                item.kind === 'playlist'
                  ? 'quickAccess.removePlaylistConfirm'
                  : 'quickAccess.removeConfirm',
                { name },
              ),
              onSelect: () => removeFromLibrary(item, name),
            },
          ]
        : []),
    ]);
  };

  return { rail, quick, top, rest, playlistOf, siteOf, nameOf, menuItems };
};
