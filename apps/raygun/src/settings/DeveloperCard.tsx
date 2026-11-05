import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { PackagePlus, Trash2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { InstallProgress } from '../extensions/InstallProgress';
import { useInstallQueue } from '../extensions/installQueue';
import { CATALOG, EXTENSIONS, useCatalog } from '../extensions/useExtensions';
import { unwrapResult } from '../lib/commandResult';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';

/**
 * Developer tools (Settings → Data): install everything the added repositories list, and take
 * off the test sources older builds could add (M46.47).
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
  // M46.47: the test repository, its extensions and the test addon go again.
  const remove = useMutation({
    mutationFn: () => unwrapResult(commands.devRemoveTestSources()),
    onSuccess: refresh,
  });
  const missing = (catalog.data ?? []).flatMap((repo) =>
    repo.entries.filter((entry) => entry.state === 'available'),
  );

  return (
    <div data-testid="developer-card">
      <PrefGroup title={t('developer.title')}>
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
