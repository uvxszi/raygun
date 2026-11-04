import type { ArtistReleaseNews } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { BellRing, Search } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { albumCover } from '../library/coverTargets';
import { AlbumTile } from '../search/MusicFinds';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { menuHandler, useAlbumMenuItems } from './cardMenus';
import { useReleaseNews } from './follows';

/** A release in Updates → All's list: its cover, title and artist, leading to the album. */
export const ReleaseRow: FC<{ item: ArtistReleaseNews }> = ({ item }) => (
  <li>
    <Link
      to="/music/album"
      search={{ provider: item.album.provider.id, id: item.album.id }}
      data-testid="release-row"
      className="flex min-w-0 items-center gap-3 py-2"
    >
      <Artwork
        url={item.album.artworkUrl}
        fallback={albumCover(item.album)}
        className="w-10 shrink-0 rounded"
        iconSize={16}
      />
      <span className="flex min-w-0 flex-col">
        <span className="truncate text-sm font-medium">{item.album.title}</span>
        <span className="text-muted truncate text-xs">
          {[item.artist, item.album.year].filter(Boolean).join(' · ')}
        </span>
      </span>
    </Link>
  </li>
);

/** Updates → Releases (M46.42): what followed artists with alerts put out lately. */
export const ReleaseNews: FC = () => {
  const { t, i18n } = useTranslation();
  const news = useReleaseNews();
  const menu = useAlbumMenuItems();
  const list = news.data ?? [];
  if (news.error) {
    return (
      <SourceIssue compact title={t('follow.newsUnreadable')}>
        {news.error.message}
      </SourceIssue>
    );
  }
  if (news.data && list.length === 0) {
    return (
      <EmptyState
        icon={BellRing}
        title={t('follow.noNewsTitle')}
        body={t('follow.noNewsHint')}
        action={{
          label: t('follow.findArtists'),
          icon: Search,
          to: '/browse',
          search: { tab: 'search', type: 'music', view: 'artists' },
        }}
      />
    );
  }
  const day = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' });
  return (
    <ul
      data-testid="release-news"
      className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
    >
      {list.map((item) => (
        <li key={`${item.album.provider.id}:${item.album.id}`}>
          <AlbumTile
            album={item.album}
            onMenu={menuHandler(() => menu(item.album))}
            detail={[item.artist, item.foundAt ? day.format(item.foundAt) : null]
              .filter(Boolean)
              .join(' · ')}
          />
        </li>
      ))}
    </ul>
  );
};
