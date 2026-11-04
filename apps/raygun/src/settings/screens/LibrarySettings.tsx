import type { CoverStyle, LibraryDisplayMode, LibraryTabs } from '@raygun/bindings';
import { Tag } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryFolders } from '../../library/LibraryFolders';
import { useLibraryStatus } from '../../library/useLibrary';
import {
  COVER_SIZE,
  DISPLAY_MODES,
  TAB_MODES,
  withLibraryDisplay,
} from '../../manga/libraryDisplay';
import type { LibraryDisplay } from '../../manga/libraryDisplay';
import { useIsDesktop } from '../../shell/useIsDesktop';
import { useSideRail } from '../../shell/useWideScreen';
import { PrefBlock, PrefChoice, PrefGroup, PrefLink, PrefSize, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Matches `MANGA_UPDATE_HOURS` in raygun-model. */
const UPDATE_HOURS = [0, 6, 12, 24, 48, 168];
/** How many automatic quick access pins are kept (M46.41). */
const QUICK_ACCESS_LIMITS = [10, 20, 50, 100, 250, 500, 1000];

/** Settings → Library: how titles show, updates, categories, music folders and the sidebar. */
export const LibrarySettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const status = useLibraryStatus();
  // The sidebar's pins show on desktop and on windows wide enough for the side rail (M46.29).
  const desktop = useIsDesktop();
  const wide = useSideRail();
  const sidebar = desktop || wide;
  if (!data) return null;
  const display = withLibraryDisplay(data.libraryDisplay);
  const setDisplay = (change: Partial<LibraryDisplay>) =>
    update.mutate({ key: 'libraryDisplay', value: { ...display, ...change } });
  const rail = data.rail;

  return (
    <>
      <PrefGroup>
        <PrefChoice<LibraryDisplayMode>
          title={t('library.display.mode')}
          value={display.mode}
          options={DISPLAY_MODES.map((value) => ({
            value,
            label: t(`library.display.modes.${value}`),
          }))}
          onChange={(mode) => setDisplay({ mode })}
        />
        <PrefSize
          title={t('settings.coverWidth')}
          unit="%"
          {...COVER_SIZE}
          value={data.coverSize ?? 0}
          onChange={(value) => update.mutate({ key: 'coverSize', value })}
        />
        <PrefChoice<CoverStyle>
          title={t('settings.browseCovers')}
          value={data.coverStyle}
          options={[
            { value: 'compact', label: t('settings.coverOver') },
            { value: 'comfortable', label: t('settings.coverUnder') },
          ]}
          onChange={(value) => update.mutate({ key: 'coverStyle', value })}
        />
        <PrefChoice<LibraryTabs>
          title={t('library.display.tabs')}
          value={display.tabs}
          options={TAB_MODES.map((value) => ({
            value,
            label: t(`library.display.tabModes.${value}`),
          }))}
          onChange={(tabs) => setDisplay({ tabs })}
        />
        <PrefSwitch
          title={t('library.display.showCounts')}
          checked={display.showCounts}
          onChange={(showCounts) => setDisplay({ showCounts })}
        />
        <PrefSwitch
          title={t('library.display.perCategory')}
          checked={display.perCategory}
          onChange={(perCategory) => setDisplay({ perCategory })}
        />
        <PrefSwitch
          title={t('library.display.pinsOnTop')}
          checked={display.pinsOnTop}
          onChange={(pinsOnTop) => setDisplay({ pinsOnTop })}
        />
        <PrefSwitch
          title={t('library.display.continueReading')}
          checked={display.continueButton}
          onChange={(continueButton) => setDisplay({ continueButton })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.badges')}>
        <PrefSwitch
          title={t('settings.unreadBadges')}
          checked={data.unreadBadges}
          onChange={(value) => update.mutate({ key: 'unreadBadges', value })}
        />
        <PrefSwitch
          title={t('settings.downloadBadges')}
          checked={data.downloadBadges}
          onChange={(value) => update.mutate({ key: 'downloadBadges', value })}
        />
        <PrefSwitch
          title={t('library.display.localBadge')}
          checked={display.localBadge}
          onChange={(localBadge) => setDisplay({ localBadge })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.libraryUpdates')}>
        <PrefChoice<number>
          title={t('settings.mangaUpdates')}
          value={data.mangaUpdateHours}
          options={UPDATE_HOURS.map((hours) => ({
            value: hours,
            label: t(`settings.updateEvery.${hours}`),
          }))}
          onChange={(value) => update.mutate({ key: 'mangaUpdateHours', value })}
        />
      </PrefGroup>
      <PrefGroup title={t('library.categories.title')}>
        <PrefLink
          icon={Tag}
          to="/library/categories"
          search={{ medium: 'manga' }}
          title={t('library.categories.mangaTitle')}
        />
        <PrefLink
          icon={Tag}
          to="/library/categories"
          search={{ medium: 'novel' }}
          title={t('library.categories.novelTitle')}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.musicFolders')}>
        <PrefBlock>
          <LibraryFolders roots={status.data?.roots ?? []} />
        </PrefBlock>
      </PrefGroup>
      {sidebar && (
        <PrefGroup title={t('settings.quickAccess.title')}>
          <PrefSwitch
            title={t('settings.quickAccess.autoAdd')}
            checked={rail.autoAdd ?? true}
            onChange={(autoAdd) => update.mutate({ key: 'rail', value: { ...rail, autoAdd } })}
          />
          <PrefChoice<number>
            title={t('settings.quickAccess.limit')}
            value={rail.limit ?? 20}
            disabled={rail.autoAdd === false}
            options={QUICK_ACCESS_LIMITS.map((value) => ({
              value,
              label: t('settings.quickAccess.pins', { count: value }),
            }))}
            onChange={(limit) => update.mutate({ key: 'rail', value: { ...rail, limit } })}
          />
        </PrefGroup>
      )}
    </>
  );
};
