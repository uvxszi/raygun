import { commands } from '@raygun/bindings';
import type { BackupOutcome } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArchiveRestore, FileUp, Plus, Trash2 } from 'lucide-react';
import { useRef, useState } from 'react';
import type { ChangeEvent, FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { readAsBase64 } from '../lib/readFile';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';

const BACKUPS = ['backups'];

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

/** What's being asked about a backup: restoring it, or deleting it. */
type Asking = { name: string; action: 'restore' | 'delete' };

/** Settings → Data → Backup: create, restore and delete backups, and restore a picked file. */
export const BackupCard: FC = () => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const fileInput = useRef<HTMLInputElement>(null);
  const [asking, setAsking] = useState<Asking | null>(null);
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
  const formatDate = (ms: number) =>
    new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' }).format(ms);
  // Each failure says what it was about; the error itself is under More info.
  const failure = create.error
    ? { title: t('settings.backupCreateFailed'), detail: create.error.message }
    : restore.error
      ? { title: t('settings.backupRestoreFailed'), detail: restore.error.message }
      : importFile.error
        ? { title: t('settings.backupImportFailed'), detail: importFile.error.message }
        : remove.error
          ? { title: t('settings.backupDeleteFailed'), detail: remove.error.message }
          : backups.error
            ? { title: t('settings.backupListFailed'), detail: backups.error.message }
            : null;

  const onPick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file) {
      setOutcome(null);
      importFile.mutate(file);
    }
  };

  return (
    <div data-testid="backup-card">
      <PrefGroup title={t('settings.backup')}>
        <PrefAction
          icon={Plus}
          title={t('settings.createBackup')}
          summary={
            create.data && !create.isPending && !outcome ? (
              <span role="status">{t('settings.backupCreated', { name: create.data.name })}</span>
            ) : undefined
          }
          disabled={busy}
          onClick={() => {
            setOutcome(null);
            create.mutate();
          }}
        />
        <PrefAction
          icon={FileUp}
          title={t('settings.importBackup')}
          summary={importFile.isPending ? t('settings.importing') : undefined}
          disabled={busy}
          onClick={() => fileInput.current?.click()}
        />
        <li className="hidden">
          <input
            ref={fileInput}
            type="file"
            aria-label={t('settings.backupFile')}
            onChange={onPick}
          />
        </li>
        {outcome && (
          <li className="px-1 py-2 text-sm">
            <Outcome outcome={outcome} />
          </li>
        )}
        {failure && (
          <li className="px-1 py-2">
            <Problem title={failure.title} detail={failure.detail} />
          </li>
        )}
        {backups.data?.map((file) => {
          const date = formatDate(file.createdAt ?? 0);
          const asked = asking?.name === file.name ? asking.action : null;
          return (
            <li
              key={file.name}
              data-testid="backup-row"
              className="flex items-center gap-2 px-1 py-2 text-sm"
            >
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate font-medium">{date}</span>
                <span className="text-muted text-xs">
                  {t('settings.backupSize', { size: ((file.sizeBytes ?? 0) / 1024).toFixed(0) })}
                </span>
              </span>
              {asked ? (
                <>
                  <button
                    type="button"
                    onClick={() => setAsking(null)}
                    className="hover:bg-surface rounded-full px-3 py-1.5 text-xs font-semibold"
                  >
                    {t('settings.cancel')}
                  </button>
                  <button
                    type="button"
                    disabled={busy || remove.isPending}
                    onClick={() => {
                      setAsking(null);
                      if (asked === 'restore') restore.mutate(file.name);
                      else remove.mutate(file.name);
                    }}
                    className="bg-primary text-background rounded-full px-3 py-1.5 text-xs font-semibold"
                  >
                    {asked === 'restore'
                      ? t('settings.confirmRestore')
                      : t('settings.confirmDelete')}
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    aria-label={t('settings.restoreBackup', { date })}
                    disabled={busy}
                    onClick={() => setAsking({ name: file.name, action: 'restore' })}
                    className="text-primary hover:bg-surface grid size-10 place-items-center rounded-full"
                  >
                    <ArchiveRestore aria-hidden size={18} />
                  </button>
                  <button
                    type="button"
                    aria-label={t('settings.deleteBackup', { date })}
                    disabled={busy || remove.isPending}
                    onClick={() => setAsking({ name: file.name, action: 'delete' })}
                    className="text-muted hover:bg-surface grid size-10 place-items-center rounded-full"
                  >
                    <Trash2 aria-hidden size={18} />
                  </button>
                </>
              )}
            </li>
          );
        })}
      </PrefGroup>
    </div>
  );
};
