import type { PagedScale } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSlider, PrefSwitch } from '../settings/prefs';
import { BAR_WIDTHS, PAGED_SCALES, WEBTOON_WIDTHS, useReaderDisplay } from './readerDisplay';

/**
 * How manga pages show (M46.7): the paged modes' fit, the webtoon column, the page bar and how far
 * ahead pages load. In Settings → Reader and in the reader's Display sheet.
 */
export const ReaderDisplaySettings: FC<{ title?: string }> = ({ title }) => {
  const { t } = useTranslation();
  const { display, set, busy } = useReaderDisplay();
  const percent = (value: number) =>
    value === 0 ? t('reader.display.byScreen') : t('reader.display.percent', { n: value });
  return (
    <PrefGroup title={title ?? t('reader.display.title')}>
      <PrefChoice<PagedScale>
        title={t('reader.display.pagedScale')}
        summary={t('reader.display.pagedScaleHint')}
        value={display.pagedScale}
        disabled={busy}
        options={PAGED_SCALES.map((value) => ({
          value,
          label: t(`reader.display.scale.${value}`),
        }))}
        onChange={(pagedScale) => set({ pagedScale })}
      />
      <PrefChoice<number>
        title={t('reader.display.webtoonWidth')}
        summary={t('reader.display.webtoonWidthHint')}
        value={display.webtoonWidth}
        disabled={busy}
        options={WEBTOON_WIDTHS.map((value) => ({ value, label: percent(value) }))}
        onChange={(webtoonWidth) => set({ webtoonWidth })}
      />
      <PrefChoice<number>
        title={t('reader.display.barWidth')}
        value={display.barWidth}
        disabled={busy}
        options={BAR_WIDTHS.map((value) => ({ value, label: percent(value) }))}
        onChange={(barWidth) => set({ barWidth })}
      />
      <PrefSwitch
        title={t('reader.display.barChapters')}
        checked={display.barChapters}
        disabled={busy}
        onChange={(barChapters) => set({ barChapters })}
      />
      <PrefSwitch
        title={t('reader.display.barZoom')}
        checked={display.barZoom}
        disabled={busy}
        onChange={(barZoom) => set({ barZoom })}
      />
      <PrefSwitch
        title={t('reader.display.barMode')}
        checked={display.barMode}
        disabled={busy}
        onChange={(barMode) => set({ barMode })}
      />
      <PrefSwitch
        title={t('reader.display.barFullscreen')}
        checked={display.barFullscreen}
        disabled={busy}
        onChange={(barFullscreen) => set({ barFullscreen })}
      />
      <PrefSlider
        title={t('reader.display.preload')}
        summary={t('reader.display.preloadHint')}
        value={display.preloadPages}
        min={1}
        max={10}
        step={1}
        format={(value) => t('reader.display.pages', { count: value })}
        onChange={(preloadPages) => set({ preloadPages })}
        disabled={busy}
      />
    </PrefGroup>
  );
};
