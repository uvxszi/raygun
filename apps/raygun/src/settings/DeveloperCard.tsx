import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FlaskConical, PackagePlus, Trash2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { InstallProgress } from '../extensions/InstallProgress';
import { useInstallQueue } from '../extensions/installQueue';
import { CATALOG, EXTENSIONS, useCatalog } from '../extensions/useExtensions';
import { unwrapResult } from '../lib/commandResult';
import { SourceIssue } from '../shell/SourceIssue';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';

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
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: CATALOG });
    void queryClient.invalidateQueries({ queryKey: EXTENSIONS });
    void queryClient.invalidateQueries({ queryKey: ['stremio'] });
  };
  const add = useMutation({
    mutationFn: () => unwrapResult(commands.devAddTestSources()),
    onSuccess: refresh,
  });
  // M46.47: the test repository, its extensions and the test addon go again.
  const remove = useMutation({
    mutationFn: () => unwrapResult(commands.devRemoveTestSources()),
    onSuccess: refresh,
  });
  const missing = (catalog.data ?? []).flatMap((repo) =>
    repo.entries.filter((entry) => entry.state === 'available'),
  );
  const report = add.data;

  return (
    <div data-testid="developer-card">
      <PrefGroup title={t('developer.title')}>
        <PrefAction
          icon={FlaskConical}
          title={add.isPending ? t('developer.adding') : t('developer.add')}
          summary={
            report && (
              <span data-testid="developer-report">
                {t('developer.report', {
                  added: report.added.length,
                  already: report.already.length,
                  failed: report.failed.length,
                })}
              </span>
            )
          }
          disabled={add.isPending}
          onClick={() => add.mutate()}
        />
        {report?.failed.map((failure) => (
          <li key={failure.url} className="px-1 py-1">
            <SourceIssue compact source={failure.url}>
              {failure.error}
            </SourceIssue>
          </li>
        ))}
        {add.error && (
          <li className="px-1 py-1">
            <Problem title={t('developer.addFailed')} detail={add.error.message} />
          </li>
        )}
        <PrefAction
          icon={PackagePlus}
          title={t('developer.installAll', { count: missing.length })}
          disabled={missing.length === 0}
          onClick={() => queue.install(missing)}
        />
        <li className="list-none px-1 py-1 empty:hidden">
          <InstallProgress />
        </li>
        <PrefAction
          icon={Trash2}
          title={t('developer.remove')}
          summary={
            remove.isSuccess ? (
              <span role="status" data-testid="developer-removed">
                {t('developer.removed', { count: remove.data })}
              </span>
            ) : undefined
          }
          disabled={remove.isPending}
          onClick={() => remove.mutate()}
        />
        {remove.error && (
          <li className="px-1 py-1">
            <Problem title={t('developer.removeFailed')} detail={remove.error.message} />
          </li>
        )}
      </PrefGroup>
    </div>
  );
};
