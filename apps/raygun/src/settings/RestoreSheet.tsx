import { commands } from '@raygun/bindings';
import type { BackupEntry, BackupManifest, BackupOutcome, DataCategory } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { KeyRound } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { Sheet } from '../shell/Sheet';
import { countsLine, folderLabel, madeBy, sortCategories } from './dataLabels';
import { Problem } from './Problem';

const option = (checked: boolean) =>
  `flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm ${checked ? 'bg-primary/15' : 'bg-background'}`;

/**
 * A backup's index, shown before anything is restored (M46.51): who made it and when, then each
 * category it holds with what's in it, all chosen at first. Restoring merges the chosen ones in.
 */
export const RestoreSheet: FC<{
  entry: BackupEntry;
  manifest: BackupManifest;
  date: string;
  onClose: () => void;
  onRestored: (outcome: BackupOutcome) => void;
}> = ({ entry, manifest, date, onClose, onRestored }) => {
  const { t } = useTranslation();
  const sections = sortCategories(manifest.sections);
  const [chosen, setChosen] = useState<DataCategory[]>(sections.map((section) => section.category));
  const restore = useMutation({
    mutationFn: () => unwrapResult(commands.backupRestore(entry.id, chosen)),
    onSuccess: onRestored,
  });
  const toggle = (category: DataCategory) =>
    setChosen((now) =>
      now.includes(category) ? now.filter((kept) => kept !== category) : [...now, category],
    );

  return (
    <Sheet title={t('data.restore.title', { date })} onClose={onClose}>
      <div data-testid="restore-backup" className="flex flex-col gap-4 pb-1">
        <div className="flex flex-col gap-0.5 text-sm">
          <p>{madeBy(t, manifest)}</p>
          <p className="text-muted">
            {t('data.backups.where', {
              size: formatBytes(entry.sizeBytes ?? 0),
              folder: folderLabel(t, entry.place, entry.folder, entry.name),
            })}
          </p>
          {manifest.secrets && (
            <p className="text-muted flex items-center gap-1.5">
              <KeyRound aria-hidden size={14} />
              {t('data.restore.secrets')}
            </p>
          )}
        </div>
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-muted pb-1 text-xs font-semibold uppercase">
            {t('data.restore.choose')}
          </legend>
          {sections.map((section) => {
            const line = countsLine(t, section.counts) || t('data.restore.empty');
            return (
              <label
                key={section.category}
                data-testid="backup-section"
                className={option(chosen.includes(section.category))}
              >
                <input
                  type="checkbox"
                  checked={chosen.includes(section.category)}
                  onChange={() => toggle(section.category)}
                  className="accent-primary size-4 shrink-0"
                />
                <span className="flex min-w-0 flex-col">
                  <span>{t(`data.categories.${section.category}`)}</span>
                  <span className="text-muted truncate text-xs">{line}</span>
                </span>
              </label>
            );
          })}
        </fieldset>
        <p className="text-muted text-xs">{t('data.restore.merges')}</p>
        {restore.error && (
          <Problem title={t('settings.backupRestoreFailed')} detail={restore.error.message} />
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
            disabled={chosen.length === 0 || restore.isPending}
            onClick={() => restore.mutate()}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {restore.isPending ? t('data.restore.restoring') : t('data.restore.restore')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
