import { commands } from '@raygun/bindings';
import type {
  MangaDownloadItem,
  SongDownload,
  VideoDownloadItem,
  WebsiteDownload,
} from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import {
  BookOpen,
  Clapperboard,
  Eye,
  EyeOff,
  ExternalLink,
  FileDown,
  FolderOpen,
  Pause,
  Play,
  RotateCw,
  Share2,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { playCollection } from '../collections/useCollections';
import { useLastExportFormat, useStartExport } from '../export/useExports';
import { usePlatform } from '../lib/usePlatform';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useRemoveDownloads, useSetDownloadsPaused, useDownloadChapters } from '../manga/useManga';
import { buildMenu } from '../music/mediaMenu';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import { isBusy, isFailed } from '../music/songDownloadState';
import { useSongDownloadActions } from '../music/useSongDownloads';
import type { MenuItem } from '../shell/contextMenuStore';
import { useToastStore } from '../shell/toastStore';
import {
  usePlayVideoDownload,
  useRemoveVideoDownload,
  useRetryVideoDownload,
  useSetVideoDownloadWatched,
  useSetVideoDownloadsPaused,
  videoTitleLink,
} from '../watch/useVideoLibrary';
import { videoLinkPage } from '../watch/useVideoLink';
import { useFileDownloadActions } from '../websites/useFileDownloads';
import { stopDownloadItem } from './stopDownloadItem';

const running = (state: string) => state === 'queued' || state === 'downloading';

/** "Episode 3 · Title", what a video download is called in questions. */
export const videoName = (item: VideoDownloadItem) =>
  [item.title, item.subtitle].filter(Boolean).join(' · ');

/**
 * A downloaded video's whole menu, wherever it's listed (Downloads → Videos, the desktop bar's
 * Downloads): play it, go to its title, mark it watched, save it as a file or share it, make AI
 * subtitles, delete it (asked first). One still downloading pauses the queue or stops (asked
 * first); one that failed tries again.
 */
export const useVideoDownloadMenu = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const play = usePlayVideoDownload();
  const remove = useRemoveVideoDownload();
  const retry = useRetryVideoDownload();
  const setWatched = useSetVideoDownloadWatched();
  const setPaused = useSetVideoDownloadsPaused();
  const start = useStartExport();
  const lastFormat = useLastExportFormat('video').data ?? null;
  const platform = usePlatform();
  const exportTitle = useMenuSheetsStore((state) => state.exportTitle);
  return (item: VideoDownloadItem, paused: boolean): MenuItem[] => {
    const name = videoName(item);
    const done = item.state === 'done';
    // A link's download (a link played on its own) opens the link's page.
    const page =
      item.sourceId && item.remoteId
        ? videoTitleLink(item.sourceId, item.remoteId)
        : item.key.startsWith('url:')
          ? videoLinkPage(item.key)
          : null;
    const episode = item.titleId && item.episodeId ? item.episodeId : null;
    const titleId = item.titleId;
    // The phone's share sheet; desktops have none.
    const canShare = platform === 'android' || platform === 'ios';
    return buildMenu({
      heading: { label: item.title, subtitle: item.subtitle, artworkUrl: item.poster },
      play: [
        done && {
          label: t('downloads.video.play'),
          icon: Play,
          onSelect: () => play.mutate(item.id),
        },
      ],
      save: [
        done && {
          label: t(item.watched ? 'downloads.video.markUnwatched' : 'downloads.video.markWatched'),
          icon: item.watched ? EyeOff : Eye,
          onSelect: () => setWatched.mutate({ id: item.id, watched: !item.watched }),
        },
      ],
      goTo: [
        page && {
          label: t('downloads.video.openTitle'),
          icon: Clapperboard,
          onSelect: () => void navigate({ to: page.to, search: page.search as never }),
        },
      ],
      keep: [
        done &&
          episode &&
          titleId && {
            label: t('downloads.video.export'),
            icon: FileDown,
            onSelect: () =>
              exportTitle({
                medium: 'video',
                titleId,
                episodes: [{ id: episode, name, season: null, downloaded: true }],
                only: [episode],
              }),
          },
        running(item.state) && {
          label: t(paused ? 'downloads.popover.resume' : 'downloads.popover.pause'),
          icon: paused ? Play : Pause,
          onSelect: () => setPaused.mutate(!paused),
        },
        item.state === 'failed' && {
          label: t('downloads.popover.retry'),
          icon: RotateCw,
          onSelect: () => retry.mutate(item.id),
        },
      ],
      share: [
        done &&
          canShare &&
          episode &&
          titleId && {
            label: t('downloads.video.share'),
            icon: Share2,
            onSelect: () => {
              start.mutate({
                medium: 'video',
                titleIds: [titleId],
                scope: { kind: 'chosen', ids: [episode] },
                format: lastFormat ?? 'mp4',
                destination: 'share',
                folder: null,
              });
              useToastStore.getState().show(t('downloads.video.sharing', { name }));
            },
          },
      ],
      edit: [
        done && {
          label: t('downloads.makeSubtitles'),
          icon: Sparkles,
          onSelect: () => void commands.captionsVideoMake(item.key),
        },
      ],
      remove: [
        running(item.state)
          ? stopDownloadItem(t, name, () => remove.mutate(item.id))
          : {
              label: t('downloads.video.delete'),
              icon: Trash2,
              danger: true,
              confirm: t('downloads.video.deleteAsk', { name }),
              onSelect: () => remove.mutate(item.id),
            },
      ],
    });
  };
};

/** A queued chapter's menu: read it, pause or resume the queue, try again, stop (asked first). */
export const useChapterDownloadMenu = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const remove = useRemoveDownloads();
  const retry = useDownloadChapters();
  const setPaused = useSetDownloadsPaused();
  return (item: MangaDownloadItem, paused: boolean): MenuItem[] => {
    const name = `${item.title} · ${item.chapter}`;
    const medium: Medium = item.medium === 'novel' ? 'novel' : 'manga';
    const state = item.download.state;
    return buildMenu({
      heading: { label: item.title, subtitle: item.chapter },
      play: [
        {
          label: t('downloads.popover.openChapter'),
          icon: BookOpen,
          onSelect: () =>
            void navigate({
              to: ROUTES[medium].read,
              search: { title: item.titleId, chapter: item.chapterId } as never,
            }),
        },
      ],
      keep: [
        running(state) && {
          label: t(paused ? 'downloads.popover.resume' : 'downloads.popover.pause'),
          icon: paused ? Play : Pause,
          onSelect: () => setPaused.mutate(!paused),
        },
        state === 'failed' && {
          label: t('downloads.popover.retry'),
          icon: RotateCw,
          onSelect: () => retry.mutate([item.chapterId]),
        },
      ],
      remove: [
        running(state)
          ? stopDownloadItem(t, name, () => remove.mutate([item.chapterId]))
          : {
              label: t('downloads.popover.delete'),
              icon: Trash2,
              danger: true,
              confirm: t('downloads.popover.deleteAsk', { name }),
              onSelect: () => remove.mutate([item.chapterId]),
            },
      ],
    });
  };
};

/** A song download's menu: play it, pause or resume the queue, try again, stop or delete. */
export const useSongDownloadMenu = () => {
  const { t } = useTranslation();
  const { remove, retry, setPaused } = useSongDownloadActions();
  return (song: SongDownload, paused: boolean): MenuItem[] =>
    buildMenu({
      heading: { label: song.title, subtitle: song.artist, artworkUrl: song.artworkUrl },
      play: [
        song.state === 'done' && {
          label: t('downloads.video.play'),
          icon: Play,
          onSelect: () =>
            void playCollection({
              collection: { kind: 'downloaded' },
              startIndex: 0,
              shuffle: false,
              order: [song.trackKey],
            }),
        },
      ],
      keep: [
        isBusy(song) && {
          label: t(paused ? 'downloads.popover.resume' : 'downloads.popover.pause'),
          icon: paused ? Play : Pause,
          onSelect: () => setPaused.mutate(!paused),
        },
        isFailed(song) && {
          label: t('downloads.popover.retry'),
          icon: RotateCw,
          onSelect: () => retry.mutate(song.id),
        },
      ],
      remove: [
        isBusy(song)
          ? stopDownloadItem(t, song.title, () => remove.mutate(song.id))
          : {
              label: t('downloads.popover.delete'),
              icon: Trash2,
              danger: true,
              confirm: t('downloads.popover.deleteAsk', { name: song.title }),
              onSelect: () => remove.mutate(song.id),
            },
      ],
    });
};

/** A file from a website: open it, show it in its folder, try again, stop or delete it. */
export const useFileDownloadMenu = () => {
  const { t } = useTranslation();
  const { cancel, retry, remove, open, reveal } = useFileDownloadActions();
  return (file: WebsiteDownload): MenuItem[] =>
    buildMenu({
      heading: { label: file.name, subtitle: file.site },
      play: [
        file.state === 'done' && {
          label: t('downloads.files.open'),
          icon: ExternalLink,
          onSelect: () => open.mutate(file.id),
        },
      ],
      goTo: [
        file.state === 'done' && {
          label: t('downloads.popover.showFolder'),
          icon: FolderOpen,
          onSelect: () => reveal.mutate(file.id),
        },
      ],
      keep: [
        (file.state === 'failed' || file.state === 'cancelled') && {
          label: t('downloads.popover.retry'),
          icon: RotateCw,
          onSelect: () => retry.mutate(file.id),
        },
      ],
      remove: [
        running(file.state)
          ? stopDownloadItem(t, file.name, () => cancel.mutate(file.id))
          : {
              label: t('downloads.files.delete'),
              icon: Trash2,
              danger: true,
              confirm: t('downloads.files.deleteAsk', { name: file.name }),
              onSelect: () => remove.mutate({ id: file.id, deleteFile: file.state === 'done' }),
            },
      ],
    });
};
