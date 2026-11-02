import type { AudioFileFormat, ChapterFormat, SaveLocation } from '@raygun/bindings';
import { FolderOpen } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { withSave } from '../export/saveDefaults';
import type { Save } from '../export/saveDefaults';
import { useFolderState, usePickFolder } from '../export/useExports';
import { PrefChoice, PrefGroup, PrefSwitch } from './prefs';
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
  const save = withSave(settings.data?.save);
  const folder = useFolderState(save.location === 'folder' ? save.folderUri : null);
  if (!settings.data) return null;
  const busy = update.isPending || pick.isPending;
  const change = (patch: Partial<Save>) =>
    update.mutate({ key: 'save', value: { ...save, ...patch } });
  const lost = save.location === 'folder' && folder.data?.granted === false;

  return (
    <>
      <PrefGroup title={t('settings.saveGroup')}>
        <PrefChoice<SaveLocation>
          title={t('settings.saveLocation')}
          summary={t('settings.saveLocationHint')}
          value={save.location}
          disabled={busy}
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
        <li>
          <button
            type="button"
            disabled={busy}
            onClick={() => pick.mutate()}
            className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-1 py-3 text-left disabled:opacity-60"
          >
            <FolderOpen aria-hidden size={22} className="text-muted shrink-0" />
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="font-medium">
                {save.folderUri ? t('settings.changeFolder') : t('settings.pickFolder')}
              </span>
              <span className="text-muted text-sm">
                {save.folderName ?? t('settings.pickFolderHint')}
              </span>
            </span>
          </button>
        </li>
        {lost && (
          <li role="alert" className="text-primary px-1 py-2 text-sm">
            {t('settings.folderLost')}
          </li>
        )}
        {pick.isError && (
          <li role="alert" className="text-primary px-1 py-2 text-sm">
            {pick.error.message}
          </li>
        )}
        <PrefChoice<ChapterFormat>
          title={t('settings.chapterFormat')}
          summary={t('settings.chapterFormatHint')}
          value={save.chapterFormat}
          disabled={busy}
          options={FORMATS.map((value) => ({
            value,
            label: t(`settings.chapterFormats.${value}`),
          }))}
          onChange={(chapterFormat) => change({ chapterFormat })}
        />
      </PrefGroup>
      <PrefGroup title={t('songs.tab')}>
        <PrefChoice<AudioFileFormat>
          title={t('songs.songFormat')}
          summary={t('songs.songFormatHint')}
          value={save.songFormat}
          disabled={busy}
          options={(['original', 'mp3', 'wav', 'flac'] as const).map((value) => ({
            value,
            label: t(`songs.formats.${value}`),
          }))}
          onChange={(songFormat) => change({ songFormat })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.savedPages')}>
        <PrefSwitch
          title={t('settings.pagesPerTitle')}
          summary={t('settings.pagesPerTitleHint')}
          checked={save.pagesPerTitle}
          disabled={busy}
          onChange={(pagesPerTitle) => change({ pagesPerTitle })}
        />
      </PrefGroup>
    </>
  );
};
