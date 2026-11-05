import { commands } from '@raygun/bindings';
import type { BackupEntry, DataCategory, DataUsage } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { FolderPlus, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { BACKUP_CATEGORIES, DATA_USAGE, DESTINATIONS, countsLine, folderLabel } from './dataLabels';
import { Problem } from './Problem';

const option = (checked: boolean) =>
  `flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm ${checked ? 'bg-primary/15' : 'bg-background'}`;

/** `null` saves to the usual place. */
type Where = string | null;

/**
 * Create backup (M46.51): what to keep (every category at first, each with what it holds here),
 * passwords and keys only when asked, and where: the usual place, a folder picked before, or a
 * new one from the system's picker. The place used last is chosen at first.
 */
export const CreateBackupSheet: FC<{
  onClose: () => void;
  onCreated: (entry: BackupEntry) => void;
}> = ({ onClose, onCreated }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const destinations = useQuery({
    queryKey: DESTINATIONS,
    queryFn: () => unwrapResult(commands.backupDestinations()),
  });
  const usage = useQuery({
    queryKey: DATA_USAGE,
    queryFn: () => unwrapResult(commands.dataUsage()),
  });
  const [chosen, setChosen] = useState<DataCategory[]>(BACKUP_CATEGORIES);
  const [secrets, setSecrets] = useState(false);
  // Until picked here, the place used last.
  const [where, setWhere] = useState<Where | undefined>(undefined);
  const shownWhere: Where = where === undefined ? (destinations.data?.last ?? null) : where;

  const pick = useMutation({
    mutationFn: () => unwrapResult(commands.backupPickFolder()),
    onSuccess: (picked) => {
      if (!picked) return;
      queryClient.setQueryData(DESTINATIONS, picked);
      setWhere(picked.last ?? null);
    },
  });
  const forget = useMutation({
    mutationFn: (uri: string) => unwrapResult(commands.backupForgetFolder(uri)),
    onSuccess: (left, uri) => {
      queryClient.setQueryData(DESTINATIONS, left);
      if (shownWhere === uri) setWhere(null);
    },
  });
  const create = useMutation({
    mutationFn: () =>
      unwrapResult(
        commands.backupCreate({
          categories: BACKUP_CATEGORIES.filter((category) => chosen.includes(category)),
          secrets,
          folder: shownWhere,
        }),
      ),
    onSuccess: (entry) => {
      void queryClient.invalidateQueries({ queryKey: DESTINATIONS });
      onCreated(entry);
    },
  });

  const holds = (category: DataCategory): string => {
    const row = usage.data?.find((entry: DataUsage) => entry.category === category);
    return row ? countsLine(t, row.counts) : '';
  };
  const toggle = (category: DataCategory) =>
    setChosen((now) =>
      now.includes(category) ? now.filter((kept) => kept !== category) : [...now, category],
    );
  const allChosen = chosen.length === BACKUP_CATEGORIES.length;
  const failure = create.error ?? pick.error ?? forget.error ?? destinations.error;

  return (
    <Sheet title={t('data.create.title')} onClose={onClose}>
      <div data-testid="create-backup" className="flex flex-col gap-4 pb-1">
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-muted flex w-full items-center justify-between pb-1 text-xs font-semibold uppercase">
            {t('data.create.what')}
          </legend>
          <button
            type="button"
            onClick={() => setChosen(allChosen ? [] : BACKUP_CATEGORIES)}
            className="text-primary self-end rounded-full px-3 py-1 text-xs font-semibold"
          >
            {allChosen ? t('data.create.none') : t('data.create.all')}
          </button>
          {BACKUP_CATEGORIES.map((category) => {
            const line = holds(category);
            return (
              <label key={category} className={option(chosen.includes(category))}>
                <input
                  type="checkbox"
                  checked={chosen.includes(category)}
                  onChange={() => toggle(category)}
                  className="accent-primary size-4 shrink-0"
                />
                <span className="flex min-w-0 flex-col">
                  <span>{t(`data.categories.${category}`)}</span>
                  {line && <span className="text-muted truncate text-xs">{line}</span>}
                </span>
              </label>
            );
          })}
        </fieldset>
        <label className={option(secrets)}>
          <input
            type="checkbox"
            checked={secrets}
            onChange={() => setSecrets(!secrets)}
            className="accent-primary size-4 shrink-0"
          />
          <span className="flex flex-col">
            <span>{t('data.create.secrets')}</span>
            <span className="text-muted text-xs">{t('data.create.secretsHint')}</span>
          </span>
        </label>
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('data.create.where')}
          </legend>
          <label className={option(shownWhere === null)}>
            <input
              type="radio"
              name="backup-where"
              checked={shownWhere === null}
              onChange={() => setWhere(null)}
              className="accent-primary"
            />
            <span className="flex flex-col">
              <span>{t('data.create.usual')}</span>
              <span className="text-muted text-xs">
                {destinations.data
                  ? folderLabel(t, destinations.data.usualPlace, destinations.data.usual)
                  : ''}
              </span>
            </span>
          </label>
          {destinations.data?.folders.map((folder) => (
            <div key={folder.uri} className={option(shownWhere === folder.uri)}>
              <label className="flex min-w-0 flex-1 items-center gap-3">
                <input
                  type="radio"
                  name="backup-where"
                  checked={shownWhere === folder.uri}
                  onChange={() => setWhere(folder.uri)}
                  className="accent-primary"
                />
                <span className="truncate">{folder.name}</span>
              </label>
              <button
                type="button"
                aria-label={t('data.create.forget', { name: folder.name })}
                disabled={forget.isPending}
                onClick={() => forget.mutate(folder.uri)}
                className="text-muted hover:bg-surface grid size-8 shrink-0 place-items-center rounded-full"
              >
                <X aria-hidden size={16} />
              </button>
            </div>
          ))}
          <button
            type="button"
            disabled={pick.isPending}
            onClick={() => pick.mutate()}
            className="text-primary flex items-center gap-2 self-start rounded-full px-3 py-2 text-sm font-semibold"
          >
            <FolderPlus aria-hidden size={18} />
            {t('data.create.choose')}
          </button>
        </fieldset>
        {failure && (
          <Problem
            title={create.error ? t('settings.backupCreateFailed') : t('data.create.placeFailed')}
            detail={failure.message}
          />
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="hover:bg-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('settings.cancel')}
          </button>
          <button
            type="button"
            disabled={chosen.length === 0 || create.isPending}
            onClick={() => create.mutate()}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {create.isPending ? t('data.create.saving') : t('data.create.save')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
