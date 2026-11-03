import { commands } from '@raygun/bindings';
import type { ImageDestination, ImageFileFormat, Settings } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Download, FolderOpen } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { useCoverSheetStore } from './coverSheetStore';
import type { CoverToSave } from './coverSheetStore';

const FORMATS: ImageFileFormat[] = ['jpeg', 'png', 'webp'];

const chip = (on: boolean) =>
  `rounded-full px-4 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`;

/**
 * A cover large, and Save image (M46.26): JPEG, PNG or WebP, into Pictures/Raygun or a folder you
 * pick. Rust asks the source for its largest version.
 */
export const CoverSheet: FC = () => {
  const cover = useCoverSheetStore((store) => store.cover);
  const close = useCoverSheetStore((store) => store.close);
  if (!cover) return null;
  // A new cover starts fresh.
  return <CoverSheetBody key={cover.url} cover={cover} onClose={close} />;
};

const CoverSheetBody: FC<{ cover: CoverToSave; onClose: () => void }> = ({ cover, onClose }) => {
  const { t } = useTranslation();
  const settings = useSettings();
  const client = useQueryClient();
  const [format, setFormat] = useState<ImageFileFormat>('jpeg');
  const [destination, setDestination] = useState<ImageDestination>('pictures');
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  const folder = settings.data?.save.folderName ?? null;
  // Remembers the folder without changing where finished downloads go.
  const pick = useMutation({
    mutationFn: () => unwrapResult(commands.songsPickFolder()),
    onSuccess: (picked: Settings | null) => {
      if (!picked) return;
      client.setQueryData(['settings'], picked);
      setDestination('folder');
    },
  });
  const save = useMutation({
    mutationFn: () => unwrapResult(commands.coverSave(cover.url, cover.title, format, destination)),
  });

  return (
    <Sheet title={t('cover.title')} onClose={onClose}>
      <div data-testid="cover-sheet" className="flex flex-col gap-4">
        <img
          src={cover.url}
          alt={cover.title}
          onLoad={(event) =>
            setSize({
              width: event.currentTarget.naturalWidth,
              height: event.currentTarget.naturalHeight,
            })
          }
          className="mx-auto max-h-[50vh] w-full rounded-xl object-contain shadow-lg"
        />
        <p className="text-muted text-center text-xs">
          {size ? t('cover.size', { width: size.width, height: size.height }) : cover.title}
          {size && <> · {t('cover.largest')}</>}
        </p>
        <fieldset className="flex flex-col gap-2">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('cover.format')}
          </legend>
          <div className="flex gap-2">
            {FORMATS.map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={format === value}
                onClick={() => setFormat(value)}
                className={chip(format === value)}
              >
                {t(`cover.formats.${value}`)}
              </button>
            ))}
          </div>
        </fieldset>
        <fieldset className="flex flex-col gap-2">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('cover.where')}
          </legend>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              aria-pressed={destination === 'pictures'}
              onClick={() => setDestination('pictures')}
              className={chip(destination === 'pictures')}
            >
              {t('cover.pictures')}
            </button>
            {folder && (
              <button
                type="button"
                aria-pressed={destination === 'folder'}
                onClick={() => setDestination('folder')}
                className={chip(destination === 'folder')}
              >
                {folder}
              </button>
            )}
            <button
              type="button"
              disabled={pick.isPending}
              onClick={() => pick.mutate()}
              className="text-muted hover:text-foreground flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
            >
              <FolderOpen aria-hidden size={16} />
              {t(folder ? 'cover.otherFolder' : 'cover.chooseFolder')}
            </button>
          </div>
        </fieldset>
        {save.isSuccess && (
          <p role="status" className="text-primary text-sm wrap-anywhere">
            {t('cover.saved', { place: save.data })}
          </p>
        )}
        {(save.error ?? pick.error) && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {(save.error ?? pick.error)?.message}
          </p>
        )}
        <button
          type="button"
          disabled={save.isPending}
          onClick={() => save.mutate()}
          className="bg-primary text-background flex items-center justify-center gap-2 self-end rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
        >
          <Download aria-hidden size={16} />
          {save.isPending ? t('cover.saving') : t('cover.save')}
        </button>
      </div>
    </Sheet>
  );
};
