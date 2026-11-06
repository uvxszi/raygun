import type { NovelReaderPrefs, NovelTheme, NovelWidth } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ReadingFontRow } from '../../fonts/ReadingFont';
import { ReaderDisplaySettings } from '../../manga/ReaderDisplaySettings';
import { DEFAULT_PREFS, INDENT, LINE_SPACING, TEXT_SIZE } from '../../novel/useNovel';
import { PrefChoice, PrefGroup, PrefSize, PrefSlider, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Settings → Reader: how comics' pages and books' text show, and reading aloud. */
export const ReaderSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  if (!data) return null;
  const prefs = data.novelReader;
  const set = (patch: Partial<NovelReaderPrefs>) =>
    update.mutate({ key: 'novelReader', value: { ...prefs, ...patch } });
  return (
    <>
      <ReaderDisplaySettings title={t('reader.display.mangaTitle')} />
      <PrefGroup title={t('settings.novelReader')}>
        <PrefSize
          title={t('settings.fontSize')}
          unit="px"
          {...TEXT_SIZE}
          value={prefs.textSize ?? 0}
          onChange={(textSize) => set({ textSize })}
        />
        <PrefSize
          title={t('settings.lineSpacing')}
          unit=""
          {...LINE_SPACING}
          zero={DEFAULT_PREFS.lineSpacing}
          factor={0.1}
          value={prefs.lineSpacing}
          onChange={(lineSpacing) => set({ lineSpacing })}
        />
        <ReadingFontRow />
        <PrefChoice<NovelWidth>
          title={t('settings.pageWidth')}
          value={prefs.width}
          options={[
            { value: 'narrow', label: t('settings.widthNarrow') },
            { value: 'wide', label: t('settings.widthWide') },
          ]}
          onChange={(width) => set({ width })}
        />
        <PrefChoice<NovelTheme>
          title={t('novel.theme')}
          value={prefs.theme ?? 'app'}
          options={(['app', 'light', 'sepia', 'dark', 'black'] as const).map((value) => ({
            value,
            label: t(`novel.themes.${value}`),
          }))}
          onChange={(theme) => set({ theme })}
        />
        <PrefSize
          title={t('settings.indent')}
          unit="em"
          {...INDENT}
          factor={0.1}
          value={prefs.indent ?? 0}
          onChange={(indent) => set({ indent })}
        />
        <PrefSwitch
          title={t('settings.justify')}
          checked={prefs.justify ?? false}
          onChange={(justify) => set({ justify })}
        />
        <PrefSwitch
          title={t('settings.paged')}
          checked={prefs.paged}
          onChange={(paged) => set({ paged })}
        />
        <PrefSwitch
          title={t('reader.display.keepScreenOn')}
          checked={prefs.keepScreenOn ?? true}
          onChange={(keepScreenOn) => set({ keepScreenOn })}
        />
        <PrefSlider
          title={t('settings.speechRate')}
          value={prefs.ttsRate ?? 10}
          min={5}
          max={20}
          step={1}
          format={(value) => `${(value / 10).toFixed(1)}×`}
          onChange={(ttsRate) => set({ ttsRate })}
        />
      </PrefGroup>
    </>
  );
};
