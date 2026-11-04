import { useMutationState } from '@tanstack/react-query';
import type { SettingChange } from '@raygun/bindings';
import { AlertCircle, Check, Copy, RotateCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SETTINGS_MUTATION, useUpdateSetting } from './useSettings';

/**
 * Something that didn't work, said plainly (what failed, what to do), with the raw detail under
 * More info to read or copy. Sources failing use `shell/SourceIssue`; this is for Raygun's own
 * screens (settings, devices, AI, updates).
 */
export const Problem: FC<{
  title: string;
  /** The raw error, under More info. */
  detail?: string | null;
  onRetry?: () => void;
  className?: string;
}> = ({ title, detail, onRetry, className = '' }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  return (
    <div
      role="alert"
      data-testid="problem"
      className={`bg-surface/70 flex flex-col gap-1.5 rounded-xl px-3 py-2 text-sm ${className}`}
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <AlertCircle aria-hidden size={16} className="text-primary shrink-0" />
        <span className="min-w-0 flex-1 font-medium">{title}</span>
        <span className="flex items-center gap-1">
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="text-muted hover:text-foreground hover:bg-background flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-semibold"
            >
              <RotateCw aria-hidden size={12} />
              {t('issue.retry')}
            </button>
          )}
          {detail && (
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setOpen(!open)}
              className="text-muted hover:text-foreground hover:bg-background rounded-full px-2.5 py-1 text-xs font-semibold"
            >
              {open ? t('issue.less') : t('issue.more')}
            </button>
          )}
        </span>
      </div>
      {detail && open && (
        <div className="flex flex-col gap-1">
          <p className="selectable bg-background text-muted rounded-lg px-3 py-2 font-mono text-xs wrap-anywhere">
            {detail}
          </p>
          <button
            type="button"
            onClick={() =>
              void navigator.clipboard
                ?.writeText(detail)
                .then(() => setCopied(true))
                .catch(() => undefined)
            }
            className="text-muted hover:text-foreground flex items-center gap-1 self-end text-xs font-semibold"
          >
            {copied ? <Check aria-hidden size={12} /> : <Copy aria-hidden size={12} />}
            {copied ? t('issue.copied') : t('issue.copy')}
          </button>
        </div>
      )}
    </div>
  );
};

/**
 * A settings change that didn't save, on the screen it was made on: the latest change since the
 * screen opened, until a later one saves. Try again sends it once more.
 */
export const SettingsSaveIssue: FC = () => {
  const { t } = useTranslation();
  const [openedAt] = useState(() => Date.now());
  const update = useUpdateSetting();
  const latest = useMutationState({
    filters: {
      mutationKey: SETTINGS_MUTATION,
      predicate: (mutation) => mutation.state.submittedAt >= openedAt,
    },
    select: (mutation) => ({
      status: mutation.state.status,
      error: mutation.state.error,
      variables: mutation.state.variables,
    }),
  }).at(-1);
  if (latest?.status !== 'error') return null;
  const change = latest.variables as SettingChange | SettingChange[] | undefined;
  return (
    <Problem
      title={t('settings.saveFailedTitle')}
      detail={latest.error instanceof Error ? latest.error.message : String(latest.error ?? '')}
      onRetry={change && !Array.isArray(change) ? () => update.mutate(change) : undefined}
      className="mx-1 mt-3"
    />
  );
};
