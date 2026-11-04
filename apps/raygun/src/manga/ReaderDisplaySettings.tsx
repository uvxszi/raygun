import type { PagedScale } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSize, PrefSlider, PrefSwitch } from '../settings/prefs';
import { PAGED_SCALES, READER_SIZE, useReaderDisplay } from './readerDisplay';

/**
 * How manga pages show (M46.7): the paged modes' fit, the webtoon column, the page bar and how far
 * ahead pages load. In Settings → Reader and in the reader's Display sheet.
 */
export const ReaderDisplaySettings: FC<{ title?: string }> = ({ title }) => {
  const { t } = useTranslation();
  const { display, set, busy } = useReaderDisplay();
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
      <PrefSize
        title={t('reader.display.webtoonWidth')}
        unit="%"
        {...READER_SIZE}
        value={display.webtoonSize}
        onChange={(webtoonSize) => set({ webtoonSize })}
      />
      <PrefSize
        title={t('reader.display.barWidth')}
        unit="%"
        {...READER_SIZE}
        value={display.barSize}
        onChange={(barSize) => set({ barSize })}
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
