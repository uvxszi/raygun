import { commands } from '@raygun/bindings';
import type { BackupEntry, BackupOutcome } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { FileUp, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useRef, useState } from 'react';
import type { ChangeEvent, FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { readAsBase64 } from '../lib/readFile';
import { CreateBackupSheet } from './CreateBackupSheet';
import { DATA_USAGE, folderLabel, madeBy, sortCategories } from './dataLabels';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';
import { RestoreSheet } from './RestoreSheet';

const BACKUPS = ['backups'];

const Outcome: FC<{ outcome: BackupOutcome }> = ({ outcome }) => {
  const { t } = useTranslation();
  if (outcome.kind === 'raygun') {
    return (
      <div role="status" className="flex flex-col gap-1">
        <p>{t('settings.backupRestored', { count: outcome.changed })}</p>
        {outcome.queued > 0 && <p>{t('data.restore.queued', { count: outcome.queued })}</p>}
        {outcome.problems.map((problem) => (
          <p key={problem} className="text-muted text-xs">
            {problem}
          </p>
        ))}
      </div>
    );
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

/**
 * Settings → Data → Backups (M46.51): create a backup of what's chosen where it's chosen, the
 * backups Raygun finds (its own folder, Download/Raygun, the folders picked) with who made each
 * and what it holds, opening one to restore from its index, and restoring a picked file (Raygun's
 * or Mihon's).
 */
export const BackupCard: FC = () => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const fileInput = useRef<HTMLInputElement>(null);
  const [creating, setCreating] = useState(false);
  const [created, setCreated] = useState<BackupEntry | null>(null);
  const [opened, setOpened] = useState<BackupEntry | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<BackupOutcome | null>(null);
  const backups = useQuery({
    queryKey: BACKUPS,
    queryFn: () => unwrapResult(commands.backupScan()),
  });
  const restored = (result: BackupOutcome) => {
    setOpened(null);
    setOutcome(result);
    // Every list may hold restored rows, as after a sync.
    void queryClient.invalidateQueries();
  };
  const remove = useMutation({
    mutationFn: (id: string) => unwrapResult(commands.backupDelete(id)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: BACKUPS }),
  });
  const importFile = useMutation({
    mutationFn: async (file: File) => unwrapResult(commands.backupImport(await readAsBase64(file))),
    onSuccess: restored,
  });
  const formatDate = (ms: number) =>
    new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' }).format(ms);
  const dateOf = (entry: BackupEntry) =>
    formatDate(entry.manifest?.createdAt ?? entry.modifiedAt ?? 0);
  const failure = importFile.error
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
      <PrefGroup title={t('data.backups.title')}>
        <PrefAction
          icon={Plus}
          title={t('settings.createBackup')}
          summary={
            created ? (
              <span role="status">
                {t('data.create.saved', {
                  name: created.name,
                  folder: folderLabel(t, created.place, created.folder),
                })}
              </span>
            ) : (
              t('data.create.summary')
            )
          }
          onClick={() => {
            setOutcome(null);
            setCreating(true);
          }}
        />
        <PrefAction
          icon={FileUp}
          title={t('settings.importBackup')}
          summary={importFile.isPending ? t('settings.importing') : undefined}
          disabled={importFile.isPending}
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
        <PrefAction
          icon={RefreshCw}
          title={t('data.backups.scan')}
          summary={backups.isFetching ? t('data.backups.scanning') : t('data.backups.scanHint')}
          disabled={backups.isFetching}
          onClick={() => void backups.refetch()}
        />
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
        {backups.data?.length === 0 && (
          <li className="text-muted px-1 py-2 text-sm">{t('data.backups.none')}</li>
        )}
        {backups.data?.map((entry) => {
          const date = dateOf(entry);
          const asking = deleting === entry.id;
          const manifest = entry.manifest;
          return (
            <li
              key={entry.id}
              data-testid="backup-row"
              className="flex items-center gap-2 px-1 py-2 text-sm"
            >
              <button
                type="button"
                aria-label={t('data.backups.open', { date })}
                disabled={!manifest}
                onClick={() => {
                  setOutcome(null);
                  setOpened(entry);
                }}
                className="hover:bg-surface flex min-w-0 flex-1 flex-col rounded-xl px-1 py-1 text-left"
              >
                <span className="truncate font-medium">{date}</span>
                <span className="text-muted truncate text-xs">
                  {manifest ? madeBy(t, manifest) : (entry.problem ?? t('data.backups.unreadable'))}
                </span>
                {manifest && (
                  <span className="text-muted truncate text-xs">
                    {sortCategories(manifest.sections)
                      .map((section) => t(`data.categories.${section.category}`))
                      .join(', ')}
                  </span>
                )}
                <span className="text-muted truncate text-xs">
                  {t('data.backups.where', {
                    size: formatBytes(entry.sizeBytes ?? 0),
                    folder: folderLabel(t, entry.place, entry.folder, entry.name),
                  })}
                </span>
              </button>
              {asking ? (
                <>
                  <button
                    type="button"
                    onClick={() => setDeleting(null)}
                    className="hover:bg-surface rounded-full px-3 py-1.5 text-xs font-semibold"
                  >
                    {t('settings.cancel')}
                  </button>
                  <button
                    type="button"
                    disabled={remove.isPending}
                    onClick={() => {
                      setDeleting(null);
                      remove.mutate(entry.id);
                    }}
                    className="bg-primary text-background rounded-full px-3 py-1.5 text-xs font-semibold"
                  >
                    {t('settings.confirmDelete')}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  aria-label={t('settings.deleteBackup', { date })}
                  disabled={remove.isPending}
                  onClick={() => setDeleting(entry.id)}
                  className="text-muted hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full"
                >
                  <Trash2 aria-hidden size={18} />
                </button>
              )}
            </li>
          );
        })}
      </PrefGroup>
      {creating && (
        <CreateBackupSheet
          onClose={() => setCreating(false)}
          onCreated={(entry) => {
            setCreating(false);
            setCreated(entry);
            void queryClient.invalidateQueries({ queryKey: BACKUPS });
            void queryClient.invalidateQueries({ queryKey: DATA_USAGE });
          }}
        />
      )}
      {opened?.manifest && (
        <RestoreSheet
          entry={opened}
          manifest={opened.manifest}
          date={dateOf(opened)}
          onClose={() => setOpened(null)}
          onRestored={restored}
        />
      )}
    </div>
  );
};
