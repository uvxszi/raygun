import { commands } from '@raygun/bindings';
import type { ImportedList, MatchedSong } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { CircleCheck, CircleDashed, FileText, ImageUp, Link2, LoaderCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { uploadFile } from '../manga/useImportComics';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { COLLECTIONS } from './useCollections';

type Way = 'link' | 'text' | 'screenshots';

/** Songs matched per call, so the list fills in as it goes. */
const MATCH_STEP = 8;

const WAYS: { id: Way; icon: typeof Link2 }[] = [
  { id: 'link', icon: Link2 },
  { id: 'text', icon: FileText },
  { id: 'screenshots', icon: ImageUp },
];

/** The songs read, matched a few at a time against the music sources. */
const Review: FC<{ list: ImportedList; onClose: () => void }> = ({ list, onClose }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const [name, setName] = useState(list.name);
  const [matched, setMatched] = useState<MatchedSong[]>([]);
  const [skipped, setSkipped] = useState<Set<number>>(new Set());
  const started = useRef(false);

  // Matches the songs in steps; a failed step leaves those songs as they were read.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    let cancelled = false;
    void (async () => {
      for (let start = 0; start < list.songs.length && !cancelled; start += MATCH_STEP) {
        const step = list.songs.slice(start, start + MATCH_STEP);
        const answer = await commands.importMatch(step);
        const songs: MatchedSong[] =
          answer.status === 'ok'
            ? answer.data
            : step.map((song) => ({ song, matchedBy: null, source: null }));
        if (!cancelled) setMatched((done) => [...done, ...songs]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [list.songs]);

  const save = useMutation({
    mutationFn: () => {
      const chosen = list.songs.map(
        (song, index) => matched[index] ?? { song, matchedBy: null, source: null },
      );
      return unwrapResult(
        commands.importSave(
          name.trim() || list.name,
          chosen.filter((_, index) => !skipped.has(index)),
        ),
      );
    },
    onSuccess: (id) => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      onClose();
      void navigate({ to: '/library/playlist', search: { id } });
    },
  });
  const found = matched.filter((song) => song.matchedBy).length;
  const done = matched.length >= list.songs.length;

  return (
    <div data-testid="import-review" className="flex flex-col gap-3">
      <input
        value={name}
        onChange={(event) => setName(event.target.value)}
        aria-label={t('collections.playlistName')}
        className="bg-background rounded-xl px-4 py-3 text-sm outline-none"
      />
      <p role="status" className="text-muted text-sm">
        {done
          ? found === list.songs.length
            ? t('import.matchedAll', { count: found })
            : t('import.matched', { found, count: list.songs.length })
          : t('import.matching', { done: matched.length, count: list.songs.length })}
        {list.capped && ` ${t('import.capped')}`}
      </p>
      <ul className="flex max-h-[45vh] flex-col overflow-y-auto">
        {list.songs.map((song, index) => {
          const result = matched[index];
          return (
            <li key={`${song.title}-${index}`}>
              <label className="flex items-center gap-3 rounded-lg px-1 py-1.5">
                <input
                  type="checkbox"
                  checked={!skipped.has(index)}
                  onChange={() =>
                    setSkipped((current) => {
                      const next = new Set(current);
                      if (next.has(index)) next.delete(index);
                      else next.add(index);
                      return next;
                    })
                  }
                  className="accent-primary"
                />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate text-sm font-medium">{song.title}</span>
                  <span className="text-muted truncate text-xs">{song.artist}</span>
                </span>
                {!result ? (
                  <LoaderCircle aria-hidden size={16} className="text-muted animate-spin" />
                ) : result.matchedBy ? (
                  <CircleCheck
                    aria-label={t('import.foundOn', { source: result.matchedBy })}
                    size={16}
                    className="text-primary"
                  />
                ) : (
                  <CircleDashed
                    aria-label={t('import.notFound')}
                    size={16}
                    className="text-muted"
                  />
                )}
              </label>
            </li>
          );
        })}
      </ul>
      {save.error && <SourceIssue>{save.error.message}</SourceIssue>}
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
          disabled={save.isPending || skipped.size === list.songs.length}
          onClick={() => save.mutate()}
          className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
        >
          {t('import.create', { count: list.songs.length - skipped.size })}
        </button>
      </div>
    </div>
  );
};

/**
 * Importing songs (M46.21): a Spotify playlist, album or song link, a CSV export or pasted
 * lines, or screenshots of a song list. The songs are matched against the music sources and
 * saved as a playlist; each plays through the sources like any other.
 */
export const ImportSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const [way, setWay] = useState<Way>('link');
  const [link, setLink] = useState('');
  const [text, setText] = useState('');
  const [list, setList] = useState<ImportedList | null>(null);
  const files = useRef<HTMLInputElement>(null);
  const reader = useQuery({
    queryKey: ['import', 'reader'],
    queryFn: () => commands.importReaderReady(),
  });
  const prepare = useMutation({
    mutationFn: () => unwrapResult(commands.importPrepareReader()),
    onSuccess: () => void reader.refetch(),
  });
  const read = useMutation({
    mutationFn: async (input: { way: Way; files?: File[] }) => {
      if (input.way === 'link') return unwrapResult(commands.importReadLink(link));
      if (input.way === 'text') return unwrapResult(commands.importReadText(text, null));
      const ids = [];
      for (const file of input.files ?? []) ids.push(await uploadFile(file));
      return unwrapResult(commands.importReadScreenshots(ids));
    },
    onSuccess: setList,
  });

  return (
    <Sheet title={t('import.title')} onClose={onClose}>
      {list ? (
        <Review list={list} onClose={onClose} />
      ) : (
        <div className="flex flex-col gap-4">
          <div role="tablist" className="flex gap-2">
            {WAYS.map(({ id, icon: Icon }) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={way === id}
                onClick={() => setWay(id)}
                className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm ${way === id ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
              >
                <Icon aria-hidden size={14} />
                {t(`import.ways.${id}`)}
              </button>
            ))}
          </div>
          {way === 'link' && (
            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                read.mutate({ way: 'link' });
              }}
            >
              <input
                value={link}
                onChange={(event) => setLink(event.target.value)}
                aria-label={t('import.linkLabel')}
                placeholder="https://open.spotify.com/playlist/…"
                className="bg-background placeholder:text-muted rounded-xl px-4 py-3 text-sm outline-none"
              />
              <p className="text-muted text-xs">{t('import.linkHint')}</p>
              <button
                type="submit"
                disabled={!link.trim() || read.isPending}
                className="bg-primary text-background self-end rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
              >
                {t(read.isPending ? 'import.reading' : 'import.read')}
              </button>
            </form>
          )}
          {way === 'text' && (
            <div className="flex flex-col gap-2">
              <textarea
                value={text}
                onChange={(event) => setText(event.target.value)}
                aria-label={t('import.textLabel')}
                placeholder={t('import.textPlaceholder')}
                rows={6}
                className="bg-background placeholder:text-muted rounded-xl px-4 py-3 text-sm outline-none"
              />
              <p className="text-muted text-xs">{t('import.textHint')}</p>
              <div className="flex justify-end gap-2">
                <label className="bg-background cursor-pointer rounded-full px-4 py-2 text-sm">
                  {t('import.openCsv')}
                  <input
                    type="file"
                    accept=".csv,.txt,text/csv,text/plain"
                    className="hidden"
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      if (file) void file.text().then(setText);
                    }}
                  />
                </label>
                <button
                  type="button"
                  disabled={!text.trim() || read.isPending}
                  onClick={() => read.mutate({ way: 'text' })}
                  className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
                >
                  {t(read.isPending ? 'import.reading' : 'import.read')}
                </button>
              </div>
            </div>
          )}
          {way === 'screenshots' && (
            <div className="flex flex-col gap-2">
              <p className="text-muted text-sm">{t('import.screenshotsHint')}</p>
              {reader.data === false ? (
                <button
                  type="button"
                  disabled={prepare.isPending}
                  onClick={() => prepare.mutate()}
                  className="bg-background self-start rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
                >
                  {t(prepare.isPending ? 'import.downloadingReader' : 'import.downloadReader')}
                </button>
              ) : (
                <button
                  type="button"
                  disabled={read.isPending || reader.data !== true}
                  onClick={() => files.current?.click()}
                  className="bg-primary text-background self-start rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
                >
                  {t(read.isPending ? 'import.reading' : 'import.chooseScreenshots')}
                </button>
              )}
              <input
                ref={files}
                type="file"
                accept="image/*"
                multiple
                aria-label={t('import.chooseScreenshots')}
                className="hidden"
                onChange={(event) => {
                  const chosen = [...(event.target.files ?? [])];
                  if (chosen.length > 0) read.mutate({ way: 'screenshots', files: chosen });
                }}
              />
              {prepare.error && <SourceIssue>{prepare.error.message}</SourceIssue>}
            </div>
          )}
          {read.error && <SourceIssue>{read.error.message}</SourceIssue>}
        </div>
      )}
    </Sheet>
  );
};
