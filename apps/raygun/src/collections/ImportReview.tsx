import { DndContext, PointerSensor, closestCenter, useSensor, useSensors } from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { commands } from '@raygun/bindings';
import type {
  ImportedList,
  ImportedSong,
  MatchedSong,
  MediaSource,
  OnlineTrack,
  SongCandidate,
} from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  ArrowDown,
  ArrowUp,
  CircleCheck,
  CircleDashed,
  GripVertical,
  LoaderCircle,
  Play,
  Plus,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatTime } from '../player/formatTime';
import { playResults } from '../search/useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';
import { savesIn, useSavedSongs } from './savedSongs';
import { COLLECTIONS, usePlaylists } from './useCollections';

/** Songs matched at once, so the list fills in as it goes. */
const MATCH_TOGETHER = 4;

type Row = {
  id: number;
  song: ImportedSong;
  /** Undefined while it's being matched. */
  result?: MatchedSong;
  skipped: boolean;
  /** The candidate picked in the sources list; the first one until the user picks. */
  chosen?: string;
};

const pendingOf = (song: ImportedSong): MatchedSong => ({
  song,
  matchedBy: null,
  source: null,
  candidates: [],
  pending: true,
});

const lookupOf = (track: OnlineTrack): MediaSource => ({
  kind: 'lookup',
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
});

/**
 * A candidate as the matched song it would make, as Rust's matching makes it: a title alone takes
 * the candidate's artist, and the song plays the candidate's own stream, else by the candidate's
 * names, which find what its preview played (M46.48).
 */
const fromCandidate = (row: Row, candidate: SongCandidate): MatchedSong => ({
  song: {
    ...row.song,
    artist: row.song.artist ?? candidate.track.artist,
    album: row.song.album ?? candidate.track.album,
    durationMs: row.song.durationMs ?? candidate.track.durationMs,
    artworkUrl: row.song.artworkUrl ?? candidate.track.artworkUrl,
  },
  matchedBy: candidate.providerName,
  source:
    candidate.track.source?.kind === 'url' ? candidate.track.source : lookupOf(candidate.track),
  candidates: row.result?.candidates ?? [],
  pending: false,
});

/** What a row saves: its match, unless its names changed since (then it's matched again). */
const resultFor = (row: Row): MatchedSong =>
  row.result &&
  row.result.song.title === row.song.title &&
  (row.song.artist === null || row.result.song.artist === row.song.artist)
    ? row.result
    : pendingOf(row.song);

const ReviewRow: FC<{
  row: Row;
  index: number;
  count: number;
  /** The destination's name when it holds the song already (M46.48). */
  already: string | null;
  onChange: (row: Row, rematch: boolean) => void;
  onMove: (from: number, to: number) => void;
}> = ({ row, index, count, already, onChange, onMove }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: row.id,
  });
  const result = row.result;
  const candidates = result?.candidates ?? [];
  const edit = (field: 'title' | 'artist', value: string) =>
    onChange(
      { ...row, song: { ...row.song, [field]: field === 'artist' && !value ? null : value } },
      false,
    );
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={`flex flex-col rounded-lg ${isDragging ? 'bg-surface relative z-10' : ''} ${row.skipped ? 'opacity-50' : ''}`}
    >
      <div className="flex items-center gap-2 px-1 py-1">
        <button
          type="button"
          aria-label={t('import.drag')}
          className="text-muted cursor-grab touch-none"
          {...attributes}
          {...listeners}
        >
          <GripVertical aria-hidden size={16} />
        </button>
        <input
          type="checkbox"
          aria-label={t('import.keep', { title: row.song.title })}
          checked={!row.skipped}
          onChange={() => onChange({ ...row, skipped: !row.skipped }, false)}
          className="accent-primary"
        />
        <span className="flex min-w-0 flex-1 flex-col">
          <input
            aria-label={t('import.songTitle')}
            value={row.song.title}
            onChange={(event) => edit('title', event.target.value)}
            onBlur={() => onChange(row, true)}
            className="focus:bg-background truncate rounded px-1 text-sm font-medium outline-none"
          />
          <input
            aria-label={t('import.songArtist')}
            value={row.song.artist ?? ''}
            // A title alone is saved with its match's artist (M46.48).
            placeholder={result?.song.artist ?? t('import.noArtist')}
            onChange={(event) => edit('artist', event.target.value)}
            onBlur={() => onChange(row, true)}
            className="text-muted focus:bg-background truncate rounded px-1 text-xs outline-none"
          />
          {already && (
            <span className="text-muted px-1 text-xs">
              {t('collections.alreadyIn', { name: already })}
            </span>
          )}
        </span>
        {candidates.length > 1 && (
          <button
            type="button"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
            className="bg-background shrink-0 rounded-full px-2.5 py-1 text-xs"
          >
            {t('import.sources', { count: candidates.length })}
          </button>
        )}
        {!result ? (
          <LoaderCircle
            aria-label={t('import.searching')}
            size={16}
            className="text-muted shrink-0 animate-spin"
          />
        ) : result.matchedBy ? (
          <CircleCheck
            aria-label={t('import.foundOn', { source: result.matchedBy })}
            size={16}
            className="text-primary shrink-0"
          />
        ) : (
          <CircleDashed
            aria-label={t('import.unavailable')}
            size={16}
            className="text-muted shrink-0"
          />
        )}
        <span className="flex shrink-0 flex-col">
          <button
            type="button"
            aria-label={t('import.moveUp', { title: row.song.title })}
            disabled={index === 0}
            onClick={() => onMove(index, index - 1)}
            className="text-muted hover:text-foreground disabled:opacity-20"
          >
            <ArrowUp aria-hidden size={12} />
          </button>
          <button
            type="button"
            aria-label={t('import.moveDown', { title: row.song.title })}
            disabled={index === count - 1}
            onClick={() => onMove(index, index + 1)}
            className="text-muted hover:text-foreground disabled:opacity-20"
          >
            <ArrowDown aria-hidden size={12} />
          </button>
        </span>
      </div>
      {open && (
        <ul
          aria-label={t('import.sourcesOf', { title: row.song.title })}
          className="bg-background ml-8 flex flex-col rounded-lg p-1"
        >
          {candidates.map((candidate) => {
            const inUse = row.chosen ?? (result?.matchedBy ? candidates[0]?.providerId : undefined);
            const using = inUse === candidate.providerId;
            return (
              <li key={candidate.providerId} className="flex items-center gap-2 px-2 py-1 text-xs">
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate font-medium">
                    {candidate.track.title} · {candidate.track.artist}
                  </span>
                  <span className="text-muted truncate">
                    {[
                      candidate.providerName,
                      candidate.track.durationMs ? formatTime(candidate.track.durationMs) : null,
                    ]
                      .filter(Boolean)
                      .join(' · ')}
                  </span>
                </span>
                <button
                  type="button"
                  aria-label={t('import.preview', { source: candidate.providerName })}
                  onClick={() => void playResults([candidate.track], candidate.track)}
                  className="text-muted hover:text-foreground grid size-7 place-items-center rounded-full"
                >
                  <Play aria-hidden size={13} />
                </button>
                <button
                  type="button"
                  disabled={using}
                  onClick={() =>
                    onChange(
                      {
                        ...row,
                        result: fromCandidate(row, candidate),
                        chosen: candidate.providerId,
                      },
                      false,
                    )
                  }
                  className="bg-surface rounded-full px-2.5 py-1 disabled:opacity-50"
                >
                  {using ? t('import.using') : t('import.use')}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </li>
  );
};

/**
 * The songs read (M46.31): names to fix, songs to add, an order to drag, other sources to try,
 * and Import at any time: Rust keeps matching what's left and fills the playlist in.
 */
export const ImportReview: FC<{ list: ImportedList; onClose: () => void }> = ({
  list,
  onClose,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const [name, setName] = useState(list.name);
  // Where the songs go (M46.45): 'new', 'favorites' or a playlist's id.
  const [destination, setDestination] = useState('new');
  const playlists = usePlaylists();
  const [rows, setRows] = useState<Row[]>(() =>
    list.songs.map((song, id) => ({ id, song, skipped: false })),
  );
  const [adding, setAdding] = useState({ title: '', artist: '' });
  const nextId = useRef(list.songs.length);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  // Matches rows waiting for it, a few at a time; a failed call leaves them unmatched.
  const waiting = rows.filter((row) => !row.result).map((row) => row.id);
  const busy = useRef(false);
  useEffect(() => {
    if (busy.current || waiting.length === 0) return;
    busy.current = true;
    const ids = waiting.slice(0, MATCH_TOGETHER);
    const songs = rows.filter((row) => ids.includes(row.id)).map((row) => row.song);
    void commands.importMatch(songs).then((answer) => {
      busy.current = false;
      setRows((current) =>
        current.map((row) => {
          const at = ids.indexOf(row.id);
          if (at < 0 || row.result) return row;
          // A row edited meanwhile is matched again.
          if (row.song !== songs[at]) return row;
          const found =
            answer.status === 'ok' ? answer.data[at] : { ...pendingOf(row.song), pending: false };
          return { ...row, result: found };
        }),
      );
    });
  }, [rows, waiting]);

  const change = (next: Row, rematch: boolean) =>
    setRows((current) =>
      current.map((row) =>
        row.id === next.id
          ? {
              ...next,
              result: rematch ? undefined : next.result,
              chosen: rematch ? undefined : next.chosen,
            }
          : row,
      ),
    );
  const move = (from: number, to: number) =>
    setRows((current) => (to < 0 || to >= current.length ? current : arrayMove(current, from, to)));
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    setRows((current) =>
      arrayMove(
        current,
        current.findIndex((row) => row.id === active.id),
        current.findIndex((row) => row.id === over.id),
      ),
    );
  };
  const add = () => {
    if (!adding.title.trim()) return;
    const song: ImportedSong = {
      title: adding.title.trim(),
      artist: adding.artist.trim() || null,
      album: null,
      durationMs: null,
      artworkUrl: null,
    };
    setRows((current) => [...current, { id: nextId.current++, song, skipped: false }]);
    setAdding({ title: '', artist: '' });
  };

  const kept = rows.filter((row) => !row.skipped);
  // Songs the playlist or Favorites holds already, by any of their names (M46.48).
  const saved = useSavedSongs();
  const destinationName =
    destination === 'favorites'
      ? t('collections.favorites')
      : (playlists.data?.find((playlist) => playlist.id === destination)?.name ?? '');
  const isAlready = (row: Row) => {
    if (destination === 'new' || !saved.data) return false;
    const saves = savesIn(saved.data, row.result?.song ?? row.song);
    return destination === 'favorites' ? saves.favorite : saves.playlistIds.includes(destination);
  };
  const alreadyCount = kept.filter(isAlready).length;
  // Asks Spotify's question before adding songs a playlist has: add the new ones, or all.
  const [asking, setAsking] = useState(false);
  const save = useMutation({
    mutationFn: (addExisting: boolean) =>
      unwrapResult(
        commands.importSave(
          destination === 'new'
            ? { kind: 'new', name: name.trim() || list.name }
            : destination === 'favorites'
              ? { kind: 'favorites' }
              : { kind: 'playlist', id: destination },
          kept.map(resultFor),
          addExisting,
        ),
      ),
    onSuccess: (id) => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      onClose();
      void navigate(
        id === 'favorites'
          ? { to: '/library', search: { view: 'favorites' } }
          : { to: '/library/playlist', search: { id } },
      );
    },
  });
  const matched = rows.filter((row) => row.result);
  const found = matched.filter((row) => row.result?.matchedBy).length;
  const done = matched.length === rows.length;

  return (
    <div data-testid="import-review" className="flex flex-col gap-3">
      <label className="flex items-center gap-2 text-sm">
        <span className="text-muted shrink-0">{t('import.saveTo')}</span>
        <select
          value={destination}
          onChange={(event) => {
            setDestination(event.target.value);
            setAsking(false);
          }}
          className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2.5 text-sm outline-none"
        >
          <option value="new">{t('collections.newPlaylist')}</option>
          <option value="favorites">{t('collections.favorites')}</option>
          {playlists.data?.map((playlist) => (
            <option key={playlist.id} value={playlist.id}>
              {playlist.name}
            </option>
          ))}
        </select>
      </label>
      {destination === 'new' && (
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          aria-label={t('collections.playlistName')}
          className="bg-background rounded-xl px-4 py-3 text-sm outline-none"
        />
      )}
      <p role="status" className="text-muted text-sm">
        {done
          ? found === rows.length
            ? t('import.matchedAll', { count: found })
            : t('import.matchedSome', { found, count: rows.length })
          : t('import.matching', { done: matched.length, count: rows.length })}
        {list.capped && ` ${t('import.capped')}`}
      </p>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={rows.map((row) => row.id)} strategy={verticalListSortingStrategy}>
          <ul data-testid="import-rows" className="flex max-h-[45vh] flex-col overflow-y-auto">
            {rows.map((row, index) => (
              <ReviewRow
                key={row.id}
                row={row}
                index={index}
                count={rows.length}
                already={!row.skipped && isAlready(row) ? destinationName : null}
                onChange={change}
                onMove={move}
              />
            ))}
          </ul>
        </SortableContext>
      </DndContext>
      <form
        className="flex items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        <input
          aria-label={t('import.addTitle')}
          placeholder={t('import.addTitle')}
          value={adding.title}
          onChange={(event) => setAdding({ ...adding, title: event.target.value })}
          className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2 text-sm"
        />
        <input
          aria-label={t('import.addArtist')}
          placeholder={t('import.addArtist')}
          value={adding.artist}
          onChange={(event) => setAdding({ ...adding, artist: event.target.value })}
          className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2 text-sm"
        />
        <button
          type="submit"
          aria-label={t('import.add')}
          disabled={!adding.title.trim()}
          className="bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-40"
        >
          <Plus aria-hidden size={16} />
        </button>
      </form>
      {!done && <p className="text-muted text-xs">{t('import.keepsLooking')}</p>}
      {save.error && <SourceIssue>{save.error.message}</SourceIssue>}
      {asking ? (
        <div role="alertdialog" aria-labelledby="import-already" className="flex flex-col gap-2">
          <p id="import-already" className="text-sm font-medium">
            {t('import.already', { count: alreadyCount, name: destinationName })}
          </p>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={save.isPending}
              onClick={() => save.mutate(true)}
              className="bg-background rounded-full px-5 py-2 text-sm disabled:opacity-60"
            >
              {t('import.addAll')}
            </button>
            <button
              type="button"
              disabled={save.isPending}
              onClick={() => save.mutate(false)}
              className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
            >
              {t('import.addNew')}
            </button>
          </div>
        </div>
      ) : (
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="bg-background rounded-full px-5 py-2 text-sm"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            disabled={save.isPending || kept.length === 0}
            // Favorites hold a song once whatever is asked; a playlist asks first.
            onClick={() =>
              alreadyCount > 0 && destination !== 'favorites' ? setAsking(true) : save.mutate(false)
            }
            className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
          >
            {destination === 'new'
              ? t('import.create', { count: kept.length })
              : t('import.addTo', { count: kept.length, name: destinationName })}
          </button>
        </div>
      )}
    </div>
  );
};
