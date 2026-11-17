import { commands, events } from '@raygun/bindings';
import type { ImportProgress, ImportedList } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { FileText, ImageUp, Link2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { uploadFile } from '../manga/useImportComics';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { ImportReview } from './ImportReview';
import { keepInMemory } from './pictureCrop';
import { READERS, readerName, useImportReaders } from './importReaders';
import { ScreenshotQueue } from './ScreenshotQueue';
import { sortByTaken } from './screenshotOrder';

type Way = 'link' | 'text' | 'screenshots';

const WAYS: { id: Way; icon: typeof Link2 }[] = [
  { id: 'link', icon: Link2 },
  { id: 'text', icon: FileText },
  { id: 'screenshots', icon: ImageUp },
];

/** Reads screenshots (copies held in memory) with a reader; `null` for Settings → Import's. */
const readPictures = async (files: File[], joinSplit: boolean, reader: string | null) => {
  const ids = [];
  for (const file of files) ids.push(await uploadFile(file));
  return unwrapResult(commands.importReadScreenshots(ids, joinSplit, reader));
};

/** Where reading screenshots stands, as one line. */
const ProgressLine: FC<{ progress: ImportProgress }> = ({ progress }) => {
  const { t } = useTranslation();
  const share =
    progress.pictures === 0
      ? 0
      : ((progress.picture - (progress.stage === 'done' || progress.stage === 'rows' ? 0 : 1)) /
          progress.pictures) *
        100;
  return (
    <div role="status" className="flex flex-col gap-1.5 text-sm">
      <span>
        {progress.stage === 'done'
          ? t('import.progress.done', { count: progress.rows })
          : t(`import.progress.${progress.stage}`, {
              number: progress.picture,
              count: progress.pictures,
              rows: progress.rows,
            })}
      </span>
      <span className="bg-background h-1.5 overflow-hidden rounded-full">
        <span
          className="bg-primary block h-full rounded-full transition-[width]"
          style={{ width: `${Math.max(4, share)}%` }}
        />
      </span>
    </div>
  );
};

/**
 * Importing songs (M46.21): a Spotify playlist, album or song link, a CSV export or pasted
 * lines, or screenshots of a song list. The songs are matched against the music sources and
 * saved as a playlist; each plays through the sources like any other. While songs are being
 * reviewed the sheet closes only by its own buttons: a tap outside does nothing and Back asks.
 */
export const ImportSheet: FC<{ onClose: () => void; initialLink?: string }> = ({
  onClose,
  initialLink = '',
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [way, setWay] = useState<Way>('link');
  const [link, setLink] = useState(initialLink);
  const [text, setText] = useState('');
  const [list, setList] = useState<ImportedList | null>(null);
  // A new list read by another reader starts the review over.
  const [reviews, setReviews] = useState(0);
  const [leaving, setLeaving] = useState(false);
  // Chosen screenshots wait in order (M46.40) until Read, held in memory from the moment they're
  // picked (`keepInMemory`).
  const [shots, setShots] = useState<File[]>([]);
  const [copying, setCopying] = useState(false);
  // An artist line read as a song of its own is joined back to the song above it.
  const [joinSplit, setJoinSplit] = useState(true);
  // The reader picked here; null for Settings → Import's choice.
  const [readerId, setReaderId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const files = useRef<HTMLInputElement>(null);
  const readers = useImportReaders();
  const reader =
    readers.data?.find((each) => each.id === readerId) ??
    readers.data?.find((each) => each.ready) ??
    readers.data?.[0];
  const prepare = useMutation({
    mutationFn: (id: string) => unwrapResult(commands.importPrepareReader(id)),
    onSuccess: () => void client.invalidateQueries({ queryKey: READERS }),
  });
  useEffect(() => {
    const unlisten = events.importProgressEvent.listen(({ payload }) => setProgress(payload));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  const read = useMutation({
    mutationFn: async (input: { way: Way }) => {
      if (input.way === 'link') return unwrapResult(commands.importReadLink(link));
      if (input.way === 'text') return unwrapResult(commands.importReadText(text, null));
      setProgress(null);
      return readPictures(shots, joinSplit, reader?.id ?? null);
    },
    onSuccess: setList,
    onSettled: () => setProgress(null),
  });
  const pick = async (chosen: File[]) => {
    setCopying(true);
    try {
      const copies = await Promise.all(sortByTaken(chosen).map(keepInMemory));
      setShots((current) => [...current, ...copies]);
    } finally {
      setCopying(false);
    }
  };

  return (
    <Sheet
      title={t('import.title')}
      onClose={onClose}
      onDismiss={list ? () => setLeaving(true) : undefined}
    >
      {list ? (
        <>
          {leaving && (
            <div
              role="alertdialog"
              aria-labelledby="import-leave"
              className="bg-background flex flex-col gap-2 rounded-xl p-3"
            >
              <p id="import-leave" className="text-sm font-medium">
                {t('import.leaveQuestion')}
              </p>
              <div className="flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setLeaving(false)}
                  className="bg-surface rounded-full px-4 py-2 text-sm"
                >
                  {t('import.keepReviewing')}
                </button>
                <button
                  type="button"
                  onClick={onClose}
                  className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
                >
                  {t('import.leave')}
                </button>
              </div>
            </div>
          )}
          <ImportReview
            key={reviews}
            list={list}
            onClose={onClose}
            pictures={
              (list.screenshots ?? []).length > 0
                ? {
                    files: shots,
                    read: (pictures, other) =>
                      readPictures(pictures, joinSplit, other ?? list.reader ?? null),
                  }
                : undefined
            }
            onReread={(next) => {
              setList(next);
              setReviews((count) => count + 1);
            }}
          />
        </>
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
              <TextField
                type="url"
                inputMode="url"
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                value={link}
                onChange={setLink}
                aria-label={t('import.linkLabel')}
                placeholder="https://…"
                className={SHEET_FIELD}
                inputClassName={`${SHEET_INPUT} text-sm`}
              />
              <p className="text-muted text-xs">
                {t('import.linkHint')}{' '}
                <Link to="/settings/import" onClick={onClose} className="underline">
                  {t('import.spotifyKeysLink')}
                </Link>
              </p>
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
              {readers.data && readers.data.length > 1 && (
                <label className="flex items-center gap-2 text-sm">
                  <span className="text-muted shrink-0">{t('import.reader')}</span>
                  <select
                    value={reader?.id ?? ''}
                    disabled={read.isPending}
                    onChange={(event) => setReaderId(event.target.value)}
                    className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2 text-sm outline-none"
                  >
                    {readers.data.map((each) => (
                      <option key={each.id} value={each.id}>
                        {each.ready
                          ? readerName(each, t)
                          : t('import.readerToDownload', {
                              name: readerName(each, t),
                              size: Math.round((each.sizeBytes ?? 0) / 1_000_000),
                            })}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {reader && !reader.ready ? (
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    disabled={prepare.isPending}
                    onClick={() => prepare.mutate(reader.id)}
                    className="bg-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
                  >
                    {prepare.isPending
                      ? t('import.downloadingReader')
                      : t('import.downloadReader', {
                          size: Math.round((reader.sizeBytes ?? 0) / 1_000_000),
                        })}
                  </button>
                  {/* Stops it at once; the reader is offered again (Rust answers as if done). */}
                  {prepare.isPending && reader.model && (
                    <button
                      type="button"
                      onClick={() => void commands.aiModelCancel(reader.model ?? '')}
                      className="text-muted hover:text-foreground rounded-full px-3 py-2 text-sm font-semibold"
                    >
                      {t('import.cancelReader')}
                    </button>
                  )}
                </div>
              ) : shots.length === 0 ? (
                <button
                  type="button"
                  disabled={read.isPending || copying || !reader?.ready}
                  onClick={() => files.current?.click()}
                  className="bg-primary text-background self-start rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
                >
                  {t('import.chooseScreenshots')}
                </button>
              ) : null}
              {shots.length > 0 && (
                <>
                  <ScreenshotQueue files={shots} onChange={setShots} disabled={read.isPending} />
                  <label className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={joinSplit}
                      disabled={read.isPending}
                      onChange={() => setJoinSplit(!joinSplit)}
                      className="accent-primary"
                    />
                    {t('import.joinSplit')}
                  </label>
                  {read.isPending && progress && <ProgressLine progress={progress} />}
                  <button
                    type="button"
                    disabled={read.isPending || copying || !reader?.ready}
                    onClick={() => read.mutate({ way: 'screenshots' })}
                    className="bg-primary text-background self-start rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
                  >
                    {read.isPending
                      ? t('import.reading')
                      : t('import.readScreenshots', { count: shots.length })}
                  </button>
                </>
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
                  event.target.value = '';
                  if (chosen.length > 0) void pick(chosen);
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
