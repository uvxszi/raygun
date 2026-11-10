import type { EditedLyrics, LyricLine, TrackQuery } from '@raygun/bindings';
import { Minus, Plus, Stamp, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { currentLine, useLyricsFor } from '../player/lyricsQuery';
import { useLivePosition } from '../player/useLivePosition';
import { sendControl, useNowPlaying } from '../player/usePlayback';
import { formatOffset, formatTrim, parseOffset, parseTrim } from './format';
import { LyricsSources } from './LyricsSources';
import type { FullSongEdit } from './soundPrefs';

/** How far one press moves the lyrics, ms (Nuclear's step). */
const OFFSET_STEP = 100;

const sameSong = (left: TrackQuery, right: { title: string; artist: string | null }) =>
  left.title === right.title && (left.artist ?? '') === (right.artist ?? '');

/** The offset as a field between − and +: typed seconds apply on Enter or when it's left. */
const OffsetField: FC<{ offset: number; onChange: (ms: number) => void }> = ({
  offset,
  onChange,
}) => {
  const { t } = useTranslation();
  // What's typed, while it's being typed; the offset itself otherwise.
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    const ms = parseOffset(draft);
    setDraft(null);
    if (ms !== null && ms !== offset) onChange(ms);
  };
  return (
    <span className="bg-background flex items-center rounded-full pr-3 pl-1">
      <input
        type="text"
        data-testid="lyrics-offset"
        aria-label={t('lyricsEdit.offsetSeconds')}
        value={draft ?? formatOffset(offset)}
        onChange={(event) => setDraft(event.target.value)}
        onFocus={(event) => event.currentTarget.select()}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault();
            commit();
          } else if (event.key === 'Escape' && draft !== null) {
            // Back to the offset as it was; the sheet stays open.
            event.stopPropagation();
            setDraft(null);
          }
        }}
        aria-invalid={draft !== null && parseOffset(draft) === null}
        autoComplete="off"
        spellCheck={false}
        enterKeyHint="done"
        className="w-16 bg-transparent py-1.5 text-center text-sm tabular-nums outline-none aria-invalid:text-red-500"
      />
      <span aria-hidden className="text-muted text-sm">
        s
      </span>
    </span>
  );
};

/** Lines as the person edits them: times as typed, so a half-typed time isn't lost. */
type Draft = { time: string; text: string }[];

const toDraft = (lines: LyricLine[]): Draft =>
  lines.map((line) => ({ time: formatTrim(line.timeMs), text: line.text }));

/** The draft as lines, or the first line whose time isn't one. */
const fromDraft = (draft: Draft): LyricLine[] | number => {
  const lines: LyricLine[] = [];
  for (const [index, row] of draft.entries()) {
    const timeMs = parseTrim(row.time);
    if (timeMs === null) return index;
    lines.push({ timeMs, text: row.text });
  }
  return lines.sort((left, right) => left.timeMs - right.timeMs);
};

/**
 * A song's lyrics, made right (M46.12): shift them with an offset (or sync to a line as it's
 * sung, as cloudstream does for subtitles), edit lines and their times, stamp each line's time
 * while the song plays, turn plain lyrics into synced ones, or write lyrics no source has. The
 * edits replace the sources' lyrics wherever the song plays; "Use the source's lyrics" undoes them.
 */
export const LyricsEditor: FC<{
  song: TrackQuery;
  edit: FullSongEdit;
  save: (edit: FullSongEdit) => void;
}> = ({ song, edit, save }) => {
  const { t } = useTranslation();
  const lyrics = useLyricsFor(song);
  const { state, item } = useNowPlaying();
  const position = useLivePosition(state);
  const playing = item !== undefined && sameSong(song, item);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [plainDraft, setPlainDraft] = useState<string | null>(null);
  const [next, setNext] = useState(0);
  const [problem, setProblem] = useState<string | null>(null);
  const offset = edit.lyricsOffsetMs;
  const synced = lyrics.data?.synced ?? [];
  const plain = lyrics.data?.plain ?? '';

  const setOffset = (lyricsOffsetMs: number) => save({ ...edit, lyricsOffsetMs });
  const saveLyrics = (own: EditedLyrics) => save({ ...edit, lyrics: own });

  const startEditing = () => {
    setDraft(toDraft(synced));
    setNext(0);
    setProblem(null);
  };
  const commitDraft = () => {
    if (!draft) return;
    const lines = fromDraft(draft);
    if (typeof lines === 'number') {
      setProblem(t('lyricsEdit.badTime', { line: lines + 1 }));
      return;
    }
    saveLyrics({ synced: lines, plain: plain || null });
    setDraft(null);
  };
  /** The playing position, as the next line's time (offset taken off, so it shows right now). */
  const stamp = () => {
    if (!draft || next >= draft.length) return;
    const time = formatTrim(Math.max(0, Math.round(position - offset)));
    setDraft(draft.map((row, index) => (index === next ? { ...row, time } : row)));
    setNext(next + 1);
  };

  if (lyrics.isPending) {
    return <p className="text-muted text-sm">{t('player.lyricsLoading')}</p>;
  }

  // Plain text: edited as a whole, or split into lines to sync.
  if (plainDraft !== null || (synced.length === 0 && draft === null)) {
    const text = plainDraft ?? plain;
    return (
      <div data-testid="lyrics-editor" className="flex flex-col gap-3">
        <p className="text-muted text-sm">
          {lyrics.data ? t('lyricsEdit.plainHint') : t('lyricsEdit.noneHint')}
        </p>
        <textarea
          value={text}
          onChange={(event) => setPlainDraft(event.target.value)}
          aria-label={t('lyricsEdit.words')}
          rows={10}
          className="bg-background placeholder:text-muted rounded-xl px-3 py-2 text-sm outline-none"
          placeholder={t('lyricsEdit.placeholder')}
        />
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={plainDraft === null || plainDraft === plain}
            onClick={() => {
              saveLyrics({ synced: [], plain: (plainDraft ?? '').trim() || null });
              setPlainDraft(null);
            }}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {t('lyricsEdit.save')}
          </button>
          <button
            type="button"
            disabled={text.trim() === ''}
            onClick={() => {
              const rows = text
                .split('\n')
                .map((line) => line.trim())
                .filter((line) => line !== '')
                .map((line) => ({ time: formatTrim(0), text: line }));
              setPlainDraft(null);
              setDraft(rows);
              setNext(0);
            }}
            className="bg-background hover:bg-surface rounded-full px-4 py-2 text-sm"
          >
            {t('lyricsEdit.makeSynced')}
          </button>
        </div>
        <LyricsSources song={song} edit={edit} save={save} />
        <SourceLink
          edited={lyrics.data?.edited ?? false}
          onUndo={() => save({ ...edit, lyrics: null })}
        />
      </div>
    );
  }

  // Synced lines being edited: times and words, stamping as the song plays.
  if (draft !== null) {
    return (
      <div data-testid="lyrics-editor" className="flex flex-col gap-3">
        <p className="text-muted text-sm">{t('lyricsEdit.editHint')}</p>
        {playing ? (
          <button
            type="button"
            onClick={stamp}
            disabled={next >= draft.length}
            className="bg-primary text-background flex items-center justify-center gap-2 rounded-full px-4 py-2.5 text-sm font-semibold disabled:opacity-50"
          >
            <Stamp aria-hidden size={16} />
            {next < draft.length
              ? t('lyricsEdit.stamp', { line: draft[next]?.text || '♪' })
              : t('lyricsEdit.stamped')}
          </button>
        ) : (
          <p className="text-muted text-xs">{t('lyricsEdit.playToStamp')}</p>
        )}
        <ol className="flex max-h-[45vh] flex-col gap-1.5 overflow-y-auto">
          {draft.map((row, index) => (
            <li
              key={index}
              className={`flex items-center gap-2 rounded-lg ${index === next ? 'ring-primary ring-1' : ''}`}
            >
              <input
                value={row.time}
                onChange={(event) =>
                  setDraft(
                    draft.map((each, at) =>
                      at === index ? { ...each, time: event.target.value } : each,
                    ),
                  )
                }
                onFocus={() => setNext(index)}
                aria-label={t('lyricsEdit.lineTime', { line: index + 1 })}
                aria-invalid={parseTrim(row.time) === null}
                className="bg-background w-24 rounded-lg px-2 py-1.5 text-sm tabular-nums outline-none aria-invalid:ring-1 aria-invalid:ring-red-500"
              />
              <input
                value={row.text}
                onChange={(event) =>
                  setDraft(
                    draft.map((each, at) =>
                      at === index ? { ...each, text: event.target.value } : each,
                    ),
                  )
                }
                aria-label={t('lyricsEdit.lineText', { line: index + 1 })}
                className="bg-background min-w-0 flex-1 rounded-lg px-2 py-1.5 text-sm outline-none"
              />
              <button
                type="button"
                aria-label={t('lyricsEdit.remove', { line: index + 1 })}
                onClick={() => setDraft(draft.filter((_, at) => at !== index))}
                className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full"
              >
                <Trash2 aria-hidden size={14} />
              </button>
            </li>
          ))}
        </ol>
        {problem && (
          <p role="alert" className="text-primary text-sm">
            {problem}
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={commitDraft}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('lyricsEdit.save')}
          </button>
          <button
            type="button"
            onClick={() => {
              const last = draft.at(-1);
              const after = last ? (parseTrim(last.time) ?? 0) + 1000 : 0;
              setDraft([...draft, { time: formatTrim(after), text: '' }]);
            }}
            className="bg-background hover:bg-surface rounded-full px-4 py-2 text-sm"
          >
            {t('lyricsEdit.addLine')}
          </button>
          <button
            type="button"
            onClick={() => setDraft(null)}
            className="text-muted hover:text-foreground rounded-full px-4 py-2 text-sm"
          >
            {t('lyricsEdit.cancel')}
          </button>
        </div>
      </div>
    );
  }

  // Synced lyrics: the offset, syncing to a line, and the lines as they'll show.
  const current = playing ? currentLine(synced, position, offset) : -1;
  return (
    <div data-testid="lyrics-editor" className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium">{t('lyricsEdit.offset')}</span>
        <button
          type="button"
          aria-label={t('lyricsEdit.earlier')}
          onClick={() => setOffset(offset - OFFSET_STEP)}
          className="bg-background hover:bg-surface grid size-8 place-items-center rounded-full"
        >
          <Minus aria-hidden size={14} />
        </button>
        <OffsetField offset={offset} onChange={setOffset} />
        <button
          type="button"
          aria-label={t('lyricsEdit.later')}
          onClick={() => setOffset(offset + OFFSET_STEP)}
          className="bg-background hover:bg-surface grid size-8 place-items-center rounded-full"
        >
          <Plus aria-hidden size={14} />
        </button>
        {offset !== 0 && (
          <button
            type="button"
            onClick={() => setOffset(0)}
            className="text-muted hover:text-foreground text-sm"
          >
            {t('lyricsEdit.resetOffset')}
          </button>
        )}
      </div>
      <p className="text-muted text-xs">
        {playing ? t('lyricsEdit.syncHint') : t('lyricsEdit.offsetHint')}
      </p>
      <ol className="flex max-h-[40vh] flex-col overflow-y-auto">
        {synced.map((line, index) => (
          <li key={`${line.timeMs}-${index}`} className="flex items-center gap-2 py-0.5">
            <button
              type="button"
              onClick={() => void sendControl({ type: 'seek', positionMs: line.timeMs + offset })}
              className="text-muted w-16 shrink-0 text-left text-xs tabular-nums"
            >
              {formatTrim(line.timeMs)}
            </button>
            <span
              aria-current={index === current ? 'true' : undefined}
              className={`min-w-0 flex-1 truncate text-sm ${index === current ? 'text-primary font-semibold' : ''}`}
            >
              {line.text || '♪'}
            </span>
            {playing && (
              <button
                type="button"
                aria-label={t('lyricsEdit.syncHere', { line: line.text || '♪' })}
                onClick={() => setOffset(Math.round(position - line.timeMs))}
                className="text-muted hover:text-foreground shrink-0 rounded-full px-2 py-1 text-xs"
              >
                {t('lyricsEdit.now')}
              </button>
            )}
          </li>
        ))}
      </ol>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={startEditing}
          className="bg-background hover:bg-surface rounded-full px-4 py-2 text-sm"
        >
          {t('lyricsEdit.editLines')}
        </button>
        <button
          type="button"
          onClick={() => setPlainDraft(plain || synced.map((line) => line.text).join('\n'))}
          className="bg-background hover:bg-surface rounded-full px-4 py-2 text-sm"
        >
          {t('lyricsEdit.editText')}
        </button>
      </div>
      <LyricsSources song={song} edit={edit} save={save} />
      <SourceLink
        edited={lyrics.data?.edited ?? false}
        onUndo={() => save({ ...edit, lyrics: null })}
      />
    </div>
  );
};

/** Where the lyrics come from, and the way back to the source's. */
const SourceLink: FC<{ edited: boolean; onUndo: () => void }> = ({ edited, onUndo }) => {
  const { t } = useTranslation();
  if (!edited) return null;
  return (
    <button
      type="button"
      onClick={onUndo}
      className="text-muted hover:text-foreground self-start text-sm underline"
    >
      {t('lyricsEdit.useSource')}
    </button>
  );
};
