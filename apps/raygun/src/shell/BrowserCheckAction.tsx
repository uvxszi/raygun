import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

type BrowserCheckActionProps = {
  /** The failure's text, as the source issue shows it. */
  error: string;
  source?: string | null;
  onRetry?: () => void;
};

/**
 * "Open the site to pass the check" under a source's browser-check failure, while Solve browser
 * checks myself is on (experimental; `raygun_ext_host::browser_check`). The site opens in a
 * browser the person uses; once it closes with cookies, every failed request is asked again.
 */
export const BrowserCheckAction: FC<BrowserCheckActionProps> = ({ error, source, onRetry }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const on = useSettings().data?.manualBrowserChecks ?? false;
  const target = useQuery({
    queryKey: ['browserCheck', error, source ?? null],
    queryFn: () => unwrapResult(commands.browserCheckTarget(error, source ?? null)),
    enabled: on,
  });
  const open = useMutation({
    mutationFn: (extensionId: string) => unwrapResult(commands.browserCheckOpen(extensionId)),
    onSuccess: (taken) => {
      if (taken === 0) return;
      onRetry?.();
      void queryClient.refetchQueries({ predicate: (query) => query.state.status === 'error' });
    },
  });
  const found = on ? target.data : null;
  if (!found) return null;

  return (
    <div className="flex flex-col items-start gap-1">
      <button
        type="button"
        disabled={open.isPending}
        onClick={() => open.mutate(found.extensionId)}
        className="bg-background hover:text-foreground flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-60"
      >
        <Globe aria-hidden size={14} />
        {open.isPending ? t('issue.browserCheck.opening') : t('issue.browserCheck.open')}
      </button>
      {open.data === 0 && <p className="text-xs">{t('issue.browserCheck.notPassed')}</p>}
      {open.isError && <p className="text-xs">{t('issue.browserCheck.failed')}</p>}
    </div>
  );
};
