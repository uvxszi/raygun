import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useNavTabs } from './navTabs';
import { RailPins } from './RailPins';
import { ResizeHandle } from './ResizeHandle';
import { useIsDesktop } from './useIsDesktop';
import { usePanelSizes } from './usePanelSizes';
import { useSideRail } from './useWideScreen';

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
  const { panels, save } = usePanelSizes();
  const [dragging, setDragging] = useState<number | null>(null);
  // On desktop the rail stays at every window size (M46.9): below `lg` it shows covers only.
  const desktop = useIsDesktop();
  const roomy = useSideRail();
  const squeezed = desktop && !roomy && dragging === null;
  const width = squeezed ? RAIL_MIN : (dragging ?? (panels.railWidth || RAIL_DEFAULT));
  const narrow = width < COVERS_ONLY_BELOW;
  const item = (current: boolean) =>
    `flex items-center rounded-xl font-medium ${narrow ? 'justify-center p-2.5' : 'gap-3 px-4 py-2.5'} ${
      current
        ? 'text-primary bg-primary/15'
        : 'text-muted hover:bg-background hover:text-foreground'
    }`;

  return (
    <div className={desktop ? 'flex shrink-0' : 'hidden shrink-0 lg:flex'}>
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
          {/* Pins you arrange (M46.29): Favorites, Downloaded, Local files, playlists and more. */}
          <RailPins narrow={narrow} shown={desktop || roomy} />
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
