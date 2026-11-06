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
import { PrefBlock, PrefChoice, PrefGroup, PrefLink, PrefSize, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Matches `MANGA_UPDATE_HOURS` in raygun-model. */
const UPDATE_HOURS = [0, 6, 12, 24, 48, 168];

/** Settings → Library: how titles show, updates, categories and music folders. */
export const LibrarySettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const status = useLibraryStatus();
  if (!data) return null;
  const display = withLibraryDisplay(data.libraryDisplay);
  const setDisplay = (change: Partial<LibraryDisplay>) =>
    update.mutate({ key: 'libraryDisplay', value: { ...display, ...change } });

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
            label: t(`library.display.tabModes.${value}`, { context: 'all' }),
          }))}
          onChange={(tabs) => setDisplay({ tabs })}
        />
        <PrefSwitch
          title={t('library.display.showCounts')}
          checked={display.showCounts}
          onChange={(showCounts) => setDisplay({ showCounts })}
        />
        <PrefSwitch
          title={t('library.display.perCategory', { context: 'all' })}
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
      <PrefGroup title={t('more.categories')}>
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
        <PrefLink
          icon={Tag}
          to="/library/categories"
          search={{ medium: 'video' }}
          title={t('library.categories.videoTitle')}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.musicFolders')}>
        <PrefBlock>
          <LibraryFolders roots={status.data?.roots ?? []} />
        </PrefBlock>
      </PrefGroup>
    </>
  );
};
