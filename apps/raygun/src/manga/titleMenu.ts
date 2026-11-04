import { commands } from '@raygun/bindings';
import type {
  MangaLibraryEntry,
  MangaTitleView,
  VideoEpisodeView,
  VideoTitleView,
} from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  BookOpen,
  BookmarkMinus,
  BookmarkPlus,
  CheckCheck,
  Copy,
  Download,
  FileOutput,
  Link as LinkIcon,
  PanelTop,
  Play,
  Search,
} from 'lucide-react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import type { CardMenuOptions } from '../music/cardMenus';
import { notInterestedItem, pageKey, useHideActions } from '../music/hidden';
import { buildMenu, copyText } from '../music/mediaMenu';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { STREMIO_SOURCE, videoTitleLink } from '../watch/useVideoLibrary';
import { ROUTES, catalogCommands, isLocalSource, libraryCommand } from './medium';
import type { Medium, ReadingMedium } from './medium';

/**
 * A comic, book or video as a card shows it: in a source's listing, a search, a Watch row, the
 * library or Continue watching. Stremio titles have the source `stremio` and the id
 * `<type>:<id>`.
 */
export type CardTitle = {
  medium: Medium;
  sourceId: string;
  /** The source's id for the title. */
  id: string;
  title: string;
  coverUrl: string | null;
  subtitle?: string | null;
  /** In the library, when the card knows (else the library list says). */
  inLibrary?: boolean;
  /** The stored title's id, when the card knows it (the library's). */
  titleId?: string;
  /** Where it opens, when not the medium's title page. */
  link?: { to: string; search: Record<string, string> };
};

/** What a card adds to the menu: Select (inside a selection scope), Add to a group. */
export type TitleMenuOptions = CardMenuOptions & {
  select?: MenuItem;
  addToGroup?: MenuItem;
  viewCover?: MenuItem;
};

/** At most this many episodes are looked up and queued by one Download all. */
const MAX_EPISODES = 200;

const stremioParts = (id: string) => {
  const split = id.indexOf(':');
  return { kind: id.slice(0, split), id: id.slice(split + 1) };
};

/** The page a card opens. */
export const cardPage = (card: CardTitle): { to: string; search: Record<string, string> } =>
  card.link ??
  (card.medium === 'video'
    ? videoTitleLink(card.sourceId, card.id)
    : { to: ROUTES[card.medium].title, search: { source: card.sourceId, id: card.id } });

const libraryEntries = (client: QueryClient, medium: Medium) =>
  client.fetchQuery({
    queryKey: [medium, 'library'],
    queryFn: () => unwrapResult(libraryCommand(medium)()),
  });

/** The library's entry for a card, if it's saved (what the library list has now). */
const savedEntry = (client: QueryClient, card: CardTitle): MangaLibraryEntry | undefined =>
  client
    .getQueryData<MangaLibraryEntry[]>([card.medium, 'library'])
    ?.find((entry) => entry.sourceId === card.sourceId && entry.remoteId === card.id);

/** A source video's stored title (asked of the source the first time; the title page's query). */
const videoView = (client: QueryClient, card: CardTitle) =>
  client.fetchQuery({
    queryKey: ['video', 'title', card.sourceId, card.id],
    queryFn: () => unwrapResult(commands.videoTitle(card.sourceId, card.id, false)),
  });

const readingView = (client: QueryClient, card: CardTitle, medium: ReadingMedium) =>
  client.fetchQuery({
    queryKey: [medium, 'title', card.sourceId, card.id],
    queryFn: () => unwrapResult(catalogCommands(medium).title(card.sourceId, card.id, false)),
  });

/** The stored title's id: a Stremio title has one only once it's in the library. */
const storedId = async (client: QueryClient, card: CardTitle): Promise<string | null> => {
  const known = card.titleId ?? savedEntry(client, card)?.titleId;
  if (known) return known;
  if (card.sourceId === STREMIO_SOURCE) {
    const entries = await libraryEntries(client, 'video');
    return (
      entries.find((entry) => entry.sourceId === STREMIO_SOURCE && entry.remoteId === card.id)
        ?.titleId ?? null
    );
  }
  if (card.medium === 'video') return (await videoView(client, card)).id;
  return (await readingView(client, card, card.medium)).id;
};

/** Everything that shows saved titles or their progress, after a change from a menu. */
const refresh = (client: QueryClient, medium: Medium) =>
  Promise.all([
    client.invalidateQueries({ queryKey: [medium, 'library'] }),
    client.invalidateQueries({ queryKey: [medium, 'title'] }),
    client.invalidateQueries({ queryKey: [medium, 'browse'] }),
    ...(medium === 'video'
      ? [
          client.invalidateQueries({ queryKey: ['stremio', 'meta'] }),
          client.invalidateQueries({ queryKey: ['watch'] }),
        ]
      : []),
  ]);

/**
 * Queues `episodes` of a source's title for download without picking links: each finds its best
 * link when its turn comes (two download at a time). Returns how many were queued.
 */
const downloadEpisodes = (view: VideoTitleView, episodes: VideoEpisodeView[]) =>
  unwrapResult(
    commands.videoDownloadEpisodes(
      `title:${view.id}`,
      episodes.slice(0, MAX_EPISODES).map((episode) => episode.remoteId),
    ),
  );

/** The episodes Download next takes: from where watching continues, the first not saved. */
const nextEpisodes = (view: VideoTitleView) => {
  const from = Math.max(
    0,
    view.episodes.findIndex((episode) => episode.id === view.continueEpisodeId),
  );
  return view.episodes.slice(from).filter((episode) => !episode.downloaded && !episode.watched);
};

/** Where reading continues: the partly read or next chapter, else the first (sources list newest first). */
const readingStart = (view: MangaTitleView) =>
  view.continueChapterId ?? view.chapters[view.chapters.length - 1]?.id ?? null;

/** Runs a menu action; a failure says so in a toast, naming the title. */
const attempt = (t: TFunction, name: string, run: () => Promise<unknown>) =>
  void run().catch(() => showToast(t('menu.reachFailed', { name })));

/**
 * Plays or reads a card's title: a video through a watching session (the next episode, the
 * best link), a comic or book at the chapter it continues at.
 */
export const usePlayTitle = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  return (card: CardTitle) =>
    attempt(t, card.title, async () => {
      if (card.sourceId === STREMIO_SOURCE) {
        const { kind, id } = stremioParts(card.id);
        await unwrapResult(commands.stremioWatch(kind, id, null));
        return;
      }
      if (card.medium === 'video') {
        const view = await videoView(client, card);
        await unwrapResult(commands.videoSourceWatch(view.id, null));
        return;
      }
      const view = await readingView(client, card, card.medium);
      const chapter = readingStart(view);
      if (!chapter) {
        showToast(t('menu.noChapters', { name: card.title }));
        return;
      }
      await navigate({
        to: ROUTES[card.medium].read,
        search: { title: view.id, chapter } as never,
      });
    });
};

/**
 * The full menu of a comic, book or video card (M46.8 order): play or read, open its page; save
 * it or take it out, mark it watched or read; find it in other sources; download, export, pin;
 * copy its link and name; Not interested.
 */
export const useTitleCardMenu = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const play = usePlayTitle();
  const pinItems = usePinMenuItem();
  const hideActions = useHideActions();
  return (card: CardTitle, options: TitleMenuOptions = {}): MenuItem[] => {
    const page = cardPage(card);
    const video = card.medium === 'video';
    const stremio = card.sourceId === STREMIO_SOURCE;
    const local = isLocalSource(card.sourceId);
    const inLibrary = card.inLibrary ?? Boolean(savedEntry(client, card));
    const name = card.title;

    const setInLibrary = (save: boolean) =>
      attempt(t, name, async () => {
        if (stremio) {
          const { kind, id } = stremioParts(card.id);
          await unwrapResult(commands.stremioSetInLibrary(kind, id, save));
        } else if (video) {
          const view = await videoView(client, card);
          await unwrapResult(commands.videoSetInLibrary(view.id, save));
        } else {
          const view = await readingView(client, card, card.medium as ReadingMedium);
          await unwrapResult(
            catalogCommands(card.medium as ReadingMedium).setInLibrary(view.id, save),
          );
        }
        await refresh(client, card.medium);
        showToast(t(save ? 'menu.savedToLibrary' : 'menu.removedFromLibrary', { name }));
      });

    const markDone = () =>
      attempt(t, name, async () => {
        const titleId = await storedId(client, card);
        if (titleId) {
          await unwrapResult(commands.libraryMarkRead(card.medium, [titleId], true));
        } else if (stremio) {
          // A Stremio movie not in the library: its own progress entry is marked.
          const { kind, id } = stremioParts(card.id);
          await unwrapResult(commands.stremioSetWatched(kind, id, name, null, card.coverUrl, true));
        }
        await refresh(client, card.medium);
        showToast(t(video ? 'menu.markedWatched' : 'menu.markedRead', { name }));
      });

    const downloadVideo = (which: 'next' | 'all') =>
      attempt(t, name, async () => {
        const view = await videoView(client, card);
        const episodes =
          which === 'next'
            ? nextEpisodes(view).slice(0, 1)
            : view.episodes.filter((episode) => !episode.downloaded);
        if (episodes.length === 0) {
          showToast(t('menu.nothingToDownload', { name }));
          return;
        }
        const queued = await downloadEpisodes(view, episodes);
        await client.invalidateQueries({ queryKey: ['video', 'downloads'] });
        showToast(
          queued > 0
            ? t('menu.downloadingEpisodes', { count: queued, name })
            : t('menu.noDownloadLinks', { name }),
        );
      });

    const downloadChapters = (amount: number | null) =>
      attempt(t, name, async () => {
        const titleId = await storedId(client, card);
        if (!titleId) return;
        const queued = await unwrapResult(
          commands.libraryDownloadNext(card.medium, [titleId], amount),
        );
        showToast(
          queued > 0
            ? t('menu.downloadingChapters', { count: queued, name })
            : t('menu.nothingToDownload', { name }),
        );
      });

    const exportIt = () =>
      attempt(t, name, async () => {
        const titleId = await storedId(client, card);
        if (titleId) useMenuSheetsStore.getState().exportTitle({ medium: card.medium, titleId });
        else showToast(t('menu.saveFirst', { name }));
      });

    const copyLink = () =>
      attempt(t, name, async () => {
        const view =
          card.medium === 'video'
            ? await videoView(client, card)
            : await readingView(client, card, card.medium);
        if (view.url) {
          copyText(view.url);
          showToast(t('menu.linkCopied'));
        } else {
          showToast(t('menu.noLink', { name }));
        }
      });

    const downloads: MenuItem[] = local
      ? []
      : video
        ? stremio
          ? []
          : [
              {
                label: t('menu.downloadNextEpisode'),
                icon: Download,
                onSelect: () => downloadVideo('next'),
              },
              {
                label: t('menu.downloadAllEpisodes'),
                icon: Download,
                onSelect: () => downloadVideo('all'),
              },
            ]
        : [
            {
              label: t('menu.downloadNextChapter'),
              icon: Download,
              onSelect: () => downloadChapters(1),
            },
            {
              label: t('menu.downloadUnread'),
              icon: Download,
              onSelect: () => downloadChapters(null),
            },
          ];

    return buildMenu({
      heading: { label: name, subtitle: card.subtitle, artworkUrl: card.coverUrl },
      play: [
        options.select,
        {
          label: t(video ? 'menu.play' : 'menu.read'),
          icon: video ? Play : BookOpen,
          onSelect: () => play(card),
        },
        {
          label: t('menu.openPage'),
          icon: PanelTop,
          onSelect: () => void navigate({ to: page.to, search: page.search as never }),
        },
      ],
      save: [
        inLibrary
          ? {
              label: t('menu.removeFromLibrary'),
              icon: BookmarkMinus,
              onSelect: () => setInLibrary(false),
            }
          : {
              label: t('menu.addToLibrary'),
              icon: BookmarkPlus,
              onSelect: () => setInLibrary(true),
            },
        options.addToGroup,
        options.viewCover,
        (inLibrary || !stremio || !stremioParts(card.id).kind.includes('series')) && {
          label: t(video ? 'menu.markWatched' : 'menu.markRead'),
          icon: CheckCheck,
          onSelect: markDone,
        },
      ],
      goTo: [
        !local && {
          label: t('menu.findInOtherSources'),
          icon: Search,
          onSelect: () =>
            void navigate({
              to: '/browse',
              search: { tab: 'search', type: card.medium, q: name } as never,
            }),
        },
      ],
      keep: [
        ...downloads,
        (!stremio || inLibrary) && {
          label: t('menu.export'),
          icon: FileOutput,
          onSelect: exportIt,
        },
        ...pinItems({
          kind: card.medium,
          to: page.to,
          search: page.search,
          name,
          artworkUrl: card.coverUrl,
        }),
      ],
      share: [
        !stremio &&
          !local && { label: t('menu.copySourceLink'), icon: LinkIcon, onSelect: copyLink },
        { label: t('menu.copyTitle'), icon: Copy, onSelect: () => copyText(name) },
      ],
      hide: [
        notInterestedItem(
          {
            kind: card.medium,
            key: pageKey(page.to, page.search),
            name,
            subtitle: card.subtitle ?? null,
            artworkUrl: card.coverUrl,
          },
          hideActions,
          t,
        ),
        ...(options.hide ?? []),
      ],
    });
  };
};
