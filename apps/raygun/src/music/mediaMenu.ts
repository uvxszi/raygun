import type { MenuItem } from '../shell/contextMenuStore';

/** An item, or nothing when it doesn't apply to this thing (`a && item` with an empty `a`). */
type Maybe = MenuItem | false | '' | 0 | null | undefined;

/**
 * Every music card's and row's menu (songs, albums, artists, playlists, mixes, podcasts, books)
 * in one order, so things are where the hand expects them: playing it, saving it, where it leads,
 * keeping it, sharing it, editing it, then hiding it. Each group shows only what applies, with a
 * line between groups.
 */
export type MenuGroups = {
  heading: { label: string; subtitle?: string | null; artworkUrl?: string | null };
  /** Play (or Shuffle, Resume), Play next, Add to queue. */
  play?: Maybe[];
  /** Add to playlist, Save to or Remove from the library (Favorites, Follow, Keep). */
  save?: Maybe[];
  /** Go to the album, the artists, the show. */
  goTo?: Maybe[];
  /** Download or remove the download, Pin to the sidebar. */
  keep?: Maybe[];
  /** Copy link (the heading's title and line under it copy when held). */
  share?: Maybe[];
  /** Edit details, and the kind's own tools. */
  edit?: Maybe[];
  /** Don't show this artist, Not interested. */
  hide?: Maybe[];
  /** Removing or deleting it. */
  remove?: Maybe[];
};

const ORDER = ['play', 'save', 'goTo', 'keep', 'share', 'edit', 'hide', 'remove'] as const;

export const buildMenu = (groups: MenuGroups): MenuItem[] => {
  const items: MenuItem[] = [{ kind: 'heading', ...groups.heading }];
  for (const name of ORDER) {
    const shown = (groups[name] ?? []).filter((item): item is MenuItem => Boolean(item));
    if (shown.length === 0) continue;
    if (items.length > 1) items.push({ kind: 'separator' });
    items.push(...shown);
  }
  return items;
};

/** Copies `text`, quietly when the clipboard isn't there. */
export const copyText = (text: string) =>
  void navigator.clipboard?.writeText(text).catch(() => undefined);
