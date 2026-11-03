import { commands } from '@raygun/bindings';
import type { DetailEdit, DetailTarget } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ImagePlus, RotateCcw } from 'lucide-react';
import { useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { uploadFile } from '../manga/useImportComics';
import { detailKey, useDetailEdit } from './useDetailEdit';

type Field = 'title' | 'artist' | 'album';

const EMPTY: DetailEdit = { title: null, artist: null, album: null, artworkUrl: null };

/**
 * Edit details (M46.30): fields over what the source said (shown as each field's hint), a cover
 * from a file or an address, and Reset. Saving refreshes every list, since edits show everywhere.
 */
export const DetailEditForm: FC<{
  target: DetailTarget;
  fields: Field[];
  /** What the source says, shown as each field's hint and the cover before an edit. */
  original: Partial<Record<Field, string | null>> & { artworkUrl?: string | null };
  /** The label of the title field ("Song title", "Album title", "Name"). */
  titleLabel: string;
  onSaved?: () => void;
}> = ({ target, fields, original, titleLabel, onSaved }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const edit = useDetailEdit(target);
  const [draft, setDraft] = useState<DetailEdit | null>(null);
  const picker = useRef<HTMLInputElement>(null);
  const current = draft ?? edit.data ?? EMPTY;
  const save = useMutation({
    mutationFn: (next: DetailEdit) => unwrapResult(commands.detailEditSet(target, next)),
    onSuccess: (saved) => {
      client.setQueryData(detailKey(target), saved);
      setDraft(null);
      // Edits show in every list, the queue and the player.
      void client.invalidateQueries();
      onSaved?.();
    },
  });
  const cover = useMutation({
    mutationFn: async (file: File) =>
      unwrapResult(commands.detailCoverImport(await uploadFile(file))),
    onSuccess: (url) => setDraft({ ...current, artworkUrl: url }),
  });
  const set = (field: keyof DetailEdit, value: string) =>
    setDraft({ ...current, [field]: value.trim() === '' ? null : value });
  const labels: Record<Field, string> = {
    title: titleLabel,
    artist: t('details.artist'),
    album: t('details.album'),
  };

  if (edit.isPending) return <p className="text-muted text-sm">{t('songEdit.loading')}</p>;
  return (
    <form
      data-testid="detail-edit"
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        save.mutate(current);
      }}
    >
      {fields.map((field) => (
        <label key={field} className="flex flex-col gap-1 text-sm">
          <span className="text-muted text-xs font-semibold uppercase">{labels[field]}</span>
          <input
            value={current[field] ?? ''}
            placeholder={original[field] ?? ''}
            onChange={(event) => set(field, event.target.value)}
            className="bg-background rounded-xl px-3 py-2"
          />
        </label>
      ))}
      <div className="flex items-center gap-3">
        <Artwork
          url={current.artworkUrl ?? original.artworkUrl ?? null}
          className="size-20 shrink-0 rounded-lg"
          iconSize={28}
        />
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <span className="text-muted text-xs font-semibold uppercase">{t('details.cover')}</span>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={cover.isPending}
              onClick={() => picker.current?.click()}
              className="bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
            >
              <ImagePlus aria-hidden size={15} />
              {cover.isPending ? t('details.coverAdding') : t('details.coverPick')}
            </button>
            {current.artworkUrl && (
              <button
                type="button"
                onClick={() => setDraft({ ...current, artworkUrl: null })}
                className="text-muted hover:text-foreground rounded-full px-3 py-1.5 text-sm"
              >
                {t('details.coverOriginal')}
              </button>
            )}
          </div>
          <input
            aria-label={t('details.coverAddress')}
            placeholder={t('details.coverAddress')}
            value={current.artworkUrl?.startsWith('http') ? current.artworkUrl : ''}
            onChange={(event) => set('artworkUrl', event.target.value)}
            className="bg-background rounded-xl px-3 py-1.5 text-sm"
          />
          <input
            ref={picker}
            type="file"
            accept="image/*"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) cover.mutate(file);
              event.target.value = '';
            }}
          />
        </div>
      </div>
      {(save.error ?? cover.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(save.error ?? cover.error)?.message}
        </p>
      )}
      <div className="flex items-center justify-between gap-3">
        <button
          type="button"
          disabled={
            save.isPending ||
            (edit.data ? Object.values(edit.data).every((value) => value === null) : true)
          }
          onClick={() => save.mutate(EMPTY)}
          className="text-muted hover:text-foreground flex items-center gap-2 text-sm disabled:opacity-40"
        >
          <RotateCcw aria-hidden size={14} />
          {t('details.reset')}
        </button>
        <button
          type="submit"
          disabled={save.isPending || draft === null}
          className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-50"
        >
          {t('details.save')}
        </button>
      </div>
    </form>
  );
};
