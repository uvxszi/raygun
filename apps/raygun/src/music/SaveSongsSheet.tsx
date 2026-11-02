import type { AudioFileFormat, SongDestination } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { useSongDownloadActions } from './useSongDownloads';

const FORMATS: AudioFileFormat[] = ['original', 'wav', 'flac'];

/**
 * Saving downloaded songs as files (M46.10): as downloaded, WAV or FLAC, to Music/Raygun or the
 * folder picked in Settings → Downloads. Says where each file went.
 */
export const SaveSongsSheet: FC<{ ids: string[]; onClose: () => void }> = ({ ids, onClose }) => {
  const { t } = useTranslation();
  const settings = useSettings();
  const { save } = useSongDownloadActions();
  const folder = settings.data?.save.folderName ?? null;
  const [format, setFormat] = useState<AudioFileFormat>(
    settings.data?.save.songFormat ?? 'original',
  );
  const [destination, setDestination] = useState<SongDestination>('music');
  const radio = (checked: boolean) =>
    `flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm ${checked ? 'bg-primary/15' : 'bg-background'}`;

  return (
    <Sheet title={t('songs.saveTitle', { count: ids.length })} onClose={onClose}>
      <div className="flex flex-col gap-4">
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('songs.format')}
          </legend>
          {FORMATS.map((value) => (
            <label key={value} className={radio(format === value)}>
              <input
                type="radio"
                name="song-format"
                checked={format === value}
                onChange={() => setFormat(value)}
                className="accent-primary"
              />
              <span className="flex flex-col">
                <span>{t(`songs.formats.${value}`)}</span>
                <span className="text-muted text-xs">{t(`songs.formatHints.${value}`)}</span>
              </span>
            </label>
          ))}
        </fieldset>
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('songs.where')}
          </legend>
          <label className={radio(destination === 'music')}>
            <input
              type="radio"
              name="song-destination"
              checked={destination === 'music'}
              onChange={() => setDestination('music')}
              className="accent-primary"
            />
            {t('songs.toMusic')}
          </label>
          <label className={radio(destination === 'folder')}>
            <input
              type="radio"
              name="song-destination"
              checked={destination === 'folder'}
              disabled={!folder}
              onChange={() => setDestination('folder')}
              className="accent-primary"
            />
            {folder ? t('songs.toFolder', { name: folder }) : t('songs.noFolder')}
          </label>
        </fieldset>
        {save.data && (
          <div role="status" className="flex flex-col gap-1 text-sm">
            {save.data.saved.map((shown) => (
              <span key={shown} className="text-muted wrap-anywhere">
                {t('songs.savedTo', { place: shown })}
              </span>
            ))}
            {save.data.failed.map((line) => (
              <span key={line} className="text-primary wrap-anywhere">
                {line}
              </span>
            ))}
          </div>
        )}
        {save.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {save.error.message}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="bg-background rounded-full px-5 py-2 text-sm"
          >
            {t(save.data ? 'songs.done' : 'collections.cancel')}
          </button>
          {!save.data && (
            <button
              type="button"
              disabled={save.isPending}
              onClick={() => save.mutate({ ids, format, destination })}
              className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
            >
              {t(save.isPending ? 'songs.saving' : 'songs.save')}
            </button>
          )}
        </div>
      </div>
    </Sheet>
  );
};
