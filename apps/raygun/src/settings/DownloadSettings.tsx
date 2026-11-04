import type { AudioFileFormat, ChapterFormat, SaveLocation } from '@raygun/bindings';
import { FolderOpen } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { withSave } from '../export/saveDefaults';
import type { Save } from '../export/saveDefaults';
import { useFolderState, usePickFolder } from '../export/useExports';
import { useIsPhone } from '../shell/useIsDesktop';
import { PrefAction, PrefChoice, PrefGroup, PrefSwitch } from './prefs';
import { Problem } from './Problem';
import { useSettings, useUpdateSetting } from './useSettings';

const LOCATIONS: SaveLocation[] = ['app', 'downloads', 'folder'];
const FORMATS: ChapterFormat[] = ['cbz', 'images'];

/**
 * Settings → Downloads (M46.1): where finished downloads are also saved (the app keeps its own
 * copy for reading), as what, and where saved pages go.
 */
export const DownloadSettings: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const pick = usePickFolder();
  // Only Android reports battery saver (`device_state.rs`).
  const phone = useIsPhone();
  const save = withSave(settings.data?.save);
  const folder = useFolderState(save.location === 'folder' ? save.folderUri : null);
  if (!settings.data) return null;
  const change = (patch: Partial<Save>) =>
    update.mutate({ key: 'save', value: { ...save, ...patch } });
  const lost = save.location === 'folder' && folder.data?.granted === false;

  return (
    <PrefGroup>
      <PrefChoice<SaveLocation>
        title={t('settings.saveLocation')}
        value={save.location}
        options={LOCATIONS.map((value) => ({
          value,
          label:
            value === 'folder' && save.folderName
              ? save.folderName
              : t(`settings.saveLocations.${value}`),
        }))}
        onChange={(location) => {
          // A folder has to be picked before it can be used.
          if (location === 'folder' && !save.folderUri) pick.mutate();
          else change({ location });
        }}
      />
      {save.location === 'folder' && (
        <PrefAction
          icon={FolderOpen}
          title={save.folderUri ? t('settings.changeFolder') : t('settings.pickFolder')}
          summary={save.folderName}
          disabled={pick.isPending}
          onClick={() => pick.mutate()}
        />
      )}
      {lost && (
        <li role="alert" className="text-primary px-1 py-2 text-sm">
          {t('settings.folderLost')}
        </li>
      )}
      {pick.isError && (
        <li className="px-1 py-2">
          <Problem title={t('settings.pickFolderFailed')} detail={pick.error.message} />
        </li>
      )}
      <PrefChoice<ChapterFormat>
        title={t('settings.chapterFormat')}
        value={save.chapterFormat}
        options={FORMATS.map((value) => ({
          value,
          label: t(`settings.chapterFormats.${value}`),
        }))}
        onChange={(chapterFormat) => change({ chapterFormat })}
      />
      <PrefChoice<AudioFileFormat>
        title={t('songs.songFormat')}
        value={save.songFormat}
        options={(['original', 'mp3', 'wav', 'flac'] as const).map((value) => ({
          value,
          label: t(`songs.formats.${value}`),
        }))}
        onChange={(songFormat) => change({ songFormat })}
      />
      <PrefSwitch
        title={t('settings.pagesPerTitle')}
        checked={save.pagesPerTitle}
        onChange={(pagesPerTitle) => change({ pagesPerTitle })}
      />
      {phone && (
        <PrefSwitch
          title={t('settings.downloadsInBatterySaver')}
          checked={settings.data.downloadsInBatterySaver ?? false}
          onChange={(value) => update.mutate({ key: 'downloadsInBatterySaver', value })}
        />
      )}
    </PrefGroup>
  );
};
