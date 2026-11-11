import { commands, events } from '@raygun/bindings';
import type { SkipSegment, SkipSegments } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { useCurrentItemId } from '../player/usePlayback';
import { showToast } from '../shell/toastStore';

const SKIP_SEGMENTS_KEY = ['playback', 'skipSegments'] as const;
const NONE: SkipSegment[] = [];

/**
 * SponsorBlock's parts of the song playing (of its YouTube video), for its seek bar; none for a
 * song that isn't a YouTube video's. `useSongSkips` keeps them current.
 */
export const useSkipSegments = (): SkipSegment[] => {
  const itemId = useCurrentItemId();
  const segments = useQuery({
    queryKey: SKIP_SEGMENTS_KEY,
    queryFn: () => commands.playbackSkipSegments(),
    staleTime: Infinity,
  }).data;
  return segments && segments.itemId === itemId ? segments.segments : NONE;
};

/**
 * Follows Rust's skips of the song playing, hosted once by the app shell: the parts it has (for
 * the seek bars), and a note for each part skipped, with Undo, which plays it after all.
 */
export const useSongSkips = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  useEffect(() => {
    const parts = events.skipSegmentsEvent.listen(({ payload }) => {
      queryClient.setQueryData<SkipSegments | null>(SKIP_SEGMENTS_KEY, payload);
    });
    const skipped = events.skippedSegmentEvent.listen(({ payload }) => {
      const part = payload.segment;
      showToast(
        t('sponsorBlock.skipped', { part: t(`sponsorBlock.categories.${part.category}`) }),
        () => void commands.playbackUndoSkip(part.id),
      );
    });
    return () => {
      void parts.then((stop) => stop());
      void skipped.then((stop) => stop());
    };
  }, [queryClient, t]);
};
