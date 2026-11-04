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

/** An episode's right-click options (M46.16). */
export const episodeMenu = (
  episode: PodcastEpisode,
  t: TFunction,
  actions: EpisodeActions,
): MenuItem[] => {
  const started = !episode.played && (episode.positionMs ?? 0) > 0;
  const state = episode.download?.state;
  const queue = (next: boolean) =>
    void commands.podcastQueue(episode.id, next).then(() => actions.onQueued?.(next));
  const items: MenuItem[] = [
    { kind: 'heading', label: episode.title },
    {
      label: t(started ? 'podcasts.menu.resume' : 'menu.play'),
      icon: Play,
      onSelect: actions.play,
    },
    { label: t('menu.playNext'), icon: ListPlus, onSelect: () => queue(true) },
    { label: t('menu.addToQueue'), icon: ListEnd, onSelect: () => queue(false) },
    { kind: 'separator' },
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
  ];
  if (actions.setEarlierPlayed) {
    items.push({
      label: t(actions.chapter ? 'audiobooks.markEarlier' : 'podcasts.menu.markOlder'),
      icon: CheckCheck,
      onSelect: actions.setEarlierPlayed,
    });
  }
  if (started && actions.startOver) {
    items.push({
      label: t('podcasts.menu.startOver'),
      icon: RotateCcw,
      onSelect: actions.startOver,
    });
  }
  items.push(
    state === 'done'
      ? { label: t('podcasts.removeDownload'), icon: Trash2, onSelect: actions.removeDownload }
      : (state === 'downloading' || state === 'queued') && actions.cancelDownload
        ? { label: t('podcasts.cancelDownload'), icon: CircleX, onSelect: actions.cancelDownload }
        : { label: t('podcasts.download'), icon: Download, onSelect: actions.download },
  );
  if (actions.toggleNotes && episode.description) {
    items.push({
      label: t(actions.notesShown ? 'podcasts.menu.hideNotes' : 'podcasts.menu.showNotes'),
      icon: NotebookText,
      onSelect: actions.toggleNotes,
    });
  }
  items.push({ kind: 'separator' });
  if (actions.openPodcast) {
    items.push({
      label: t('podcasts.menu.goToShow', { name: episode.podcastTitle }),
      icon: ListVideo,
      onSelect: actions.openPodcast,
    });
  }
  items.push(
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
  );
  return items;
};

type ShowActions = {
  open: () => void;
  refresh?: () => void;
  setSubscribed?: (subscribed: boolean) => void;
};

/** A show's right-click options (M46.16). */
export const showMenu = (
  show: { title: string; feedUrl: string; subscribed: boolean },
  t: TFunction,
  actions: ShowActions,
): MenuItem[] => {
  const items: MenuItem[] = [
    { kind: 'heading', label: show.title },
    { label: t('menu.open'), icon: Rss, onSelect: actions.open },
  ];
  if (actions.refresh) {
    items.push({ label: t('podcasts.menu.refresh'), icon: RefreshCw, onSelect: actions.refresh });
  }
  if (actions.setSubscribed) {
    const setSubscribed = actions.setSubscribed;
    items.push(
      show.subscribed
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
          },
    );
  }
  items.push(
    { kind: 'separator' },
    { label: t('podcasts.menu.copyName'), icon: Copy, onSelect: () => copy(show.title) },
    { label: t('podcasts.menu.copyFeed'), icon: Copy, onSelect: () => copy(show.feedUrl) },
  );
  return items;
};
