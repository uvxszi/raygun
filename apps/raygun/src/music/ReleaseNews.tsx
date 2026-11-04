import { BellRing } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AlbumTile } from '../search/MusicFinds';
import { EmptyState } from '../shell/EmptyState';
import { menuHandler, useAlbumMenuItems } from './cardMenus';
import { useReleaseNews } from './follows';

/** Updates → Releases (M46.42): what followed artists with alerts put out lately. */
export const ReleaseNews: FC<{ quiet?: boolean }> = ({ quiet = false }) => {
  const { t, i18n } = useTranslation();
  const news = useReleaseNews();
  const menu = useAlbumMenuItems();
  const list = news.data ?? [];
  if (list.length === 0) {
    return quiet ? (
      <p className="text-muted text-sm">{t('follow.noNews')}</p>
    ) : (
      <EmptyState icon={BellRing} title={t('follow.noNewsTitle')} body={t('follow.noNewsHint')} />
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
