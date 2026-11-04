import { commands } from '@raygun/bindings';
import type { ImageDestination, ImageFileFormat, Settings } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Download, FolderOpen, Share2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { useCoverSheetStore } from './coverSheetStore';
import type { CoverToSave } from './coverSheetStore';
import { ZoomableImage } from './ZoomableImage';

const FORMATS: ImageFileFormat[] = ['jpeg', 'png', 'webp', 'jxl'];

const chip = (on: boolean) =>
  `rounded-full px-4 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`;

/**
 * A cover, poster or backdrop close up (M46.26): the largest version its host has, to zoom and
 * pan, save as JPEG, PNG, WebP or JPEG XL into Pictures/Raygun or a picked folder, share or copy.
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
  const android = usePlatform() === 'android';
  const [format, setFormat] = useState<ImageFileFormat>('jpeg');
  const [destination, setDestination] = useState<ImageDestination>('pictures');
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  // Larger versions first; one that fails to load gives way to the next, the cover itself last.
  const versions = useQuery({
    queryKey: ['coverVersions', cover.url],
    queryFn: () => commands.coverVersions(cover.url),
    staleTime: Infinity,
  });
  const [failed, setFailed] = useState(0);
  const candidates = versions.data?.length ? versions.data : versions.isPending ? [] : [cover.url];
  const shown = candidates[Math.min(failed, candidates.length - 1)];
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
  const hand = useMutation({
    mutationFn: (action: 'share' | 'copy') =>
      unwrapResult(
        action === 'share'
          ? commands.coverShare(cover.url, cover.title)
          : commands.coverCopy(cover.url, cover.title),
      ),
  });
  const error = save.error ?? pick.error ?? hand.error;

  return (
    <Sheet title={t('cover.title')} onClose={onClose}>
      <div data-testid="cover-sheet" className="flex flex-col gap-4">
        {shown ? (
          <ZoomableImage
            key={shown}
            src={shown}
            alt={cover.title}
            onLoad={setSize}
            onError={() => {
              if (failed < candidates.length - 1) setFailed(failed + 1);
            }}
          />
        ) : (
          <div aria-hidden className="bg-background h-[50vh] animate-pulse rounded-xl" />
        )}
        <p className="text-muted text-center text-xs">
          {size ? t('cover.size', { width: size.width, height: size.height }) : cover.title}
          {size && <> · {t('cover.largest')}</>}
        </p>
        <fieldset className="flex flex-col gap-2">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('cover.format')}
          </legend>
          <div className="flex flex-wrap gap-2">
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
        {hand.isSuccess && hand.variables === 'copy' && (
          <p role="status" className="text-primary text-sm">
            {t('cover.copied')}
          </p>
        )}
        {error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {error.message}
          </p>
        )}
        <div className="flex flex-wrap items-center justify-end gap-2">
          {android && (
            <button
              type="button"
              disabled={hand.isPending}
              onClick={() => hand.mutate('share')}
              className="hover:bg-background flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
            >
              <Share2 aria-hidden size={16} />
              {t('cover.share')}
            </button>
          )}
          <button
            type="button"
            disabled={hand.isPending}
            onClick={() => hand.mutate('copy')}
            className="hover:bg-background flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60"
          >
            <Copy aria-hidden size={16} />
            {t('cover.copy')}
          </button>
          <button
            type="button"
            disabled={save.isPending}
            onClick={() => save.mutate()}
            className="bg-primary text-background flex items-center justify-center gap-2 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
          >
            <Download aria-hidden size={16} />
            {save.isPending ? t('cover.saving') : t('cover.save')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
