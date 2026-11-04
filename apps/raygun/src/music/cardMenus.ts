import { commands } from '@raygun/bindings';
import type { MixSummary, OnlineTrack, PlayContext, TrackInput } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  BookmarkMinus,
  BookmarkPlus,
  Copy,
  Download,
  LibraryBig,
  ListEnd,
  ListPlus,
  ListStart,
  Pencil,
  Play,
  RefreshCw,
  Shuffle,
  UserMinus,
  UserPlus,
  UserRound,
} from 'lucide-react';
import type { TFunction } from 'i18next';
import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  COLLECTIONS,
  fromLibraryTrack,
  fromOnlineTrack,
  fromSavedTrack,
  playCollection,
} from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { playFromLibrary } from '../library/useLibrary';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { playResults } from '../search/useOnlineSearch';
import { openContextMenu } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { creditedArtists } from './artists';
import { sameArtist, useFollowActions, useFollowedArtists } from './follows';
import {
  feedKey,
  hideArtistItem,
  notInterestedItem,
  pageKey,
  useHidden,
  useHideActions,
} from './hidden';
import type { HideTarget } from './hidden';
import { buildMenu, copyText, nameWith } from './mediaMenu';
import { useMenuSheetsStore } from './menuSheetsStore';
import { mixTitle } from './useMadeForYou';
import { downloadSongs } from './useSongDownloads';

/** Something with songs: how to play them, and the songs to queue, save or download. */
type Songs = {
  /** What they are, for messages ("Random Access Memories"). */
  name: string;
  play: (shuffle: boolean) => Promise<unknown>;
  tracks: () => Promise<TrackInput[]>;
};

/**
 * Where a card's menu shows: in one of Home's rows it also offers Not interested; `hide` adds
 * more to the hiding group (Remove from Your music).
 */
export type CardMenuOptions = { notInterested?: boolean; hide?: MenuItem[] };

/**
 * Runs a menu action that asks a source first; a failure says so in a toast (`songs`: getting
 * its songs failed, else reaching it did).
 */
const attempt = (t: TFunction, name: string, run: () => Promise<unknown>, songs = true) =>
  void run().catch(() => showToast(t(songs ? 'menu.songsFailed' : 'menu.reachFailed', { name })));

/** Play (and Shuffle), Play next, Add to queue for `songs`. */
const playItems = (t: TFunction, songs: Songs, shuffle = false): MenuItem[] => {
  const queue = (next: boolean) =>
    attempt(t, songs.name, async () => {
      const tracks = await songs.tracks();
      await unwrapResult(commands.playbackQueueTracks(tracks, next));
      showToast(t(next ? 'menu.queuedNext' : 'menu.queued', { count: tracks.length }));
    });
  return [
    {
      label: t('menu.play'),
      icon: Play,
      onSelect: () => attempt(t, songs.name, () => songs.play(false)),
    },
    ...(shuffle
      ? [
          {
            label: t('collections.shuffle'),
            icon: Shuffle,
            onSelect: () => attempt(t, songs.name, () => songs.play(true)),
          },
        ]
      : []),
    { label: t('menu.playNext'), icon: ListStart, onSelect: () => queue(true) },
    { label: t('menu.addToQueue'), icon: ListEnd, onSelect: () => queue(false) },
  ];
};

const addToPlaylistItem = (t: TFunction, songs: Songs): MenuItem => ({
  label: t('collections.addToPlaylist'),
  icon: ListPlus,
  onSelect: () =>
    useMenuSheetsStore.getState().addToPlaylist({ title: songs.name, load: songs.tracks }),
});

const downloadItem = (t: TFunction, songs: Songs): MenuItem => ({
  label: t('songs.download'),
  icon: Download,
  onSelect: () =>
    attempt(t, songs.name, async () => {
      const tracks = await songs.tracks();
      await downloadSongs(tracks);
      showToast(t('menu.downloading', { count: tracks.length }));
    }),
});

const copyNameItem = (t: TFunction, name: string, by?: string | null): MenuItem => ({
  label: t('menu.copyName'),
  icon: Copy,
  onSelect: () => copyText(nameWith(name, by)),
});

/** Plays `tracks` from the first, or from a random one. */
const playOnline = (tracks: OnlineTrack[], shuffle: boolean) => {
  if (tracks.length === 0) return Promise.reject(new Error('no songs'));
  const start = shuffle ? tracks[Math.floor(Math.random() * tracks.length)] : tracks[0];
  return playResults(tracks, start as OnlineTrack);
};

/** A source's album, or the library's (by title and artist) when there's no provider. */
export type AlbumRef = {
  title: string;
  artist: string | null;
  artworkUrl: string | null;
  provider?: { id: string } | null;
  id?: string | null;
};

/** The album's page: the source's, or the library's. */
export const albumPage = (album: AlbumRef): { to: string; search: Record<string, string> } =>
  album.provider && album.id
    ? { to: '/music/album', search: { provider: album.provider.id, id: album.id } }
    : { to: '/library/album', search: { artist: album.artist ?? '', album: album.title } };

/** A source album's songs, through the same cache as its page. */
const albumTracks = async (client: QueryClient, provider: string, id: string) =>
  (
    await client.fetchQuery({
      queryKey: ['music', 'album', provider, id],
      queryFn: () => unwrapResult(commands.musicAlbum(provider, id)),
      staleTime: 30 * 60_000,
    })
  ).tracks;

/** What Home's Not interested keeps for a page (an album, a playlist, a mix…). */
const pageTarget = (
  kind: string,
  page: { to: string; search: Record<string, unknown> },
  name: string,
  subtitle: string | null,
  artworkUrl: string | null,
): HideTarget => ({ kind, key: pageKey(page.to, page.search), name, subtitle, artworkUrl });

/** An album's menu: play, queue, add to a playlist, its artists, download, pin, edit, hide. */
export const useAlbumMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const pinItems = usePinMenuItem();
  const hidden = useHidden();
  const hideActions = useHideActions();
  return (album: AlbumRef, options: CardMenuOptions = {}): MenuItem[] => {
    const page = albumPage(album);
    const provider = album.provider?.id;
    const id = album.id;
    const online = Boolean(provider && id);
    const songs: Songs = {
      name: album.title,
      play: async (shuffle) =>
        provider && id
          ? playOnline(await albumTracks(client, provider, id), shuffle)
          : playFromLibrary({
              selection: { kind: 'album', artist: album.artist ?? '', album: album.title },
              startTrackId: null,
              shuffle,
            }),
      tracks: async () =>
        provider && id
          ? (await albumTracks(client, provider, id)).map(fromOnlineTrack)
          : (await unwrapResult(commands.libraryAlbumTracks(album.artist ?? '', album.title))).map(
              fromLibraryTrack,
            ),
    };
    const artists = creditedArtists(album.artist);
    return buildMenu({
      heading: { label: album.title, subtitle: album.artist, artworkUrl: album.artworkUrl },
      play: playItems(t, songs, true),
      save: [addToPlaylistItem(t, songs)],
      goTo: artists.map((artist) => ({
        label: t('menu.goToArtist', { name: artist }),
        icon: UserRound,
        onSelect: () => void navigate({ to: '/artist', search: { name: artist } }),
      })),
      keep: [
        online && downloadItem(t, songs),
        ...pinItems({
          kind: 'album',
          to: page.to,
          search: page.search,
          name: album.title,
          artworkUrl: album.artworkUrl,
        }),
      ],
      share: [copyNameItem(t, album.title, album.artist)],
      edit: [
        {
          label: t('details.editAlbum'),
          icon: Pencil,
          onSelect: () =>
            useMenuSheetsStore.getState().editDetails({
              kind: 'album',
              title: album.title,
              artist: album.artist,
              artworkUrl: album.artworkUrl,
            }),
        },
      ],
      hide: [
        artists[0] && hideArtistItem(artists[0], hidden, hideActions, t),
        options.notInterested &&
          notInterestedItem(
            pageTarget('album', page, album.title, album.artist, album.artworkUrl),
            hideActions,
            t,
          ),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** An artist's menu: play their popular songs, follow, edit, don't show them. */
export const useArtistMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const followed = useFollowedArtists();
  const { follow, unfollow } = useFollowActions();
  const hidden = useHidden();
  const hideActions = useHideActions();
  return (
    artist: { name: string; imageUrl?: string | null },
    options: CardMenuOptions & { goTo?: boolean } = {},
  ): MenuItem[] => {
    const { name } = artist;
    const overview = () =>
      client.fetchQuery({
        queryKey: ['music', 'artist', name],
        queryFn: () => unwrapResult(commands.musicArtist(name)),
        staleTime: 10 * 60_000,
      });
    const songs: Songs = {
      name,
      play: async (shuffle) => {
        const found = await overview();
        if (found.popular.length > 0) return playOnline(found.popular, shuffle);
        return playFromLibrary({
          selection: { kind: 'artist', artist: name },
          startTrackId: null,
          shuffle,
        });
      },
      tracks: async () => {
        const found = await overview();
        return found.popular.length > 0
          ? found.popular.map(fromOnlineTrack)
          : found.tracks.map(fromLibraryTrack);
      },
    };
    const isFollowed = (followed.data ?? []).some((other) => sameArtist(other.name, name));
    return buildMenu({
      heading: { label: name, subtitle: t('artist.kind'), artworkUrl: artist.imageUrl ?? null },
      play: playItems(t, songs, true),
      save: [
        isFollowed
          ? {
              label: t('follow.unfollow'),
              icon: UserMinus,
              onSelect: () => unfollow.mutate(name),
            }
          : {
              label: t('follow.follow'),
              icon: UserPlus,
              onSelect: () => follow.mutate({ name, imageUrl: artist.imageUrl ?? null }),
            },
      ],
      goTo: [
        options.goTo && {
          label: t('menu.goToArtist', { name }),
          icon: UserRound,
          onSelect: () => void navigate({ to: '/artist', search: { name } }),
        },
      ],
      share: [copyNameItem(t, name)],
      edit: [
        {
          label: t('details.editArtist'),
          icon: Pencil,
          onSelect: () =>
            useMenuSheetsStore
              .getState()
              .editDetails({ kind: 'artist', name, artworkUrl: artist.imageUrl ?? null }),
        },
      ],
      hide: [hideArtistItem(name, hidden, hideActions, t), ...(options.hide ?? [])],
    });
  };
};

/** A source's playlist (M46.39). */
export type SourcePlaylistRef = {
  provider: { id: string };
  id: string;
  title: string;
  author: string | null;
  artworkUrl: string | null;
};

/** A source playlist's menu: play, queue, save a copy to the library, download, pin. */
export const useSourcePlaylistMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (playlist: SourcePlaylistRef, options: CardMenuOptions = {}): MenuItem[] => {
    const page = {
      to: '/music/playlist',
      search: { provider: playlist.provider.id, id: playlist.id },
    };
    const load = async () =>
      (
        await client.fetchQuery({
          queryKey: ['music', 'playlist', playlist.provider.id, playlist.id],
          queryFn: () => unwrapResult(commands.musicPlaylist(playlist.provider.id, playlist.id)),
          staleTime: 30 * 60_000,
        })
      ).tracks;
    const songs: Songs = {
      name: playlist.title,
      play: async (shuffle) => playOnline(await load(), shuffle),
      tracks: async () => (await load()).map(fromOnlineTrack),
    };
    return buildMenu({
      heading: {
        label: playlist.title,
        subtitle: playlist.author,
        artworkUrl: playlist.artworkUrl,
      },
      play: playItems(t, songs, true),
      save: [
        addToPlaylistItem(t, songs),
        {
          label: t('playlist.save'),
          icon: LibraryBig,
          onSelect: () =>
            attempt(t, playlist.title, async () => {
              const tracks = await songs.tracks();
              const created = await unwrapResult(commands.playlistCreate(playlist.title));
              await unwrapResult(commands.playlistAdd(created.id, tracks, false));
              void client.invalidateQueries({ queryKey: [COLLECTIONS] });
              showToast(t('menu.savedToLibrary', { name: playlist.title }));
            }),
        },
      ],
      keep: [
        downloadItem(t, songs),
        ...pinItems({
          kind: 'playlist',
          to: page.to,
          search: page.search,
          name: playlist.title,
          artworkUrl: playlist.artworkUrl,
        }),
      ],
      share: [copyNameItem(t, playlist.title, playlist.author)],
      hide: [
        options.notInterested &&
          notInterestedItem(
            pageTarget('playlist', page, playlist.title, playlist.author, playlist.artworkUrl),
            hideActions,
            t,
          ),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** A Made for You mix's menu: play, queue, save as a playlist, download, pin. */
export const useMixMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (mix: MixSummary, title: string, options: CardMenuOptions = {}): MenuItem[] => {
    const page = { to: '/music/mix', search: { id: mix.id } };
    const load = async () =>
      (
        await client.fetchQuery({
          queryKey: ['music', 'forYou', 'mix', mix.id],
          queryFn: () => unwrapResult(commands.madeForYouMix(mix.id)),
        })
      ).tracks;
    const songs: Songs = {
      name: title,
      play: async (shuffle) => playOnline(await load(), shuffle),
      tracks: async () => (await load()).map(fromOnlineTrack),
    };
    const artwork = mix.artworkUrls[0] ?? null;
    return buildMenu({
      heading: { label: title, subtitle: mix.artists.join(', ') || null, artworkUrl: artwork },
      play: playItems(t, songs, true),
      save: [
        addToPlaylistItem(t, songs),
        {
          label: t('forYou.save'),
          icon: LibraryBig,
          onSelect: () =>
            attempt(t, title, async () => {
              await unwrapResult(commands.madeForYouSave(mix.id, title));
              void client.invalidateQueries({ queryKey: [COLLECTIONS] });
              showToast(t('menu.savedToLibrary', { name: title }));
            }),
        },
      ],
      keep: [
        downloadItem(t, songs),
        ...pinItems({
          kind: 'mix',
          to: page.to,
          search: page.search,
          name: title,
          artworkUrl: artwork,
        }),
      ],
      share: [copyNameItem(t, title)],
      hide: [
        options.notInterested &&
          notInterestedItem(pageTarget('mix', page, title, null, artwork), hideActions, t),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** A podcast, from the directory (its feed) or followed (its id too). */
export type PodcastRef = {
  title: string;
  author: string | null;
  /** Unknown for a Your music tile, which has only the id. */
  feedUrl?: string | null;
  artworkUrl: string | null;
  id?: string | null;
  subscribed?: boolean;
};

/** A podcast's menu: follow, pin, refresh, copy its feed, Not interested. */
export const usePodcastMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (podcast: PodcastRef, options: CardMenuOptions = {}): MenuItem[] => {
    const refresh = () => void client.invalidateQueries({ queryKey: ['podcasts'] });
    const withId = async () =>
      podcast.id ?? (await unwrapResult(commands.podcastOpenFeed(podcast.feedUrl ?? ''))).id;
    const subscribe = (subscribed: boolean) =>
      attempt(
        t,
        podcast.title,
        async () => {
          await unwrapResult(commands.podcastSetSubscribed(await withId(), subscribed));
          refresh();
        },
        false,
      );
    return buildMenu({
      heading: { label: podcast.title, subtitle: podcast.author, artworkUrl: podcast.artworkUrl },
      play: [
        {
          label: t('menu.open'),
          icon: Play,
          onSelect: () =>
            attempt(
              t,
              podcast.title,
              async () => navigate({ to: '/podcast', search: { id: await withId() } }),
              false,
            ),
        },
      ],
      save: [
        podcast.subscribed
          ? {
              label: t('podcasts.menu.unfollow'),
              icon: UserMinus,
              onSelect: () => subscribe(false),
            }
          : { label: t('podcasts.menu.follow'), icon: UserPlus, onSelect: () => subscribe(true) },
      ],
      keep: podcast.id
        ? pinItems({
            kind: 'podcast',
            to: '/podcast',
            search: { id: podcast.id },
            name: podcast.title,
            artworkUrl: podcast.artworkUrl,
          })
        : [],
      share: [
        copyNameItem(t, podcast.title),
        podcast.feedUrl && {
          label: t('podcasts.menu.copyFeed'),
          icon: Copy,
          onSelect: () => copyText(podcast.feedUrl as string),
        },
      ],
      edit: [
        podcast.subscribed &&
          podcast.id && {
            label: t('podcasts.menu.refresh'),
            icon: RefreshCw,
            onSelect: () =>
              attempt(
                t,
                podcast.title,
                async () => {
                  await unwrapResult(commands.podcastGet(podcast.id as string, true));
                  refresh();
                },
                false,
              ),
          },
      ],
      hide: [
        options.notInterested &&
          notInterestedItem(
            {
              kind: 'podcast',
              key: podcast.feedUrl
                ? feedKey(podcast.feedUrl)
                : pageKey('/podcast', { id: podcast.id }),
              name: podcast.title,
              subtitle: podcast.author,
              artworkUrl: podcast.artworkUrl,
            },
            hideActions,
            t,
          ),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** An audiobook, from a directory (its feed) or in the library (its id too). */
export type BookRef = {
  title: string;
  author: string | null;
  artworkUrl: string | null;
  feedUrl?: string | null;
  id?: string | null;
  /** In the library. */
  kept?: boolean;
};

/** A book's menu: play it, keep it in the library, pin, copy, Not interested. */
export const useBookMenuItems = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (book: BookRef, options: CardMenuOptions = {}): MenuItem[] => {
    const refresh = () => void client.invalidateQueries({ queryKey: ['audiobooks'] });
    const withId = async () =>
      book.id ?? (await unwrapResult(commands.audiobookOpen(book.feedUrl ?? '')));
    const keep = (kept: boolean) =>
      attempt(
        t,
        book.title,
        async () => {
          await unwrapResult(commands.podcastSetSubscribed(await withId(), kept));
          refresh();
          showToast(
            t(kept ? 'menu.savedToLibrary' : 'menu.removedFromLibrary', { name: book.title }),
          );
        },
        false,
      );
    return buildMenu({
      heading: { label: book.title, subtitle: book.author, artworkUrl: book.artworkUrl },
      play: [
        {
          label: t('menu.play'),
          icon: Play,
          onSelect: () =>
            attempt(
              t,
              book.title,
              async () => unwrapResult(commands.audiobookPlay(await withId(), null)),
              false,
            ),
        },
      ],
      save: [
        book.kept
          ? {
              label: t('audiobooks.removeFromLibrary'),
              icon: BookmarkMinus,
              onSelect: () => keep(false),
            }
          : { label: t('audiobooks.add'), icon: BookmarkPlus, onSelect: () => keep(true) },
      ],
      keep: book.id
        ? pinItems({
            kind: 'audiobook',
            to: '/audiobook',
            search: { id: book.id },
            name: book.title,
            artworkUrl: book.artworkUrl,
          })
        : [],
      share: [
        copyNameItem(t, book.title, book.author),
        book.feedUrl && {
          label: t('podcasts.menu.copyFeed'),
          icon: Copy,
          onSelect: () => copyText(book.feedUrl as string),
        },
      ],
      hide: [
        options.notInterested &&
          notInterestedItem(
            {
              kind: 'audiobook',
              key: book.feedUrl ? feedKey(book.feedUrl) : pageKey('/audiobook', { id: book.id }),
              name: book.title,
              subtitle: book.author,
              artworkUrl: book.artworkUrl,
            },
            hideActions,
            t,
          ),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** A comic, book or video title from a source, as Home's popular rows show them. */
export type TitleRef = {
  medium: Medium;
  sourceId: string;
  id: string;
  title: string;
  coverUrl: string | null;
};

/** The page a title opens. */
export const titlePage = (title: TitleRef) => ({
  to: ROUTES[title.medium].title,
  search: { source: title.sourceId, id: title.id },
});

/** A title's menu on Home: open, pin, copy its name, Not interested. */
export const useTitleMenuItems = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (title: TitleRef, options: CardMenuOptions = {}): MenuItem[] => {
    const page = titlePage(title);
    return buildMenu({
      heading: { label: title.title, artworkUrl: title.coverUrl },
      play: [
        {
          label: t('menu.open'),
          icon: Play,
          onSelect: () => void navigate({ to: page.to, search: page.search as never }),
        },
      ],
      keep: pinItems({
        kind: title.medium,
        to: page.to,
        search: page.search,
        name: title.title,
        artworkUrl: title.coverUrl,
      }),
      share: [copyNameItem(t, title.title)],
      hide: [
        options.notInterested &&
          notInterestedItem(
            pageTarget(title.medium, page, title.title, null, title.coverUrl),
            hideActions,
            t,
          ),
        ...(options.hide ?? []),
      ],
    });
  };
};

/** The songs of a playlist made in Raygun (a Your music tile), by its id. */
const useOwnPlaylistSongs = () => {
  const client = useQueryClient();
  return (id: string, name: string): Songs => {
    const load = async () =>
      (
        await client.fetchQuery({
          queryKey: [COLLECTIONS, 'playlist', id],
          queryFn: () => unwrapResult(commands.playlistGet(id)),
        })
      ).items.map((item) => fromSavedTrack(item.track));
    return {
      name,
      play: (shuffle) =>
        playCollection({ collection: { kind: 'playlist', id }, startIndex: null, shuffle }),
      tracks: load,
    };
  };
};

/**
 * A Your music tile's menu (M46.37): the menu of what it is (an album, a playlist, a mix, an
 * artist, a podcast, a book), and Remove from Your music.
 */
export const useContextMenuItems = () => {
  const { t } = useTranslation();
  const album = useAlbumMenuItems();
  const artist = useArtistMenuItems();
  const sourcePlaylist = useSourcePlaylistMenuItems();
  const podcast = usePodcastMenuItems();
  const book = useBookMenuItems();
  const ownPlaylist = useOwnPlaylistSongs();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (context: PlayContext): MenuItem[] => {
    const search = context.search as Record<string, string>;
    const remove = notInterestedItem(
      pageTarget(
        context.kind,
        { to: context.to, search },
        context.name,
        context.subtitle,
        context.artworkUrl,
      ),
      hideActions,
      t,
      'hidden.removeFromYourMusic',
    );
    const also = { hide: [remove] };
    switch (context.to) {
      case '/music/album':
      case '/library/album':
        return album(
          {
            title: search.album ?? context.name,
            artist: search.artist ?? context.subtitle,
            artworkUrl: context.artworkUrl,
            provider: search.provider ? { id: search.provider } : null,
            id: search.id ?? null,
          },
          also,
        );
      case '/artist':
        return artist({ name: search.name ?? context.name, imageUrl: context.artworkUrl }, also);
      case '/music/playlist':
        return sourcePlaylist(
          {
            provider: { id: search.provider ?? '' },
            id: search.id ?? '',
            title: context.name,
            author: context.subtitle,
            artworkUrl: context.artworkUrl,
          },
          also,
        );
      case '/podcast':
        return podcast(
          {
            title: context.name,
            author: context.subtitle,
            artworkUrl: context.artworkUrl,
            id: search.id ?? null,
            subscribed: true,
          },
          also,
        );
      case '/audiobook':
        return book(
          {
            title: context.name,
            author: context.subtitle,
            artworkUrl: context.artworkUrl,
            id: search.id ?? null,
            kept: true,
          },
          also,
        );
      case '/library/playlist': {
        const songs = ownPlaylist(search.id ?? '', context.name);
        return buildMenu({
          heading: { label: context.name, artworkUrl: context.artworkUrl },
          play: playItems(t, songs, true),
          save: [addToPlaylistItem(t, songs)],
          keep: [
            downloadItem(t, songs),
            ...pinItems({
              kind: 'playlist',
              to: context.to,
              search,
              name: context.name,
              artworkUrl: context.artworkUrl,
            }),
          ],
          share: [copyNameItem(t, context.name)],
          hide: [remove],
        });
      }
      default:
        return buildMenu({
          heading: {
            label: context.name,
            subtitle: context.subtitle,
            artworkUrl: context.artworkUrl,
          },
          keep: pinItems({
            kind: context.kind,
            to: context.to,
            search,
            name: context.name,
            artworkUrl: context.artworkUrl,
          }),
          share: [copyNameItem(t, context.name, context.subtitle)],
          hide: [remove],
        });
    }
  };
};

/** `onContextMenu` for mix cards: play, queue, save, download, pin (and Not interested on Home). */
export const useMixCardMenu = (options: CardMenuOptions = {}) => {
  const { t } = useTranslation();
  const items = useMixMenuItems();
  return (mix: MixSummary) => (event: MouseEvent) =>
    openContextMenu(event, items(mix, mixTitle(t, mix), options));
};

/** `onContextMenu` for anything with menu items. */
export const menuHandler = (items: () => MenuItem[]) => (event: MouseEvent) =>
  openContextMenu(event, items());
