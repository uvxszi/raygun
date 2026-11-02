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
 */
export const TrackTagChips: FC<{ track: TrackQuery; onOpen?: () => void; className?: string }> = ({
  track,
  onOpen,
  className = '',
}) => {
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
    <ul aria-label={t('discover.tags')} className={`flex flex-wrap gap-1.5 ${className}`}>
      {tags.data.map((tag) => (
        <li key={tag.name}>
          <Link
            to="/music/tag"
            search={{ name: tag.name }}
            onClick={onOpen}
            className={`block rounded-full px-2.5 py-0.5 text-xs ${
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
