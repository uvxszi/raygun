import { commands } from '@raygun/bindings';
import type { ImportedList } from '@raygun/bindings';
import { useMutation, useQuery } from '@tanstack/react-query';
import { FileText, ImageUp, Link2 } from 'lucide-react';
import { useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { uploadFile } from '../manga/useImportComics';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { ImportReview } from './ImportReview';
import { ScreenshotQueue } from './ScreenshotQueue';
import { sortByTaken } from './screenshotOrder';

type Way = 'link' | 'text' | 'screenshots';

const WAYS: { id: Way; icon: typeof Link2 }[] = [
  { id: 'link', icon: Link2 },
  { id: 'text', icon: FileText },
  { id: 'screenshots', icon: ImageUp },
];

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
  // Chosen screenshots wait in order (M46.40) until Read.
  const [shots, setShots] = useState<File[]>([]);
  // An artist line read as a song of its own is joined back to the song above it.
  const [joinSplit, setJoinSplit] = useState(true);
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
      return unwrapResult(commands.importReadScreenshots(ids, joinSplit));
    },
    onSuccess: setList,
  });

  return (
    <Sheet title={t('import.title')} onClose={onClose}>
      {list ? (
        <ImportReview list={list} onClose={onClose} />
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
                placeholder="https://open.spotify.com/playlist/…"
                className={SHEET_FIELD}
                inputClassName={`${SHEET_INPUT} text-sm`}
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
              ) : shots.length === 0 ? (
                <button
                  type="button"
                  disabled={read.isPending || reader.data !== true}
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
                  <button
                    type="button"
                    disabled={read.isPending || reader.data !== true}
                    onClick={() => read.mutate({ way: 'screenshots', files: shots })}
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
                  const chosen = sortByTaken([...(event.target.files ?? [])]);
                  if (chosen.length > 0) setShots([...shots, ...chosen]);
                  event.target.value = '';
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
