import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { READERS, readerName, useImportReaders } from './importReaders';

/**
 * "Try another reader" in the review: every other reader. One not on this device yet is
 * downloaded first (its size shown), then every screenshot is read again with it, all without
 * leaving the review.
 */
export const ReaderPicker: FC<{
  current: string | null;
  busy: boolean;
  onPick: (reader: string) => void;
}> = ({ current, busy, onPick }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const readers = useImportReaders();
  const others = (readers.data ?? []).filter((reader) => reader.id !== current);
  const [chosen, setChosen] = useState('');
  const pick = others.find((reader) => reader.id === chosen) ?? others[0];
  const prepare = useMutation({
    mutationFn: (id: string) => unwrapResult(commands.importPrepareReader(id)),
    onSuccess: (_, id) => {
      void client.invalidateQueries({ queryKey: READERS });
      onPick(id);
    },
  });
  if (others.length === 0) return null;
  const working = busy || prepare.isPending;
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2 text-sm">
        <label className="flex min-w-0 flex-1 items-center gap-2">
          <span className="text-muted shrink-0">{t('import.tryAnother')}</span>
          <select
            value={pick?.id ?? ''}
            disabled={working}
            onChange={(event) => setChosen(event.target.value)}
            className="bg-background min-w-0 flex-1 rounded-xl px-3 py-2 text-sm outline-none"
          >
            {others.map((reader) => (
              <option key={reader.id} value={reader.id}>
                {reader.ready
                  ? readerName(reader, t)
                  : t('import.readerToDownload', {
                      name: readerName(reader, t),
                      size: Math.round((reader.sizeBytes ?? 0) / 1_000_000),
                    })}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          disabled={working || !pick}
          onClick={() => {
            if (!pick) return;
            if (pick.ready) onPick(pick.id);
            else prepare.mutate(pick.id);
          }}
          className="bg-background flex shrink-0 items-center gap-1.5 rounded-full px-4 py-2 font-semibold disabled:opacity-60"
        >
          {working && <LoaderCircle aria-hidden size={14} className="animate-spin" />}
          {t(
            prepare.isPending
              ? 'import.downloadingReader'
              : busy
                ? 'import.reading'
                : pick && !pick.ready
                  ? 'import.downloadAndRead'
                  : 'import.readAgain',
          )}
        </button>
      </div>
      {prepare.error && <p className="text-muted text-xs">{prepare.error.message}</p>}
    </div>
  );
};
