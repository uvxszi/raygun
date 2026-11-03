import type { TrackQuery } from '@raygun/bindings';
import { RotateCcw, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useLivePosition } from '../player/useLivePosition';
import { useNowPlaying } from '../player/usePlayback';
import { Sheet } from '../shell/Sheet';
import { formatDb, formatTrim, parseTrim } from './format';
import { LyricsEditor } from './LyricsEditor';
import { useSongEditStore } from './songEditStore';
import { DetailEditForm } from '../music/DetailEditForm';
import type { SongEditTab } from './songEditStore';
import type { FullSongEdit } from './soundPrefs';
import { useSongEdit } from './useSound';

const sameSong = (left: TrackQuery, right: TrackQuery) =>
  left.title === right.title && (left.artist ?? '') === (right.artist ?? '');

/** A trim: typed as a time, taken from the playing position, or cleared. */
const TrimField: FC<{
  label: string;
  value: number | null;
  placeholder: string;
  now: number | null;
  onChange: (value: number | null) => void;
}> = ({ label, value, placeholder, now, onChange }) => {
  const { t } = useTranslation();
  const shown = value === null ? '' : formatTrim(value);
  const [text, setText] = useState(shown);
  // A saved or cleared trim replaces what's typed.
  const [textFor, setTextFor] = useState(value);
  if (textFor !== value) {
    setTextFor(value);
    setText(shown);
  }
  const parsed = text.trim() === '' ? null : parseTrim(text);
  const invalid = text.trim() !== '' && parsed === null;
  const commit = () => {
    if (!invalid && parsed !== value) onChange(parsed);
  };
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-sm font-medium">{label}</span>
      <div className="flex items-center gap-2">
        <input
          value={text}
          onChange={(event) => setText(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === 'Enter') commit();
          }}
          placeholder={placeholder}
          aria-label={label}
          aria-invalid={invalid}
          inputMode="decimal"
          className="bg-background placeholder:text-muted w-28 rounded-xl px-3 py-2 tabular-nums outline-none aria-invalid:ring-1 aria-invalid:ring-red-500"
        />
        {now !== null && (
          <button
            type="button"
            onClick={() => onChange(Math.round(now))}
            className="bg-background hover:bg-surface rounded-full px-3 py-2 text-sm"
          >
            {t('songEdit.setNow', { time: formatTrim(now) })}
          </button>
        )}
        {value !== null && (
          <button
            type="button"
            aria-label={t('songEdit.clear', { name: label })}
            onClick={() => onChange(null)}
            className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </div>
    </div>
  );
};

const MOVE_LYRICS_KEY = 'raygun.songEdit.moveLyrics';

const readMoveLyrics = () => {
  try {
    return localStorage.getItem(MOVE_LYRICS_KEY) !== '0';
  } catch {
    return true;
  }
};

/** The sound half of the editor: trims and the song's own volume. */
const SoundTab: FC<{
  song: TrackQuery;
  edit: FullSongEdit;
  save: (edit: FullSongEdit) => void;
}> = ({ song, edit, save }) => {
  const { t } = useTranslation();
  const { state, item } = useNowPlaying();
  const position = useLivePosition(state);
  const playing =
    item !== undefined &&
    sameSong(song, { title: item.title, artist: item.artist, album: null, durationMs: null });
  const [gain, setGain] = useState(edit.gain);
  const [gainFor, setGainFor] = useState(edit.gain);
  if (gainFor !== edit.gain) {
    setGainFor(edit.gain);
    setGain(edit.gain);
  }
  const endTooEarly = edit.endMs !== null && edit.endMs <= edit.startMs;
  // The lyrics move with the start (M46.36): a video's cut intro takes its seconds off the
  // lyrics' timing too. Remembered on this device.
  const [moveLyrics, setMoveLyrics] = useState(readMoveLyrics);
  const setStart = (startMs: number) =>
    save({
      ...edit,
      startMs,
      lyricsOffsetMs: moveLyrics
        ? edit.lyricsOffsetMs + (startMs - edit.startMs)
        : edit.lyricsOffsetMs,
    });
  return (
    <div className="flex flex-col gap-5">
      <p className="text-muted text-sm">{t('songEdit.trimHint')}</p>
      <div className="flex flex-wrap gap-6">
        <TrimField
          label={t('songEdit.start')}
          value={edit.startMs > 0 ? edit.startMs : null}
          placeholder="0:00.0"
          now={playing ? position : null}
          onChange={(startMs) => setStart(startMs ?? 0)}
        />
        <TrimField
          label={t('songEdit.end')}
          value={edit.endMs}
          placeholder={t('songEdit.toTheEnd')}
          now={playing ? position : null}
          onChange={(endMs) => save({ ...edit, endMs })}
        />
      </div>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={moveLyrics}
          onChange={(event) => {
            setMoveLyrics(event.target.checked);
            try {
              localStorage.setItem(MOVE_LYRICS_KEY, event.target.checked ? '1' : '0');
            } catch {
              // A per-device convenience.
            }
          }}
          className="accent-primary size-4"
        />
        {t('songEdit.moveLyrics')}
      </label>
      {endTooEarly && (
        <p role="alert" className="text-primary text-sm">
          {t('songEdit.endBeforeStart')}
        </p>
      )}
      <label className="flex flex-col gap-1 text-sm">
        <span className="flex justify-between">
          <span className="font-medium">{t('songEdit.volume')}</span>
          <span className="text-primary font-semibold tabular-nums">{formatDb(gain)}</span>
        </span>
        <input
          type="range"
          aria-label={t('songEdit.volume')}
          aria-valuetext={formatDb(gain)}
          min={-120}
          max={120}
          step={5}
          value={gain}
          onChange={(event) => setGain(Number(event.target.value))}
          onPointerUp={() => gain !== edit.gain && save({ ...edit, gain })}
          onKeyUp={() => gain !== edit.gain && save({ ...edit, gain })}
          className="accent-primary w-full"
        />
        <span className="text-muted text-xs">{t('songEdit.volumeHint')}</span>
      </label>
    </div>
  );
};

/** A song's own sound and lyrics (M46.12), opened from its menu or the player. */
export const SongEditSheet: FC = () => {
  const { t } = useTranslation();
  const song = useSongEditStore((store) => store.song);
  const anchor = useSongEditStore((store) => store.anchor);
  const close = useSongEditStore((store) => store.close);
  if (!song) return null;
  return (
    <Sheet
      title={t('songEdit.title', { title: song.title })}
      anchor={anchor}
      popoverWidth={420}
      onClose={close}
    >
      <SongEditBody key={`${song.artist ?? ''}|${song.title}`} song={song} />
    </Sheet>
  );
};

const TABS: SongEditTab[] = ['sound', 'lyrics', 'details'];

const SongEditBody: FC<{ song: TrackQuery }> = ({ song }) => {
  const { t } = useTranslation();
  const { edit, save } = useSongEdit(song);
  const tab = useSongEditStore((store) => store.tab);
  if (!edit.data) {
    return <p className="text-muted text-sm">{t('songEdit.loading')}</p>;
  }
  const current = edit.data;
  const changed =
    current.startMs > 0 ||
    current.endMs !== null ||
    current.gain !== 0 ||
    current.lyricsOffsetMs !== 0 ||
    current.lyrics !== null;
  return (
    <div data-testid="song-edit" className="flex flex-col gap-5">
      <div role="tablist" aria-label={t('songEdit.parts')} className="flex gap-2">
        {TABS.map((each) => (
          <button
            key={each}
            type="button"
            role="tab"
            aria-selected={tab === each}
            onClick={() => useSongEditStore.setState({ tab: each })}
            className={`rounded-full px-4 py-1.5 text-sm ${
              tab === each
                ? 'bg-primary text-background font-semibold'
                : 'bg-background hover:bg-surface'
            }`}
          >
            {t(`songEdit.tab.${each}`)}
          </button>
        ))}
      </div>
      {tab === 'sound' && (
        <SoundTab song={song} edit={current} save={(next) => save.mutate(next)} />
      )}
      {tab === 'lyrics' && (
        <LyricsEditor song={song} edit={current} save={(next) => save.mutate(next)} />
      )}
      {/* Title, artist, album and cover over what the source said (M46.30). */}
      {tab === 'details' && (
        <DetailEditForm
          target={{ kind: 'song', query: song }}
          fields={['title', 'artist', 'album']}
          original={{ title: song.title, artist: song.artist, album: song.album }}
          titleLabel={t('details.songTitle')}
        />
      )}
      {save.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {String(save.error.message ?? save.error)}
        </p>
      )}
      <button
        type="button"
        disabled={!changed || save.isPending}
        onClick={() =>
          save.mutate({ startMs: 0, endMs: null, gain: 0, lyricsOffsetMs: 0, lyrics: null })
        }
        className="text-muted hover:text-foreground flex items-center gap-2 self-start text-sm disabled:opacity-40"
      >
        <RotateCcw aria-hidden size={14} />
        {t('songEdit.reset')}
      </button>
    </div>
  );
};
