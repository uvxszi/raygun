import { commands } from '@raygun/bindings';
import type { ContinueWatching, VideoDownloadItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import type { TFunction } from 'i18next';
import {
  BookmarkMinus,
  BookmarkPlus,
  CheckCheck,
  CircleX,
  Download,
  ListVideo,
  LoaderCircle,
  PanelTop,
  Play,
  RotateCcw,
  SkipForward,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useMangaLibrary } from '../manga/useManga';
import { buildMenu } from '../music/mediaMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import {
  LOCAL_VIDEO,
  canChooseSource,
  isStarted,
  storedTitle,
  stremioParts,
} from './continueEntry';
import {
  STREMIO_SOURCE,
  downloadShare,
  useDownloadsByKey,
  useRemoveVideoDownload,
  useRetryVideoDownload,
  useSetStremioInLibrary,
  useSetVideoInLibrary,
  videoTitleLink,
} from './useVideoLibrary';
import { useMarkContinueWatched, useRemoveContinue } from './useWatch';

/** Where an entry is listed: Continue watching, or the watch history. */
export type ContinueListing = 'continue' | 'history';

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

type DownloadActions = {
  /** Whether it can be downloaded at all (a source's episode, not a file on this device). */
  canDownload: boolean;
  download: () => void;
  retry: (id: string) => void;
  /** Deletes or cancels the download; `done` is said once it's gone. */
  remove: (id: string, done: string) => void;
  name: string;
  t: TFunction;
};

/**
 * The menu's download lines, by where the video's download is: Download when there's none (or
 * Download again when it failed), its progress and Cancel while it's queued or downloading, Delete
 * download once it's done. Cancelling and deleting ask first.
 */
export const downloadItems = (
  saved: VideoDownloadItem | undefined,
  { canDownload, download, retry, remove, name, t }: DownloadActions,
): MenuItem[] => {
  switch (saved?.state) {
    case 'done':
      return [
        {
          label: t('watch.continueMenu.deleteDownload'),
          icon: Trash2,
          confirm: t('watch.continueMenu.deleteDownloadAsk', { name }),
          onSelect: () => remove(saved.id, t('watch.continueMenu.deleted', { name })),
        },
      ];
    case 'queued':
    case 'downloading': {
      const share = downloadShare(saved);
      return [
        {
          label:
            saved.state === 'queued'
              ? t('watch.continueMenu.waiting')
              : share === null
                ? t('watch.continueMenu.downloadingNow')
                : t('watch.continueMenu.progress', { percent: Math.floor(share * 100) }),
          icon: LoaderCircle,
          disabled: true,
          onSelect: () => undefined,
        },
        {
          label: t('watch.continueMenu.cancelDownload'),
          icon: CircleX,
          // Stopping asks first, as every download's ring does.
          confirm: t('downloads.stop.ask', { name }),
          onSelect: () => remove(saved.id, t('watch.continueMenu.cancelled', { name })),
        },
      ];
    }
    case 'failed':
      return [
        {
          label: t('watch.continueMenu.downloadAgain'),
          icon: Download,
          onSelect: () => retry(saved.id),
        },
      ];
    default:
      return canDownload
        ? [{ label: t('watch.continueMenu.download'), icon: Download, onSelect: download }]
        : [];
  }
};

/**
 * The ⋮ menu of a Continue watching entry or a watch history row: resume it, play it from the
 * start or its series' next episode, open its title, choose its source; save the title or take it
 * out, mark it watched; download it (or cancel or delete its download, see `downloadItems`), pin
 * its title; and take it out of the list, asked first. `onChooseSource` opens the source picker.
 */
export const useContinueMenu = (
  listing: ContinueListing,
  onChooseSource: (item: ContinueWatching) => void,
) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const library = useMangaLibrary('video');
  const pinItems = usePinMenuItem();
  const markWatched = useMarkContinueWatched();
  const remove = useRemoveContinue();
  const setVideoInLibrary = useSetVideoInLibrary();
  const setStremioInLibrary = useSetStremioInLibrary();
  const downloads = useDownloadsByKey();
  const removeDownload = useRemoveVideoDownload();
  const retry = useRetryVideoDownload();

  return (item: ContinueWatching): MenuItem[] => {
    const name = item.title;
    /** Runs an action; a failure says why in a toast. */
    const attempt = (run: () => Promise<unknown>) =>
      void run().catch((error: unknown) =>
        showToast(t('watch.continueMenu.failed', { name, message: message(error) })),
      );
    const failed = { onError: (error: Error) => showToast(error.message) };
    const page =
      item.sourceId && item.remoteId ? videoTitleLink(item.sourceId, item.remoteId) : null;
    const stremio = item.sourceId === STREMIO_SOURCE;
    const local = item.sourceId === LOCAL_VIDEO;
    const titleId = storedTitle(item);
    const inLibrary = Boolean(
      library.data?.some(
        (entry) => entry.sourceId === item.sourceId && entry.remoteId === item.remoteId,
      ),
    );
    const started = isStarted(item);
    const episodeName = [name, item.subtitle].filter(Boolean).join(' · ');
    const { group, episodeId } = item;

    const setInLibrary = (save: boolean) => {
      const done = {
        ...failed,
        onSuccess: () =>
          showToast(t(save ? 'menu.savedToLibrary' : 'menu.removedFromLibrary', { name })),
      };
      if (stremio && item.remoteId) {
        setStremioInLibrary.mutate({ ...stremioParts(item.remoteId), inLibrary: save }, done);
      } else if (titleId) {
        setVideoInLibrary.mutate({ titleId, inLibrary: save }, done);
      }
    };

    return buildMenu({
      heading: { label: name, subtitle: item.subtitle, artworkUrl: item.poster },
      play: [
        {
          label: t(started ? 'watch.resume' : 'watch.play'),
          icon: Play,
          onSelect: () => attempt(() => unwrapResult(commands.videoResume(item.key))),
        },
        started && {
          label: t('watch.continueMenu.fromStart'),
          icon: RotateCcw,
          onSelect: () => attempt(() => unwrapResult(commands.videoPlayFromStart(item.key))),
        },
        item.series && {
          label: t('watch.continueMenu.nextEpisode'),
          icon: SkipForward,
          onSelect: () =>
            attempt(async () => {
              if (!(await unwrapResult(commands.videoPlayNext(item.key)))) {
                showToast(t('watch.continueMenu.noNextEpisode', { name }));
              }
            }),
        },
        page && {
          label: t('menu.openPage'),
          icon: PanelTop,
          onSelect: () => void navigate({ to: page.to, search: page.search as never }),
        },
        canChooseSource(item) && {
          label: t('watch.continueMenu.chooseSource'),
          icon: ListVideo,
          onSelect: () => onChooseSource(item),
        },
      ],
      save: [
        page &&
          (stremio || titleId) &&
          (inLibrary
            ? {
                label: t('menu.removeFromLibrary'),
                icon: BookmarkMinus,
                onSelect: () => setInLibrary(false),
              }
            : {
                label: t('menu.addToLibrary'),
                icon: BookmarkPlus,
                onSelect: () => setInLibrary(true),
              }),
        !item.watched && {
          label: t('menu.markWatched'),
          icon: CheckCheck,
          onSelect: () =>
            markWatched.mutate(item.key, {
              ...failed,
              onSuccess: () => showToast(t('menu.markedWatched', { name: episodeName })),
            }),
        },
      ],
      keep: [
        ...downloadItems(downloads.get(item.key), {
          canDownload: Boolean(group && episodeId && !local),
          download: () =>
            attempt(async () => {
              if (!group || !episodeId) return;
              // It finds its best link when its turn comes, as a title's episodes do.
              const queued = await unwrapResult(commands.videoDownloadEpisodes(group, [episodeId]));
              await client.invalidateQueries({ queryKey: ['video', 'downloads'] });
              showToast(
                t(
                  queued > 0
                    ? 'watch.continueMenu.downloading'
                    : 'watch.continueMenu.downloadedAlready',
                  { name: episodeName },
                ),
              );
            }),
          retry: (id) => retry.mutate(id, failed),
          remove: (id, done) =>
            removeDownload.mutate(id, { ...failed, onSuccess: () => showToast(done) }),
          name: episodeName,
          t,
        }),
        ...(page
          ? pinItems({
              kind: 'video',
              to: page.to,
              search: page.search,
              name,
              artworkUrl: item.poster,
            })
          : []),
      ],
      remove: [
        {
          label: t(
            listing === 'history'
              ? 'watch.continueMenu.removeHistory'
              : 'watch.continueMenu.remove',
          ),
          icon: Trash2,
          confirm: t(
            listing === 'history' ? 'watch.continueMenu.removeHistoryAsk' : 'watch.removeContinue',
            { title: episodeName },
          ),
          onSelect: () => remove.mutate(item.key, failed),
        },
      ],
    });
  };
};
