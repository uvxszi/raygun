import { commands } from '@raygun/bindings';
import type { ContinueWatching } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  BookmarkMinus,
  BookmarkPlus,
  CheckCheck,
  Download,
  ListVideo,
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
  useSetStremioInLibrary,
  useSetVideoInLibrary,
  videoTitleLink,
} from './useVideoLibrary';
import { useMarkContinueWatched, useRemoveContinue } from './useWatch';

/** Where an entry is listed: Continue watching, or the watch history. */
export type ContinueListing = 'continue' | 'history';

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * The ⋮ menu of a Continue watching entry or a watch history row: resume it, play it from the
 * start or its series' next episode, open its title, choose its source; save the title or take it
 * out, mark it watched; download it, pin its title; and take it out of the list, asked first.
 * `onChooseSource` opens the source picker for it.
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
        group &&
          episodeId &&
          !local && {
            label: t('watch.continueMenu.download'),
            icon: Download,
            onSelect: () =>
              attempt(async () => {
                // It finds its best link when its turn comes, as a title's episodes do.
                const queued = await unwrapResult(
                  commands.videoDownloadEpisodes(group, [episodeId]),
                );
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
          },
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
