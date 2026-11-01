import { commands } from '@raygun/bindings';
import type { BackupOutcome } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArchiveRestore, FileUp, Plus, Trash2 } from 'lucide-react';
import { useRef, useState } from 'react';
import type { ChangeEvent, FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

const BACKUPS = ['backups'];

/** A picked file's contents as base64 (without the data URL prefix). */
const readAsBase64 = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result);
      resolve(url.slice(url.indexOf(',') + 1));
    };
    reader.onerror = () => reject(reader.error ?? new Error('read failed'));
    reader.readAsDataURL(file);
  });

const Outcome: FC<{ outcome: BackupOutcome }> = ({ outcome }) => {
  const { t } = useTranslation();
  if (outcome.kind === 'raygun') {
    return <p role="status">{t('settings.backupRestored', { count: outcome.changed })}</p>;
  }
  const { summary } = outcome;
  return (
    <div role="status" className="flex flex-col gap-1">
      <p>
        {t('settings.mihonImported', {
          count: summary.titles,
          chapters: summary.chapters,
          read: summary.readChapters,
        })}
      </p>
      {summary.skipped.length > 0 && (
        <p className="text-muted text-xs">
          {t('settings.mihonSkipped', {
            sources: summary.skipped
              .map((source) => `${source.name} (${source.titles})`)
              .join(', '),
          })}
        </p>
      )}
    </div>
  );
};

/** Settings → Backup: create, restore and delete backups, and import a picked file. */
export const BackupCard: FC = () => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const fileInput = useRef<HTMLInputElement>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<BackupOutcome | null>(null);
  const backups = useQuery({
    queryKey: BACKUPS,
    queryFn: () => unwrapResult(commands.backupList()),
  });
  const refreshAll = () => queryClient.invalidateQueries();
  const create = useMutation({
    mutationFn: () => unwrapResult(commands.backupCreate()),
    onSettled: () => queryClient.invalidateQueries({ queryKey: BACKUPS }),
  });
  const remove = useMutation({
    mutationFn: (name: string) => unwrapResult(commands.backupDelete(name)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: BACKUPS }),
  });
  const restore = useMutation({
    mutationFn: (name: string) => unwrapResult(commands.backupRestore(name)),
    onSuccess: (result) => {
      setOutcome(result);
      setConfirming(null);
      void refreshAll();
    },
  });
  const importFile = useMutation({
    mutationFn: async (file: File) => unwrapResult(commands.backupImport(await readAsBase64(file))),
    onSuccess: (result) => {
      setOutcome(result);
      void refreshAll();
    },
  });
  const busy = create.isPending || restore.isPending || importFile.isPending;
  const error = create.error ?? restore.error ?? importFile.error ?? remove.error;
  const formatDate = (ms: number) =>
    new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' }).format(ms);

  const onPick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file) {
      setOutcome(null);
      importFile.mutate(file);
    }
  };

  return (
    <section
      data-testid="backup-card"
      className="bg-surface flex w-full flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.backup')}</h2>
      <p className="text-muted text-xs">{t('settings.backupHint')}</p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => {
            setOutcome(null);
            create.mutate();
          }}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-4 py-2 font-semibold disabled:opacity-60"
        >
          <Plus aria-hidden size={16} />
          {t('settings.createBackup')}
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => fileInput.current?.click()}
          className="bg-background flex items-center gap-2 rounded-full px-4 py-2 disabled:opacity-60"
        >
          <FileUp aria-hidden size={16} />
          {t('settings.importBackup')}
        </button>
        <input
          ref={fileInput}
          type="file"
          aria-label={t('settings.backupFile')}
          onChange={onPick}
          className="hidden"
        />
      </div>
      {importFile.isPending && <p role="status">{t('settings.importing')}</p>}
      {create.data && !create.isPending && !outcome && (
        <p role="status">{t('settings.backupCreated', { name: create.data.name })}</p>
      )}
      {outcome && <Outcome outcome={outcome} />}
      {error && (
        <p role="alert" className="text-primary wrap-anywhere">
          {error.message}
        </p>
      )}
      {backups.data && backups.data.length > 0 && (
        <ul className="flex flex-col gap-1">
          {backups.data.map((file) => (
            <li key={file.name} data-testid="backup-row" className="flex items-center gap-2 py-1">
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{formatDate(file.createdAt ?? 0)}</span>
                <span className="text-muted text-xs">
                  {t('settings.backupSize', { size: ((file.sizeBytes ?? 0) / 1024).toFixed(0) })}
                </span>
              </span>
              {confirming === file.name ? (
                <>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => restore.mutate(file.name)}
                    className="bg-primary text-background rounded-full px-3 py-1.5 text-xs font-semibold"
                  >
                    {t('settings.confirmRestore')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setConfirming(null)}
                    className="rounded-full px-3 py-1.5 text-xs"
                  >
                    {t('settings.cancel')}
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    aria-label={t('settings.restoreBackup', {
                      date: formatDate(file.createdAt ?? 0),
                    })}
                    disabled={busy}
                    onClick={() => setConfirming(file.name)}
                    className="text-primary grid size-9 place-items-center rounded-full"
                  >
                    <ArchiveRestore aria-hidden size={18} />
                  </button>
                  <button
                    type="button"
                    aria-label={t('settings.deleteBackup', {
                      date: formatDate(file.createdAt ?? 0),
                    })}
                    disabled={busy || remove.isPending}
                    onClick={() => remove.mutate(file.name)}
                    className="text-muted grid size-9 place-items-center rounded-full"
                  >
                    <Trash2 aria-hidden size={18} />
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};
