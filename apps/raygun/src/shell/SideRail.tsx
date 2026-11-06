import { Link } from '@tanstack/react-router';
import { useLayoutEffect, useRef, useState } from 'react';
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
/** The tabs keep a row at least, and Your library this much, when the split between them moves. */
const TABS_MIN = 44;
const PINS_MIN = 120;
/** Highest the tabs are kept: `PanelPrefs::rail_nav_height`'s limit in Rust. */
const TABS_MAX = 2000;

/**
 * The desktop's navigation: every tab the user shows (M46.5), in their order, then quick access,
 * as Spotify's library (M46.8): pins on top, then the audio library by itself. Its edge drags to
 * resize it, and the line between the tabs and quick access moves up and down (a double click
 * puts it back); narrow, it shows icons and covers only.
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
  // The split between the tabs and Your library: the tabs' height, dragged (0: as tall as they
  // are). Shorter than its tabs, the part scrolls.
  const railRef = useRef<HTMLElement>(null);
  const tabsRef = useRef<HTMLDivElement>(null);
  const [tabsDrag, setTabsDrag] = useState<number | null>(null);
  const [space, setSpace] = useState({ natural: 0, room: 0 });
  useLayoutEffect(() => {
    const whole = railRef.current;
    const tabs = tabsRef.current;
    if (!whole || !tabs) return;
    const measure = () => {
      const natural = tabs.scrollHeight;
      const room = whole.clientHeight - tabs.offsetTop - PINS_MIN;
      setSpace((current) =>
        current.natural === natural && current.room === room ? current : { natural, room },
      );
    };
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(whole);
    return () => observer?.disconnect();
  }, [bar.length, narrow]);
  // Not laid out (or nothing measured): any height within Rust's limits.
  const tabsMax = space.room > 0 ? Math.min(TABS_MAX, Math.max(TABS_MIN, space.room)) : TABS_MAX;
  const storedTabs = panels.railNavHeight ?? 0;
  const tabsHeight =
    tabsDrag ?? (storedTabs > 0 ? Math.min(tabsMax, Math.max(TABS_MIN, storedTabs)) : null);
  const item = (current: boolean) =>
    `flex items-center rounded-xl font-medium ${narrow ? 'justify-center p-2.5' : 'gap-3 px-4 py-2.5'} ${
      current
        ? 'text-primary bg-primary/15'
        : 'text-muted hover:bg-background hover:text-foreground'
    }`;

  return (
    <div className={desktop ? 'flex shrink-0' : 'hidden shrink-0 lg:flex'}>
      <nav
        ref={railRef}
        aria-label={t('nav.main')}
        data-testid="side-rail"
        style={{ width }}
        className="bg-surface relative flex min-h-0 flex-col gap-1 overflow-hidden p-3"
      >
        <span
          className={`text-primary flex items-center gap-2 pt-2 pb-4 text-2xl font-black tracking-tight ${narrow ? 'justify-center' : 'px-2'}`}
        >
          <img src="/icon.svg" alt="" className="size-9 shrink-0" />
          {!narrow && 'Raygun'}
        </span>
        <div
          ref={tabsRef}
          data-testid="rail-tabs"
          style={tabsHeight === null ? undefined : { height: tabsHeight }}
          className={`flex shrink-0 flex-col gap-1 ${tabsHeight === null ? '' : 'overflow-y-auto overscroll-contain'}`}
        >
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
              <Icon aria-hidden size={20} className="shrink-0" />
              {!narrow && <span>{t(labelKey)}</span>}
            </Link>
          ))}
        </div>
        <ResizeHandle
          across
          label={t('rail.resizeTabs')}
          value={tabsHeight ?? Math.max(TABS_MIN, Math.min(tabsMax, space.natural || TABS_MIN))}
          min={TABS_MIN}
          max={tabsMax}
          direction={1}
          onPreview={setTabsDrag}
          onCommit={(railNavHeight) => {
            setTabsDrag(null);
            save({ railNavHeight });
          }}
          onReset={() => {
            setTabsDrag(null);
            save({ railNavHeight: 0 });
          }}
          className="my-0.5"
        />
        <section
          aria-label={t('rail.library')}
          className="border-foreground/10 flex min-h-0 flex-1 flex-col gap-1 border-t pt-3"
        >
          {/* Your library (quick access): pins on top, then the audio library by itself. */}
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
