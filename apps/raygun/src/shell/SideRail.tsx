import { Link } from '@tanstack/react-router';
import { Heart } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { usePlaylists } from '../collections/useCollections';
import { useNavTabs } from './navTabs';
import { ResizeHandle } from './ResizeHandle';
import { usePanelSizes } from './usePanelSizes';

/** The rail's default width, and its limits (72 shows icons and covers only, M46.8). */
const RAIL_DEFAULT = 224;
const RAIL_MIN = 72;
const RAIL_MAX = 360;
const COVERS_ONLY_BELOW = 140;

/**
 * The desktop's navigation: every tab the user shows (M46.5), in their order, then Favorites and
 * the playlists, as Spotify's library (M46.8). Its edge drags to resize it; narrow, it shows icons
 * and covers only.
 */
export const SideRail: FC = () => {
  const { t } = useTranslation();
  const { bar, active } = useNavTabs();
  const playlists = usePlaylists();
  const { panels, save } = usePanelSizes();
  const [dragging, setDragging] = useState<number | null>(null);
  const width = dragging ?? (panels.railWidth || RAIL_DEFAULT);
  const narrow = width < COVERS_ONLY_BELOW;
  const item = (current: boolean) =>
    `flex items-center rounded-xl font-medium ${narrow ? 'justify-center p-2.5' : 'gap-3 px-4 py-2.5'} ${
      current
        ? 'text-primary bg-primary/15'
        : 'text-muted hover:bg-background hover:text-foreground'
    }`;

  return (
    <div className="hidden shrink-0 lg:flex">
      <nav
        aria-label={t('nav.main')}
        data-testid="side-rail"
        style={{ width }}
        className="bg-surface flex min-h-0 flex-col gap-1 overflow-hidden p-3"
      >
        <span
          className={`text-primary flex items-center gap-2 pt-2 pb-4 text-2xl font-black tracking-tight ${narrow ? 'justify-center' : 'px-2'}`}
        >
          <img src="/icon.svg" alt="" className="size-9 shrink-0" />
          {!narrow && 'Raygun'}
        </span>
        {bar.map(({ id, to, search, labelKey, icon: Icon }) => (
          <Link
            key={id}
            to={to}
            search={search}
            data-testid={`nav-${id}-rail`}
            aria-label={narrow ? t(labelKey) : undefined}
            aria-current={active === id ? 'page' : undefined}
            // The bar decides which tab is current; the link only counts an exact match.
            activeOptions={{ exact: true, includeSearch: true }}
            className={item(active === id)}
          >
            <Icon aria-hidden size={20} />
            {!narrow && <span>{t(labelKey)}</span>}
          </Link>
        ))}
        <section
          aria-label={t('rail.library')}
          className="border-foreground/10 mt-3 flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto border-t pt-3"
        >
          {!narrow && (
            <h2 className="text-muted px-4 pb-1 text-xs font-semibold tracking-wide uppercase">
              {t('rail.library')}
            </h2>
          )}
          <Link
            to="/library"
            search={{ view: 'favorites' } as never}
            aria-label={narrow ? t('collections.favorites') : undefined}
            className={`flex items-center gap-3 rounded-xl ${narrow ? 'justify-center p-1.5' : 'px-2 py-1.5'} text-muted hover:bg-background hover:text-foreground`}
          >
            <span className="from-primary/80 to-primary/30 grid size-10 shrink-0 place-items-center rounded-md bg-gradient-to-br">
              <Heart aria-hidden size={16} className="text-background" fill="currentColor" />
            </span>
            {!narrow && <span className="truncate text-sm">{t('collections.favorites')}</span>}
          </Link>
          {(playlists.data ?? []).map((playlist) => (
            <Link
              key={playlist.id}
              to="/library/playlist"
              search={{ id: playlist.id }}
              aria-label={narrow ? playlist.name : undefined}
              className={`flex items-center gap-3 rounded-xl ${narrow ? 'justify-center p-1.5' : 'px-2 py-1.5'} text-muted hover:bg-background hover:text-foreground`}
            >
              <PlaylistCover
                artworkUrls={playlist.artworkUrls}
                className="size-10 shrink-0 rounded-md"
              />
              {!narrow && (
                <span className="flex min-w-0 flex-col">
                  <span className="text-foreground truncate text-sm">{playlist.name}</span>
                  <span className="truncate text-xs">
                    {t('collections.songs', { count: playlist.itemCount })}
                  </span>
                </span>
              )}
            </Link>
          ))}
        </section>
      </nav>
      <ResizeHandle
        label={t('rail.resize')}
        value={width}
        min={RAIL_MIN}
        max={RAIL_MAX}
        direction={1}
        onPreview={setDragging}
        onCommit={(railWidth) => {
          setDragging(null);
          save({ railWidth });
        }}
        className="-ml-1"
      />
    </div>
  );
};
