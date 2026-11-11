import { commands } from '@raygun/bindings';
import type { SkipSegment } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

const NONE: SkipSegment[] = [];

/**
 * SponsorBlock's parts of a YouTube video as Settings handle them, for YouTube's own players in
 * Raygun's window (trailers, YouTube links), which skip them themselves; none until known, and
 * none while SponsorBlock is off. `durationMs`, once the player says it, leaves out parts marked
 * on another cut of the video.
 */
export const useSponsorParts = (
  videoId: string,
  durationMs: number | null,
  enabled = true,
): SkipSegment[] => {
  const prefs = useSettings().data?.youtube?.sponsorBlock;
  // Whole seconds: reports a few milliseconds apart don't ask again.
  const seconds = durationMs && durationMs > 0 ? Math.round(durationMs / 1000) : null;
  const parts = useQuery({
    queryKey: ['sponsorblock', videoId, seconds, prefs],
    queryFn: () =>
      unwrapResult(
        commands.sponsorblockSegments(videoId, seconds === null ? null : seconds * 1000),
      ),
    enabled,
    staleTime: Infinity,
    retry: false,
    placeholderData: (previous) => previous,
  });
  return parts.data ?? NONE;
};
