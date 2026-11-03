import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FlaskConical, PackagePlus, Trash2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useInstallQueue } from '../extensions/installQueue';
import { CATALOG, EXTENSIONS, useCatalog } from '../extensions/useExtensions';
import { unwrapResult } from '../lib/commandResult';
import { SourceIssue } from '../shell/SourceIssue';

const BUTTON =
  'bg-background hover:bg-surface flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50';

/**
 * Testing with many sources (M46.17): every known repository, Raygun's own sources that fail on
 * purpose and some legal Stremio addons, then everything those repositories list. Never part of
 * a fresh install; it's for seeing what breaks.
 */
export const DeveloperCard: FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const catalog = useCatalog();
  const queue = useInstallQueue();
  const add = useMutation({
    mutationFn: () => unwrapResult(commands.devAddTestSources()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: CATALOG });
      void queryClient.invalidateQueries({ queryKey: EXTENSIONS });
      void queryClient.invalidateQueries({ queryKey: ['stremio'] });
    },
  });
  // M46.47: the test repository, its extensions and the test addon go again.
  const remove = useMutation({
    mutationFn: () => unwrapResult(commands.devRemoveTestSources()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: CATALOG });
      void queryClient.invalidateQueries({ queryKey: EXTENSIONS });
      void queryClient.invalidateQueries({ queryKey: ['stremio'] });
    },
  });
  const missing = (catalog.data ?? []).flatMap((repo) =>
    repo.entries.filter((entry) => entry.state === 'available'),
  );
  const report = add.data;

  return (
    <section
      data-testid="developer-card"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <div className="flex flex-col gap-0.5">
        <h2 className="flex items-center gap-2 font-medium">
          <FlaskConical aria-hidden size={16} />
          {t('developer.title')}
        </h2>
        <p className="text-muted text-xs">{t('developer.hint')}</p>
      </div>
      <button
        type="button"
        disabled={add.isPending}
        onClick={() => add.mutate()}
        className={BUTTON}
      >
        <FlaskConical aria-hidden size={16} />
        {add.isPending ? t('developer.adding') : t('developer.add')}
      </button>
      {report && (
        <p data-testid="developer-report" className="text-muted text-xs">
          {t('developer.report', {
            added: report.added.length,
            already: report.already.length,
            failed: report.failed.length,
          })}
        </p>
      )}
      {report?.failed.map((failure) => (
        <SourceIssue key={failure.url} compact source={failure.url}>
          {failure.error}
        </SourceIssue>
      ))}
      {add.error && <SourceIssue>{add.error.message}</SourceIssue>}
      <button
        type="button"
        disabled={remove.isPending}
        onClick={() => remove.mutate()}
        className={BUTTON}
      >
        <Trash2 aria-hidden size={16} />
        {t('developer.remove')}
      </button>
      {remove.isSuccess && (
        <p role="status" data-testid="developer-removed" className="text-muted text-xs">
          {t('developer.removed', { count: remove.data })}
        </p>
      )}
      {remove.error && <SourceIssue>{remove.error.message}</SourceIssue>}
      <button
        type="button"
        disabled={missing.length === 0}
        onClick={() => queue.install(missing)}
        className={BUTTON}
      >
        <PackagePlus aria-hidden size={16} />
        {t('developer.installAll', { count: missing.length })}
      </button>
    </section>
  );
};
