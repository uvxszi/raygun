import { commands } from '@raygun/bindings';
import type { SourceQueueAdded } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import type { TFunction } from 'i18next';
import { ListPlus, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { LocalNetworkSheet } from '../network/LocalNetworkSheet';
import { LinkOrFileLines } from '../shell/LinkOrFileField';
import { SourceIssue } from '../shell/SourceIssue';
import { SourceQueueCard } from './SourceQueueCard';
import { useSourceQueue, withLines } from './sourceQueue';

/** "3 addresses queued · 1 was already in the list". */
const queuedNote = (t: TFunction, added: SourceQueueAdded): string =>
  [
    added.queued > 0
      ? t('sources.add.several.queued', { count: added.queued })
      : t('sources.add.several.nothingNew'),
    added.duplicates > 0 && t('sources.add.several.duplicates', { count: added.duplicates }),
    added.overLimit > 0 && t('sources.add.several.overLimit', { count: added.overLimit }),
  ]
    .filter(Boolean)
    .join(' · ');

/**
 * Sources → Add several: addresses one a line, typed, pasted or read from a text file chosen
 * beside the field. Add all queues them in Rust, which looks at and adds them one after
 * another; the list under the field follows it, and it carries on when the sheet closes (the
 * Sources page shows it too). A list with an address on the home network asks for Android 17's
 * local network access first; without it the others are still added.
 */
export const AddSeveral: FC<{ text: string; onTextChange: (text: string) => void }> = ({
  text,
  onTextChange,
}) => {
  const { t } = useTranslation();
  const list = useSourceQueue();
  const [asking, setAsking] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const add = useMutation({
    mutationFn: list.add,
    onSuccess: () => onTextChange(''),
  });
  const read = useMutation({
    mutationFn: (path: string) => unwrapResult(commands.sourcesQueueReadFile(path)),
    onSuccess: (addresses) => onTextChange(withLines(text, addresses)),
  });
  const busy = add.isPending || checking;

  const addAll = async () => {
    const wanted = text;
    if (!wanted.trim()) return;
    add.reset();
    setChecking(true);
    try {
      const local = await unwrapResult(commands.sourcesQueueLocalAddress(wanted)).catch(() => null);
      if (local) setAsking(wanted);
      else add.mutate(wanted);
    } finally {
      setChecking(false);
    }
  };
  // Allowed or not, the list goes ahead: without the access, addresses on the home network say so.
  const goAhead = () => {
    if (asking !== null) add.mutate(asking);
    setAsking(null);
  };

  return (
    <div className="flex flex-col gap-3 text-sm">
      <LinkOrFileLines
        value={text}
        onChange={onTextChange}
        onPicked={(path) => read.mutate(path)}
        rows={6}
        autoFocus
        placeholder={t('sources.add.several.placeholder')}
        aria-label={t('sources.add.several.label')}
      />
      <p className="text-muted text-xs">{t('sources.add.several.hint')}</p>
      {read.error && <SourceIssue compact>{read.error.message}</SourceIssue>}
      {add.data && (
        <p role="status" className="text-muted text-xs">
          {queuedNote(t, add.data)}
        </p>
      )}
      <div className="flex justify-end">
        <button
          type="button"
          disabled={!text.trim() || busy}
          onClick={() => void addAll()}
          className="bg-primary text-background flex items-center gap-1.5 rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
        >
          {busy ? (
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
          ) : (
            <ListPlus aria-hidden size={16} />
          )}
          {t('sources.add.several.addAll')}
        </button>
      </div>
      <SourceQueueCard />
      {asking !== null && (
        <LocalNetworkSheet reason="source" onGranted={goAhead} onClose={goAhead} />
      )}
    </div>
  );
};
