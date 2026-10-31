import { commands } from '@raygun/bindings';
import type { McpStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Eye, EyeOff, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

const MCP = ['mcp'];

const useMcpStatus = () =>
  useQuery({ queryKey: MCP, queryFn: () => unwrapResult(commands.mcpStatus()) });

/** Every change answers with the new status, which replaces the cached one. */
const useMcpChange = <Variables,>(run: (variables: Variables) => Promise<McpStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(MCP, status),
  });
};

const CopyButton: FC<{ text: string; label: string }> = ({ text, label }) => {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      aria-label={label}
      onClick={() =>
        void navigator.clipboard?.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        })
      }
      className="text-muted flex shrink-0 items-center gap-1 text-xs"
    >
      <Copy aria-hidden size={14} />
      {copied ? t('mcp.copied') : t('mcp.copy')}
    </button>
  );
};

/** Lets AI agents control Raygun through the Model Context Protocol (M19). */
export const McpCard: FC = () => {
  const { t } = useTranslation();
  const status = useMcpStatus();
  const setEnabled = useMcpChange((enabled: boolean) =>
    unwrapResult(commands.mcpSetEnabled(enabled)),
  );
  const newToken = useMcpChange(() => unwrapResult(commands.mcpNewToken()));
  const [showToken, setShowToken] = useState(false);

  const data = status.data;
  const error = status.error ?? setEnabled.error ?? newToken.error;

  return (
    <section
      data-testid="mcp-card"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <h2 id="mcp-title" className="font-medium">
            {t('mcp.title')}
          </h2>
          <p className="text-muted text-xs">{t('mcp.description')}</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={data?.enabled ?? false}
          aria-labelledby="mcp-title"
          disabled={!data || setEnabled.isPending}
          onClick={() => data && setEnabled.mutate(!data.enabled)}
          className={`relative h-7 w-12 shrink-0 rounded-full transition-colors disabled:opacity-60 ${data?.enabled ? 'bg-primary' : 'bg-background'}`}
        >
          <span
            aria-hidden
            className={`bg-foreground absolute top-1 size-5 rounded-full transition-all ${data?.enabled ? 'left-6' : 'left-1'}`}
          />
        </button>
      </div>
      {data?.enabled && (
        <dl className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <dt className="text-muted text-xs">{t('mcp.address')}</dt>
            <dd className="flex items-center justify-between gap-2">
              <code data-testid="mcp-url" className="min-w-0 break-all">
                {data.url ?? t('mcp.notRunning')}
              </code>
              {data.url && <CopyButton text={data.url} label={t('mcp.copyAddress')} />}
            </dd>
          </div>
          <div className="flex flex-col gap-1">
            <dt className="text-muted text-xs">{t('mcp.token')}</dt>
            <dd className="flex items-center justify-between gap-2">
              <code data-testid="mcp-token" className="min-w-0 break-all">
                {showToken ? data.token : '•'.repeat(16)}
              </code>
              <span className="flex shrink-0 items-center gap-3">
                <button
                  type="button"
                  aria-label={t(showToken ? 'mcp.hideToken' : 'mcp.showToken')}
                  onClick={() => setShowToken((shown) => !shown)}
                  className="text-muted"
                >
                  {showToken ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
                </button>
                <CopyButton text={data.token} label={t('mcp.copyToken')} />
              </span>
            </dd>
          </div>
          <p className="text-muted text-xs">{t('mcp.hint')}</p>
          <button
            type="button"
            disabled={newToken.isPending}
            onClick={() => newToken.mutate(undefined)}
            className="text-primary flex items-center gap-2 self-start text-sm disabled:opacity-60"
          >
            <RefreshCw aria-hidden size={14} />
            {t('mcp.newToken')}
          </button>
        </dl>
      )}
      {(data?.error ?? error) && (
        <p role="alert" className="text-primary wrap-anywhere">
          {data?.error ?? error?.message}
        </p>
      )}
    </section>
  );
};
