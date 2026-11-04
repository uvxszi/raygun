import { commands } from '@raygun/bindings';
import type { PodcastEpisode } from '@raygun/bindings';
import type { TFunction } from 'i18next';
import {
  Check,
  CheckCheck,
  CircleX,
  Copy,
  Download,
  ListEnd,
  ListPlus,
  ListVideo,
  NotebookText,
  Play,
  RefreshCw,
  RotateCcw,
  Rss,
  Trash2,
  UserMinus,
  UserPlus,
} from 'lucide-react';

import { buildMenu } from '../music/mediaMenu';
import type { MenuItem } from '../shell/contextMenuStore';

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => undefined);

type EpisodeActions = {
  /** A book's chapter: finished rather than played, earlier chapters rather than older ones. */
  chapter?: boolean;
  play: () => void;
  setPlayed: (played: boolean) => void;
  /** Marks the episodes before this one played (M46.48). */
  setEarlierPlayed?: () => void;
  /** Forgets where listening stopped (M46.48); only for started episodes. */
  startOver?: () => void;
  download: () => void;
  cancelDownload?: () => void;
  removeDownload: () => void;
  toggleNotes?: () => void;
  notesShown?: boolean;
  /** Leads to the show's page; left out on that page. */
  openPodcast?: () => void;
  /** The queue changed: show it. */
  onQueued?: (next: boolean) => void;
};

/**
 * An episode's right-click options (M46.16), in the order every media menu keeps: play it, play
 * it next or last; mark it played; go to the show; download it; copy its names; its notes.
 */
export const episodeMenu = (
  episode: PodcastEpisode,
  t: TFunction,
  actions: EpisodeActions,
): MenuItem[] => {
  const started = !episode.played && (episode.positionMs ?? 0) > 0;
  const state = episode.download?.state;
  const queue = (next: boolean) =>
    void commands.podcastQueue(episode.id, next).then(() => actions.onQueued?.(next));
  return buildMenu({
    heading: {
      label: episode.title,
      subtitle: episode.podcastTitle,
      artworkUrl: episode.artworkUrl,
    },
    play: [
      {
        label: t(started ? 'podcasts.menu.resume' : 'menu.play'),
        icon: Play,
        onSelect: actions.play,
      },
      { label: t('menu.playNext'), icon: ListPlus, onSelect: () => queue(true) },
      { label: t('menu.addToQueue'), icon: ListEnd, onSelect: () => queue(false) },
    ],
    save: [
      {
        label: t(
          actions.chapter
            ? episode.played
              ? 'audiobooks.markUnfinished'
              : 'audiobooks.markFinished'
            : episode.played
              ? 'podcasts.markUnplayed'
              : 'podcasts.markPlayed',
        ),
        icon: Check,
        onSelect: () => actions.setPlayed(!episode.played),
      },
      actions.setEarlierPlayed && {
        label: t(actions.chapter ? 'audiobooks.markEarlier' : 'podcasts.menu.markOlder'),
        icon: CheckCheck,
        onSelect: actions.setEarlierPlayed,
      },
      started &&
        actions.startOver && {
          label: t('podcasts.menu.startOver'),
          icon: RotateCcw,
          onSelect: actions.startOver,
        },
    ],
    goTo: [
      actions.openPodcast && {
        label: t('podcasts.menu.goToShow', { name: episode.podcastTitle }),
        icon: ListVideo,
        onSelect: actions.openPodcast,
      },
    ],
    keep: [
      state === 'done'
        ? { label: t('podcasts.removeDownload'), icon: Trash2, onSelect: actions.removeDownload }
        : (state === 'downloading' || state === 'queued') && actions.cancelDownload
          ? { label: t('podcasts.cancelDownload'), icon: CircleX, onSelect: actions.cancelDownload }
          : { label: t('podcasts.download'), icon: Download, onSelect: actions.download },
    ],
    share: [
      {
        label: t(actions.chapter ? 'audiobooks.copyChapter' : 'podcasts.menu.copyTitle'),
        icon: Copy,
        onSelect: () => copy(episode.title),
      },
      {
        label: t(actions.chapter ? 'audiobooks.copyBook' : 'podcasts.menu.copyShow'),
        icon: Copy,
        onSelect: () => copy(episode.podcastTitle),
      },
    ],
    edit: [
      actions.toggleNotes &&
        episode.description && {
          label: t(actions.notesShown ? 'podcasts.menu.hideNotes' : 'podcasts.menu.showNotes'),
          icon: NotebookText,
          onSelect: actions.toggleNotes,
        },
    ],
  });
};

type ShowActions = {
  open: () => void;
  refresh?: () => void;
  setSubscribed?: (subscribed: boolean) => void;
  /** Pin to the sidebar (desktop), for a show with a page. */
  pin?: MenuItem[];
  /** Not interested, in Home's rows. */
  hide?: MenuItem[];
};

/** A show's right-click options (M46.16), in the order every media menu keeps. */
export const showMenu = (
  show: { title: string; author?: string | null; feedUrl: string; subscribed: boolean },
  t: TFunction,
  actions: ShowActions,
): MenuItem[] => {
  const setSubscribed = actions.setSubscribed;
  return buildMenu({
    heading: { label: show.title, subtitle: show.author ?? null },
    play: [{ label: t('menu.open'), icon: Rss, onSelect: actions.open }],
    save: [
      setSubscribed &&
        (show.subscribed
          ? {
              label: t('podcasts.menu.unfollow'),
              icon: UserMinus,
              danger: true,
              onSelect: () => setSubscribed(false),
            }
          : {
              label: t('podcasts.menu.follow'),
              icon: UserPlus,
              onSelect: () => setSubscribed(true),
            }),
    ],
    keep: actions.pin ?? [],
    share: [
      { label: t('podcasts.menu.copyName'), icon: Copy, onSelect: () => copy(show.title) },
      { label: t('podcasts.menu.copyFeed'), icon: Copy, onSelect: () => copy(show.feedUrl) },
    ],
    edit: [
      actions.refresh && {
        label: t('podcasts.menu.refresh'),
        icon: RefreshCw,
        onSelect: actions.refresh,
      },
    ],
    hide: actions.hide ?? [],
  });
};
