import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { readerName, useImportReaders } from './importReaders';

/**
 * "Try another reader" in the review: the other readers that are ready here; reading with one
 * reads every screenshot again.
 */
export const ReaderPicker: FC<{
  current: string | null;
  busy: boolean;
  onPick: (reader: string) => void;
}> = ({ current, busy, onPick }) => {
  const { t } = useTranslation();
  const readers = useImportReaders();
  const others = (readers.data ?? []).filter((reader) => reader.ready && reader.id !== current);
  const [chosen, setChosen] = useState('');
  const pick = others.find((reader) => reader.id === chosen) ?? others[0];
  if (others.length === 0) return null;
  return (
    <div className="flex items-center gap-2 text-sm">
      <label className="flex min-w-0 flex-1 items-center gap-2">
        <span className="text-muted shrink-0">{t('import.tryAnother')}</span>
        <select
          value={pick?.id ?? ''}
          disabled={busy}
          onChange={(event) => setChosen(event.target.value)}
          className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2 text-sm outline-none"
        >
          {others.map((reader) => (
            <option key={reader.id} value={reader.id}>
              {readerName(reader, t)}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        disabled={busy || !pick}
        onClick={() => pick && onPick(pick.id)}
        className="bg-background flex shrink-0 items-center gap-1.5 rounded-full px-4 py-2 font-semibold disabled:opacity-60"
      >
        {busy && <LoaderCircle aria-hidden size={14} className="animate-spin" />}
        {t(busy ? 'import.reading' : 'import.readAgain')}
      </button>
    </div>
  );
};
