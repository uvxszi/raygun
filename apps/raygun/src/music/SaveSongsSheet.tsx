import { commands } from '@raygun/bindings';
import type { AudioFileFormat, Settings, SongDestination } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FolderOpen } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { useSongDownloadActions } from './useSongDownloads';

const FORMATS: AudioFileFormat[] = ['original', 'mp3', 'wav', 'flac'];

/**
 * Saving downloaded songs as files (M46.10): as downloaded, MP3, WAV or FLAC, to Music/Raygun or
 * a folder, which can be picked right here (M46.18; it also becomes Settings → Downloads'
 * folder). Says where each file went.
 */
export const SaveSongsSheet: FC<{ ids: string[]; onClose: () => void }> = ({ ids, onClose }) => {
  const { t } = useTranslation();
  const settings = useSettings();
  const { save } = useSongDownloadActions();
  const client = useQueryClient();
  // Remembers the folder without changing where finished downloads go; cancelling changes nothing.
  const pick = useMutation({
    mutationFn: () => unwrapResult(commands.songsPickFolder()),
    onSuccess: (settings: Settings | null) => {
      if (!settings) return;
      client.setQueryData(['settings'], settings);
      setDestination('folder');
    },
  });
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
          {folder && (
            <label className={radio(destination === 'folder')}>
              <input
                type="radio"
                name="song-destination"
                checked={destination === 'folder'}
                onChange={() => setDestination('folder')}
                className="accent-primary"
              />
              <span className="min-w-0 flex-1 truncate">{folder}</span>
            </label>
          )}
          <button
            type="button"
            disabled={pick.isPending}
            onClick={() => pick.mutate()}
            className="text-primary flex items-center gap-2 self-start rounded-full px-3 py-2 text-sm font-semibold disabled:opacity-60"
          >
            <FolderOpen aria-hidden size={16} />
            {folder ? t('songs.otherFolder') : t('songs.chooseFolder')}
          </button>
          {pick.isError && (
            <p role="alert" className="text-primary px-1 text-sm wrap-anywhere">
              {pick.error.message}
            </p>
          )}
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
