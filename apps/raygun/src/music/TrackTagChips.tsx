import { commands } from '@raygun/bindings';
import type { TrackQuery } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

/**
 * A song's genres and moods as links to their pages (M46.9), found when first shown if the
 * background tagger hasn't reached the song yet. Nothing shows while they load or when none.
 * `compact` chips (the side panel's) are smaller and sit closer, so a row holds more of them.
 */
export const TrackTagChips: FC<{
  track: TrackQuery;
  onOpen?: () => void;
  compact?: boolean;
  className?: string;
}> = ({ track, onOpen, compact = false, className = '' }) => {
  const { t } = useTranslation();
  const tags = useQuery({
    queryKey: ['music', 'tags', 'track', track.title, track.artist],
    queryFn: () =>
      unwrapResult(
        commands.musicTrackTags({
          title: track.title,
          artist: track.artist,
          album: track.album,
          durationMs: track.durationMs,
        }),
      ),
    enabled: track.title !== '',
    staleTime: Infinity,
    retry: false,
  });
  if (!tags.data?.length) return null;
  return (
    <ul
      aria-label={t('discover.tags')}
      className={`flex flex-wrap ${compact ? 'gap-1' : 'gap-1.5'} ${className}`}
    >
      {tags.data.map((tag) => (
        // A chip stays one line; a name wider than the row ends in an ellipsis.
        <li key={tag.name} className="max-w-full min-w-0">
          <Link
            to="/music/tag"
            search={{ name: tag.name }}
            onClick={onOpen}
            className={`block truncate rounded-full ${
              compact ? 'px-2 py-px text-[0.6875rem] leading-4' : 'px-2.5 py-0.5 text-xs'
            } ${
              tag.kind === 'mood'
                ? 'bg-primary/15 text-primary'
                : 'bg-foreground/10 text-foreground hover:bg-foreground/20'
            }`}
          >
            {tag.name}
          </Link>
        </li>
      ))}
    </ul>
  );
};
