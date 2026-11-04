import { commands } from '@raygun/bindings';
import type { McpStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Eye, EyeOff, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PrefAction, PrefGroup, PrefSwitch } from './prefs';
import { Problem } from './Problem';

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
      className="text-muted hover:text-foreground flex shrink-0 items-center gap-1 text-xs font-semibold"
    >
      <Copy aria-hidden size={14} />
      {copied ? t('mcp.copied') : t('mcp.copy')}
    </button>
  );
};

/** Lets AI assistants on this device control Raygun through the Model Context Protocol (M19). */
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
    <div data-testid="mcp-card">
      <PrefGroup title={t('mcp.group')}>
        <PrefSwitch
          title={t('mcp.title')}
          summary={t('mcp.description')}
          checked={data?.enabled ?? false}
          disabled={!data || setEnabled.isPending}
          onChange={(enabled) => setEnabled.mutate(enabled)}
        />
        {data?.enabled && (
          <>
            <li className="flex flex-col gap-1 px-1 py-2">
              <span className="text-muted text-xs">{t('mcp.address')}</span>
              <span className="flex items-center justify-between gap-2">
                <code data-testid="mcp-url" className="selectable min-w-0 text-sm break-all">
                  {data.url ?? t('mcp.notRunning')}
                </code>
                {data.url && <CopyButton text={data.url} label={t('mcp.copyAddress')} />}
              </span>
            </li>
            <li className="flex flex-col gap-1 px-1 py-2">
              <span className="text-muted text-xs">{t('mcp.token')}</span>
              <span className="flex items-center justify-between gap-2">
                <code data-testid="mcp-token" className="selectable min-w-0 text-sm break-all">
                  {showToken ? data.token : '•'.repeat(16)}
                </code>
                <span className="flex shrink-0 items-center gap-3">
                  <button
                    type="button"
                    aria-label={t(showToken ? 'mcp.hideToken' : 'mcp.showToken')}
                    onClick={() => setShowToken((shown) => !shown)}
                    className="text-muted hover:text-foreground"
                  >
                    {showToken ? <EyeOff aria-hidden size={16} /> : <Eye aria-hidden size={16} />}
                  </button>
                  <CopyButton text={data.token} label={t('mcp.copyToken')} />
                </span>
              </span>
            </li>
            <PrefAction
              icon={RefreshCw}
              title={t('mcp.newToken')}
              disabled={newToken.isPending}
              onClick={() => newToken.mutate(undefined)}
            />
          </>
        )}
        {(data?.error ?? error) && (
          <li className="px-1 py-2">
            <Problem title={t('mcp.problem')} detail={data?.error ?? error?.message} />
          </li>
        )}
      </PrefGroup>
    </div>
  );
};
