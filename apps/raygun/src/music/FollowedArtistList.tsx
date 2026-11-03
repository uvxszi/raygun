import { Link } from '@tanstack/react-router';
import { BellRing } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { artistCover } from '../library/coverTargets';
import { useFollowedArtists } from './follows';

/** Library → Audio → Artists: the artists followed (M46.42), before the library's own. */
export const FollowedArtistList: FC = () => {
  const { t } = useTranslation();
  const followed = useFollowedArtists();
  const list = followed.data ?? [];
  if (list.length === 0) return null;
  return (
    <section
      aria-label={t('follow.followed')}
      data-testid="followed-artists"
      className="flex flex-col gap-1"
    >
      <h2 className="text-muted text-sm font-semibold">{t('follow.followed')}</h2>
      <ul className="flex flex-col">
        {list.map((artist) => (
          <li key={artist.name}>
            <Link
              to="/artist"
              search={{ name: artist.name }}
              className="flex items-center gap-3 py-2"
            >
              <Artwork
                url={artist.imageUrl}
                fallback={artistCover(artist.name)}
                className="size-12 rounded-full"
                iconSize={20}
              />
              <span className="min-w-0 flex-1 truncate font-medium">{artist.name}</span>
              {artist.alerts && (
                <BellRing
                  role="img"
                  aria-label={t('follow.alertsOnNote')}
                  size={16}
                  className="text-primary shrink-0"
                />
              )}
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
};
