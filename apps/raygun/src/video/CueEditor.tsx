import { Ear, Pencil, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTrim, parseTrim } from '../sound/format';
import type { CueEdit } from './cueEdits';
import type { Cue } from './subtitles';

/** Lines shown around the one playing. */
const AROUND = 4;

type CueEditorProps = {
  /** The track's original cues; edits are applied by the caller. */
  original: Cue[];
  edits: CueEdit[];
  timeMs: number;
  delayMs: number;
  /** The delay that makes a line show at the time it was heard (cloudstream's way). */
  onDelay: (delayMs: number) => void;
  onEdit: (at: number, edit: Omit<CueEdit, 'at'> | null) => void;
};

/**
 * The subtitle lines around the playing one (M46.12): "Heard now" shifts every line so this one
 * shows at once (cloudstream's subtitle sync), and a line's words and times can be changed. Edits
 * are kept for this episode and track.
 */
export const CueEditor: FC<CueEditorProps> = ({
  original,
  edits,
  timeMs,
  delayMs,
  onDelay,
  onEdit,
}) => {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<number | null>(null);
  const editsByStart = new Map(edits.map((edit) => [edit.at, edit]));
  const shown = original.map((cue) => {
    const edit = editsByStart.get(cue.startMs);
    return {
      at: cue.startMs,
      startMs: edit?.startMs ?? cue.startMs,
      endMs: edit?.endMs ?? cue.endMs,
      text: edit?.text ?? cue.lines.join('\n'),
      edited: edit !== undefined,
    };
  });
  // The line playing (or the next one), and a few on each side.
  const now = timeMs - delayMs;
  let middle = shown.findIndex((cue) => cue.endMs >= now);
  if (middle < 0) middle = shown.length - 1;
  const first = Math.max(0, middle - AROUND);
  const visible = shown.slice(first, middle + AROUND + 1);

  if (original.length === 0) {
    return <p className="px-3 py-1 text-xs text-white/60">{t('cueEdit.none')}</p>;
  }
  return (
    <ol data-testid="cue-editor" className="flex flex-col gap-1 px-2 pb-2">
      {visible.map((cue) => {
        const playing = cue.startMs <= now && now <= cue.endMs;
        if (editing === cue.at) {
          return (
            <li key={cue.at}>
              <CueForm
                cue={cue}
                onCancel={() => setEditing(null)}
                onSave={(edit) => {
                  onEdit(cue.at, edit);
                  setEditing(null);
                }}
              />
            </li>
          );
        }
        return (
          <li
            key={cue.at}
            aria-current={playing ? 'true' : undefined}
            className={`flex items-start gap-2 rounded-lg px-1 py-1 ${playing ? 'bg-white/15' : ''}`}
          >
            <span className="w-14 shrink-0 pt-0.5 text-2xs text-white/60 tabular-nums">
              {formatTrim(cue.startMs)}
            </span>
            <span className="min-w-0 flex-1 text-sm whitespace-pre-line">
              {cue.text}
              {cue.edited && <span className="ml-1 text-3xs text-white/50">✎</span>}
            </span>
            <button
              type="button"
              aria-label={t('cueEdit.heard', { line: cue.text })}
              title={t('cueEdit.heardHint')}
              onClick={() => onDelay(Math.round(timeMs - cue.startMs))}
              className="grid size-7 shrink-0 place-items-center rounded-full hover:bg-white/15"
            >
              <Ear aria-hidden size={14} />
            </button>
            <button
              type="button"
              aria-label={t('cueEdit.edit', { line: cue.text })}
              onClick={() => setEditing(cue.at)}
              className="grid size-7 shrink-0 place-items-center rounded-full hover:bg-white/15"
            >
              <Pencil aria-hidden size={13} />
            </button>
            {cue.edited && (
              <button
                type="button"
                aria-label={t('cueEdit.restore', { line: cue.text })}
                onClick={() => onEdit(cue.at, null)}
                className="grid size-7 shrink-0 place-items-center rounded-full hover:bg-white/15"
              >
                <RotateCcw aria-hidden size={13} />
              </button>
            )}
          </li>
        );
      })}
    </ol>
  );
};

/** A line's times and words being changed. */
const CueForm: FC<{
  cue: { startMs: number; endMs: number; text: string };
  onSave: (edit: Omit<CueEdit, 'at'>) => void;
  onCancel: () => void;
}> = ({ cue, onSave, onCancel }) => {
  const { t } = useTranslation();
  const [start, setStart] = useState(formatTrim(cue.startMs));
  const [end, setEnd] = useState(formatTrim(cue.endMs));
  const [text, setText] = useState(cue.text);
  const startMs = parseTrim(start);
  const endMs = parseTrim(end);
  const valid = startMs !== null && endMs !== null && endMs > startMs;
  const field = 'w-24 rounded-md bg-white/10 px-2 py-1 text-sm tabular-nums outline-none';
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (valid) onSave({ startMs, endMs, text });
      }}
      className="flex flex-col gap-2 rounded-lg bg-white/10 p-2"
    >
      <div className="flex gap-2">
        <input
          value={start}
          onChange={(event) => setStart(event.target.value)}
          aria-label={t('cueEdit.start')}
          className={field}
        />
        <input
          value={end}
          onChange={(event) => setEnd(event.target.value)}
          aria-label={t('cueEdit.end')}
          className={field}
        />
      </div>
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        aria-label={t('cueEdit.words')}
        rows={2}
        className="rounded-md bg-white/10 px-2 py-1 text-sm outline-none"
      />
      {!valid && <p className="text-xs text-red-300">{t('cueEdit.badTimes')}</p>}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!valid}
          className="rounded-full bg-white px-3 py-1 text-sm font-semibold text-black disabled:opacity-50"
        >
          {t('cueEdit.save')}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="rounded-full px-3 py-1 text-sm hover:bg-white/10"
        >
          {t('cueEdit.cancel')}
        </button>
      </div>
    </form>
  );
};
