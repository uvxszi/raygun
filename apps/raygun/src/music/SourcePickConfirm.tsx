import { commands } from '@raygun/bindings';
import type { OnlineTrack, TrackInput } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { unwrapResult } from '../lib/commandResult';
import { differences } from './sourcePick';
import type { DetailField } from './sourcePick';

type SourcePickConfirmProps = {
  song: TrackInput;
  track: OnlineTrack;
  /** The source's name, when known. */
  sourceName: string | undefined;
  confirmLabel: string;
  busy: boolean;
  /** Only the details are taken (the source Raygun found itself): the boxes are on, no switch. */
  detailsOnly?: boolean;
  /** The details to take from the source (none when the switch is off). */
  onConfirm: (ticked: ReadonlySet<DetailField>) => void;
  onBack: () => void;
};

/**
 * Confirming a picked source whose details differ from the song's (M46.48): both side by side,
 * and "Use these details" with a box per detail. The switch starts on for songs that came in
 * through Import songs or that no source had; Edit details reverts what it changes.
 */
export const SourcePickConfirm: FC<SourcePickConfirmProps> = ({
  song,
  track,
  sourceName,
  confirmLabel,
  busy,
  detailsOnly = false,
  onConfirm,
  onBack,
}) => {
  const { t } = useTranslation();
  const status = useQuery({
    queryKey: ['trackImportStatus', song.artist ?? '', song.title],
    queryFn: () => unwrapResult(commands.trackImportStatus(song)),
  });
  const offered = differences(song, track);
  const [chosen, setChosen] = useState<boolean | null>(null);
  const using =
    detailsOnly ||
    (chosen ?? (status.data ? status.data.imported || status.data.unavailable : false));
  const [unticked, setUnticked] = useState<ReadonlySet<DetailField>>(new Set());
  const ticked = new Set(
    offered.map((difference) => difference.field).filter((field) => !unticked.has(field)),
  );
  const toggle = (field: DetailField) =>
    setUnticked((current) => {
      const next = new Set(current);
      if (next.has(field)) next.delete(field);
      else next.add(field);
      return next;
    });

  return (
    <div data-testid="source-pick-confirm" className="flex flex-col gap-3">
      <div className="grid grid-cols-[auto_minmax(0,1fr)_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-sm">
        <span />
        <span className="text-muted text-xs font-semibold">{t('sourcePicker.thisSong')}</span>
        <span className="text-muted truncate text-xs font-semibold">
          {sourceName
            ? t('sourcePicker.fromSource', { source: sourceName })
            : t('sourcePicker.fromPick')}
        </span>
        {offered.map((difference) => {
          const label = t(`sourcePicker.fields.${difference.field}`);
          return (
            <label key={difference.field} className="contents">
              <span className="flex items-center gap-2">
                <input
                  type="checkbox"
                  aria-label={label}
                  disabled={!using}
                  checked={using && ticked.has(difference.field)}
                  onChange={() => toggle(difference.field)}
                  className="accent-primary"
                />
                <span className="text-muted text-xs">{label}</span>
              </span>
              {difference.field === 'cover' ? (
                <>
                  <Artwork url={difference.song} className="size-10 rounded" iconSize={16} />
                  <Artwork url={difference.source} className="size-10 rounded" iconSize={16} />
                </>
              ) : (
                <>
                  <span className="text-muted truncate">{difference.song ?? '—'}</span>
                  <span className="truncate font-medium">{difference.source}</span>
                </>
              )}
            </label>
          );
        })}
      </div>
      {!detailsOnly && (
        <button
          type="button"
          role="switch"
          aria-checked={using}
          onClick={() => setChosen(!using)}
          className="flex items-center justify-between gap-3 text-left text-sm"
        >
          {t('sourcePicker.useDetails')}
          <span
            aria-hidden
            className={`flex h-5 w-9 items-center rounded-full p-0.5 transition ${using ? 'bg-primary justify-end' : 'bg-background justify-start'}`}
          >
            <span className="bg-foreground size-4 rounded-full" />
          </span>
        </button>
      )}
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onBack}
          className="bg-background rounded-full px-4 py-2 text-sm"
        >
          {t('sourcePicker.back')}
        </button>
        <button
          type="button"
          disabled={busy || (detailsOnly && ticked.size === 0)}
          onClick={() => onConfirm(using ? ticked : new Set())}
          className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  );
};
