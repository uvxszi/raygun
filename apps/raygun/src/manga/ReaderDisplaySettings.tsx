import type { PagedScale, ReaderMode, TapInvert, TapZones } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSize, PrefSlider, PrefSwitch } from '../settings/prefs';
import { TAP_INVERTS, TAP_ZONES } from './reader/tapZones';
import { PAGED_SCALES, READER_MODES, READER_SIZE, useReaderDisplay } from './readerDisplay';

/**
 * How manga read and show (M46.7): the default mode, tap zones, the paged modes' fit, the webtoon
 * column, the page bar and how far ahead pages load. In Settings → Reader and in the reader's
 * Display sheet.
 */
export const ReaderDisplaySettings: FC<{ title?: string }> = ({ title }) => {
  const { t } = useTranslation();
  const { display, set, busy } = useReaderDisplay();
  return (
    <>
      <PrefGroup title={title ?? t('reader.display.reading')}>
        <PrefChoice<ReaderMode>
          title={t('reader.display.defaultMode')}
          value={display.defaultMode}
          disabled={busy}
          options={READER_MODES.map((value) => ({ value, label: t(`manga.mode.${value}`) }))}
          onChange={(defaultMode) => set({ defaultMode })}
        />
        <PrefChoice<TapZones>
          title={t('reader.display.tapZones')}
          value={display.tapZones}
          disabled={busy}
          options={TAP_ZONES.map((value) => ({
            value,
            label: t(`reader.display.tapZone.${value}`),
          }))}
          onChange={(tapZones) => set({ tapZones })}
        />
        {display.tapZones !== 'off' && (
          <PrefChoice<TapInvert>
            title={t('reader.display.invertTaps')}
            value={display.invertTaps}
            disabled={busy}
            options={TAP_INVERTS.map((value) => ({
              value,
              label: t(`reader.display.invert.${value}`),
            }))}
            onChange={(invertTaps) => set({ invertTaps })}
          />
        )}
        <PrefSwitch
          title={t('reader.display.doubleTapZoom')}
          checked={display.doubleTapZoom}
          disabled={busy}
          onChange={(doubleTapZoom) => set({ doubleTapZoom })}
        />
        <PrefSwitch
          title={t('reader.display.skipDuplicates')}
          checked={display.skipDuplicates}
          disabled={busy}
          onChange={(skipDuplicates) => set({ skipDuplicates })}
        />
        <PrefSwitch
          title={t('reader.display.keepScreenOn')}
          checked={display.keepScreenOn}
          disabled={busy}
          onChange={(keepScreenOn) => set({ keepScreenOn })}
        />
        <PrefSlider
          title={t('reader.display.preload')}
          value={display.preloadPages}
          min={1}
          max={10}
          step={1}
          format={(value) => t('reader.display.pages', { count: value })}
          onChange={(preloadPages) => set({ preloadPages })}
          disabled={busy}
        />
      </PrefGroup>
      <PrefGroup title={t('reader.display.pagesTitle')}>
        <PrefChoice<PagedScale>
          title={t('reader.display.pagedScale')}
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
        <PrefSwitch
          title={t('reader.display.pageNumber')}
          checked={display.pageNumber}
          disabled={busy}
          onChange={(pageNumber) => set({ pageNumber })}
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
      </PrefGroup>
    </>
  );
};
