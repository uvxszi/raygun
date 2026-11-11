import { commands } from '@raygun/bindings';
import type { DetailEdit, DetailTarget } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ImagePlus, LoaderCircle, RotateCcw, Search } from 'lucide-react';
import { useRef, useState } from 'react';
import type { FC, KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { uploadFile } from '../manga/useImportComics';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { TextField } from '../shell/TextField';
import { detailKey, useDetailEdit } from './useDetailEdit';

type Field = 'title' | 'artist' | 'album';

/** The sources covers are searched on; those waiting for a key are counted in one line. */
const COVER_KINDS = ['music.metadata'];

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
          <TextField
            value={current[field] ?? ''}
            placeholder={original[field] ?? ''}
            onChange={(value) => set(field, value)}
            aria-label={labels[field]}
            className="bg-background rounded-xl pr-1 pl-3"
            inputClassName="py-2"
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
          <TextField
            type="url"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            aria-label={t('details.coverAddress')}
            placeholder={t('details.coverAddress')}
            value={current.artworkUrl?.startsWith('http') ? current.artworkUrl : ''}
            onChange={(value) => set('artworkUrl', value)}
            className="bg-background rounded-xl pr-1 pl-3"
            inputClassName="py-1.5 text-sm"
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
      <SourceCovers
        target={target}
        picked={current.artworkUrl}
        onPick={(url) => setDraft({ ...current, artworkUrl: url })}
      />
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

/** The names a cover search goes by: a song's title and artist, an album's, an artist's name. */
type CoverNames = { title: string; artist: string };

const namesOf = (target: DetailTarget): CoverNames | null => {
  switch (target.kind) {
    case 'song':
      return { title: target.query.title, artist: target.query.artist ?? '' };
    case 'album':
      return { title: target.album, artist: target.artist ?? '' };
    case 'artist':
      return { title: target.name, artist: '' };
    case 'playlist':
      return null;
  }
};

/** `target` searched by other names (its edit stays under its own). */
const searchedAs = (target: DetailTarget, names: CoverNames): DetailTarget => {
  const title = names.title.trim();
  const artist = names.artist.trim() || null;
  switch (target.kind) {
    case 'song':
      return { kind: 'song', query: { ...target.query, title, artist } };
    case 'album':
      return { kind: 'album', album: title, artist };
    case 'artist':
      return { kind: 'artist', name: title };
    case 'playlist':
      return target;
  }
};

/**
 * Covers from the music sources (M46.36): when one doesn't load, or the user prefers another
 * source's picture. Asked only on request, since it searches every source; the names it searches
 * by show above the covers, to change and search again (a misspelt title finds nothing).
 */
const SourceCovers: FC<{
  target: DetailTarget;
  picked: string | null;
  onPick: (url: string) => void;
}> = ({ target, picked, onPick }) => {
  const { t } = useTranslation();
  const [names, setNames] = useState(() => namesOf(target));
  const [asked, setAsked] = useState<DetailTarget | null>(null);
  const options = useQuery({
    queryKey: ['detailCoverOptions', JSON.stringify(asked)],
    queryFn: () => unwrapResult(commands.detailCoverOptions(asked ?? target)),
    enabled: asked !== null,
    staleTime: 10 * 60_000,
  });
  const search = () => setAsked(names ? searchedAs(target, names) : target);
  if (!asked) {
    return (
      <button
        type="button"
        onClick={search}
        className="bg-background flex items-center gap-1.5 self-start rounded-full px-3 py-1.5 text-sm"
      >
        <Search aria-hidden size={15} />
        {t('details.coverFind')}
      </button>
    );
  }
  // Inside the details form: Enter searches here rather than saving the details.
  const onEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    if (names?.title.trim()) search();
  };
  const list = options.data ?? [];
  return (
    <div className="flex flex-col gap-2">
      <span className="text-muted text-xs font-semibold uppercase">
        {t('details.coverSources')}
      </span>
      {names && (
        <div
          role="search"
          aria-label={t('details.coverSearch')}
          className="flex flex-wrap items-center gap-2"
        >
          <TextField
            value={names.title}
            onChange={(title) => setNames({ ...names, title })}
            onKeyDown={onEnter}
            aria-label={
              target.kind === 'song'
                ? t('details.coverSearchSong')
                : target.kind === 'album'
                  ? t('details.coverSearchAlbum')
                  : t('details.coverSearchName')
            }
            placeholder={
              target.kind === 'song'
                ? t('details.songTitle')
                : target.kind === 'album'
                  ? t('details.albumTitle')
                  : t('details.artistName')
            }
            className="bg-background flex-1 basis-36 rounded-xl pr-1 pl-3"
            inputClassName="py-1.5 text-sm"
          />
          {target.kind !== 'artist' && (
            <TextField
              value={names.artist}
              onChange={(artist) => setNames({ ...names, artist })}
              onKeyDown={onEnter}
              aria-label={t('details.coverSearchArtist')}
              placeholder={t('details.artist')}
              className="bg-background flex-1 basis-28 rounded-xl pr-1 pl-3"
              inputClassName="py-1.5 text-sm"
            />
          )}
          <button
            type="button"
            disabled={!names.title.trim() || options.isFetching}
            onClick={search}
            className="bg-primary text-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
          >
            <Search aria-hidden size={14} />
            {t('details.coverSearchAgain')}
          </button>
        </div>
      )}
      {options.isPending ? (
        <p className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={15} className="animate-spin" />
          {t('details.coverFinding')}
        </p>
      ) : list.length === 0 ? (
        <p className="text-muted text-sm">{t('details.coverNone')}</p>
      ) : (
        <ul
          data-testid="source-covers"
          className="grid max-h-56 grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-2 overflow-y-auto"
        >
          {list.map((option) => {
            const label = [option.source, option.of].filter(Boolean).join(' · ');
            return (
              <li key={option.url}>
                <button
                  type="button"
                  aria-pressed={picked === option.url}
                  aria-label={t('details.coverFrom', { source: label })}
                  title={label}
                  onClick={() => onPick(option.url)}
                  className={`flex w-full flex-col gap-1 rounded-lg p-1 text-left ${picked === option.url ? 'ring-primary ring-2' : 'hover:bg-background'}`}
                >
                  <Artwork url={option.url} className="w-full rounded-md" iconSize={20} />
                  <span className="text-muted truncate text-[calc(0.625rem*var(--text-scale,1))]">
                    {option.source}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {!options.isPending && <NeedsSetupNote kinds={COVER_KINDS} />}
    </div>
  );
};
