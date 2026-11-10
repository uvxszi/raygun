import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import type { ExtensionList } from './tabs';
import { useRefreshCatalog, useReloadExtensions } from './useExtensions';

/**
 * The extension catalogue's Reload (Installed: the installed extensions read again) or Refresh
 * (every repository's index fetched again) for the list shown, and a line while either runs or
 * after one failed.
 */
export const useCatalogRefresh = (list: ExtensionList) => {
  const { t } = useTranslation();
  const reload = useReloadExtensions();
  const refresh = useRefreshCatalog();
  const installed = list === 'installed';
  const error = reload.error ?? refresh.error;
  return {
    label: t(installed ? 'extensions.reload' : 'extensions.refresh'),
    run: () => (installed ? reload.mutate() : refresh.mutate()),
    status: (
      <>
        {error && <SourceIssue>{error.message}</SourceIssue>}
        {(reload.isPending || refresh.isPending) && (
          <p role="status" className="text-muted flex items-center gap-2 text-sm">
            <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
            {t(reload.isPending ? 'extensions.reload' : 'extensions.refresh')}
          </p>
        )}
      </>
    ),
  };
};
