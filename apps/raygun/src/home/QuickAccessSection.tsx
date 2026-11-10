import type { PlaylistSummary, QuickAccessItem, Website } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ArrowDown, Folder, Heart, Pin } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { playCollection, fromSavedTrack, useFavorites } from '../collections/useCollections';
import { TrackMoreButton } from '../collections/TrackMoreButton';
import { Artwork } from '../library/Artwork';
import { CARD_MENU_BESIDE, CARD_MENU_BESIDE_TIGHT } from '../manga/CardPlayButton';
import { useHidden } from '../music/hidden';
import { CardMenu } from '../shell/CardMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { PinArt } from '../shell/RailPins';
import { useQuickAccessItems } from '../shell/useQuickAccessItems';
import { pageKey } from '../shell/useRail';
import { useOpenWebsite } from '../websites/useWebsites';
import { HomeCarousel, HomeSection, RowSkeleton } from './HomeSection';

/** Quick access's items shown on Home at most: its pins, then the most recent. */
const HOME_SHOWN = 40;
/** The latest favorite songs shown under them. */
const FAVORITES_SHOWN = 12;

const CARD = 'bg-surface flex w-32 flex-col gap-2 rounded-xl p-2 text-left text-sm';

/** Favorites, Downloaded and Local files: their colour and icon, as large as a cover. */
const FIXED_ART: Record<string, { colour: string; icon: ReactNode }> = {
  favorites: {
    colour: 'bg-primary/15 text-primary',
    icon: <Heart aria-hidden size={32} fill="currentColor" />,
  },
  downloaded: {
    colour: 'bg-sky-500/20 text-sky-500',
    icon: <ArrowDown aria-hidden size={32} strokeWidth={3} />,
  },
  localFiles: {
    colour: 'bg-emerald-500/20 text-emerald-500',
    icon: <Folder aria-hidden size={30} fill="currentColor" />,
  },
};

/**
 * One quick access item as a card: its cover (the fixed blocks' colours, a playlist's mosaic, a
 * site's icon), its name and what it is, and ⋮ with the same menu as in quick access. A site
 * opens in the built-in browser; everything else is a link to its page.
 */
const QuickAccessCard: FC<{
  item: QuickAccessItem;
  name: string;
  playlist?: PlaylistSummary;
  website?: Website;
  menu: () => MenuItem[];
  onOpenSite: (id: string) => void;
}> = ({ item, name, playlist, website, menu, onOpenSite }) => {
  const { t, i18n } = useTranslation();
  // What it is (Album, Playlist…); the fixed blocks say it with their name.
  const kindKey = `home.played.${item.kind}`;
  const kind = i18n.exists(kindKey) ? t(kindKey) : '';
  const detail = [kind, item.subtitle].filter(Boolean).join(' · ');
  const label = [name, kind, item.pinned ? t('quickAccess.pinned') : '']
    .filter(Boolean)
    .join(' · ');
  const fixed = FIXED_ART[item.kind];
  const content = (
    <>
      {fixed ? (
        <span className={`grid aspect-square w-full place-items-center rounded-lg ${fixed.colour}`}>
          {fixed.icon}
        </span>
      ) : (
        // As quick access draws it (a round artist, a playlist's mosaic), as large as a cover.
        <PinArt pin={item} playlist={playlist} website={website} className="aspect-square w-full" />
      )}
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="flex min-w-0 items-center gap-1">
          <span className="truncate font-medium">{name}</span>
          {item.pinned && <Pin aria-hidden size={12} className="text-primary shrink-0 rotate-45" />}
        </span>
        {detail && <span className="text-muted truncate text-xs">{detail}</span>}
      </span>
    </>
  );
  return (
    <li data-testid="quick-access-card" className="relative shrink-0">
      <CardMenu label={t('menu.options', { name })} items={menu} className={CARD_MENU_BESIDE} />
      {item.kind === 'website' && item.search.site ? (
        <button
          type="button"
          aria-label={label}
          onClick={() => onOpenSite(item.search.site ?? '')}
          className={CARD}
        >
          {content}
        </button>
      ) : (
        <Link to={item.to} search={item.search as never} aria-label={label} className={CARD}>
          {content}
        </Link>
      )}
    </li>
  );
};

/** The latest favorite songs, newest first (hidden artists' stay out). */
const useLatestFavorites = () => {
  const favorites = useFavorites();
  const hidden = useHidden();
  const all = favorites.data ?? [];
  const latest = [...all]
    .filter((track) => !hidden.hidesCredit(track.artist))
    .sort((left, right) => (right.addedAt ?? 0) - (left.addedAt ?? 0))
    .slice(0, FAVORITES_SHOWN);
  return { all, latest };
};

/** The latest favorite songs as a row under quick access; each plays Favorites from it. */
const LatestFavorites: FC<{ favorites: ReturnType<typeof useLatestFavorites> }> = ({
  favorites: { all, latest },
}) => {
  const { t } = useTranslation();
  if (latest.length === 0) return null;
  return (
    <>
      <div className="mt-2 flex items-center gap-3">
        <h3 className="text-muted text-sm font-semibold">{t('home.latestFavorites')}</h3>
        <Link
          to="/library"
          search={{ view: 'favorites' }}
          aria-label={t('home.showAllOf', { name: t('home.latestFavorites') })}
          className="text-muted hover:text-foreground ml-auto px-1 text-sm font-semibold"
        >
          {t('discover.seeAll')}
        </Link>
      </div>
      <HomeCarousel>
        {latest.map((track) => (
          <li key={track.trackKey} className="relative w-28 shrink-0">
            {/* The same menu as anywhere a song is listed (M46.40). */}
            <TrackMoreButton
              track={fromSavedTrack(track)}
              extras={{ favorite: true }}
              className={CARD_MENU_BESIDE_TIGHT}
              iconSize={16}
            />
            <button
              type="button"
              onClick={() =>
                void playCollection({
                  collection: { kind: 'favorites' },
                  startIndex: all.indexOf(track),
                  shuffle: false,
                })
              }
              className="hover:bg-surface flex w-full flex-col gap-1.5 rounded-xl p-1.5 text-left"
            >
              <Artwork url={track.artworkUrl} className="w-full rounded-lg" iconSize={28} />
              <span className="truncate text-sm font-medium">{track.title}</span>
              <span className="text-muted -mt-0.5 truncate text-xs">{track.artist}</span>
            </button>
          </li>
        ))}
      </HomeCarousel>
    </>
  );
};

/**
 * Home's Quick access row (was Your music): quick access's own list as cards, the same items in
 * the same order as the desktop sidebar and the phone's drawer (`useQuickAccessItems`: pins
 * first, then the audio library, the most recently played from first by default), each with its
 * quick access menu; then the latest favorite songs.
 */
export const QuickAccessSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const quick = useQuickAccessItems();
  const openSite = useOpenWebsite();
  const favorites = useLatestFavorites();
  const items = [...quick.top, ...quick.rest].slice(0, HOME_SHOWN);
  // Every kind switched off in quick access, nothing pinned and no favorites: no row.
  if (!quick.quick.isPending && items.length === 0 && favorites.latest.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('quickAccess.title')}
      all={{ to: '/library' }}
      testId="quick-access-row"
    >
      <div className="flex flex-col gap-2">
        {quick.quick.isPending ? (
          <RowSkeleton card="w-32" />
        ) : (
          items.length > 0 && (
            <HomeCarousel>
              {items.map((item) => (
                <QuickAccessCard
                  key={pageKey(item)}
                  item={item}
                  name={quick.nameOf(item)}
                  playlist={quick.playlistOf(item)}
                  website={quick.siteOf(item)}
                  menu={() => quick.menuItems(item)}
                  onOpenSite={(site) => openSite.mutate(site)}
                />
              ))}
            </HomeCarousel>
          )
        )}
        <LatestFavorites favorites={favorites} />
      </div>
    </HomeSection>
  );
};
