import { commands } from '@raygun/bindings';
import type { RemoteStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { RefreshCw, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

const REMOTE = ['remote'];

/** While the remote is on, devices pair from elsewhere: refresh to show them and the used code. */
const REFRESH_MS = 3000;

const useRemoteStatus = () =>
  useQuery({
    queryKey: REMOTE,
    queryFn: () => unwrapResult(commands.remoteStatus()),
    refetchInterval: (query) => (query.state.data?.enabled ? REFRESH_MS : false),
  });

/** Every change answers with the new status, which replaces the cached one. */
const useRemoteChange = <Variables,>(run: (variables: Variables) => Promise<RemoteStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(REMOTE, status),
  });
};

/** Lets browsers on the same network control playback after pairing with a QR code (M20). */
export const RemoteCard: FC = () => {
  const { t, i18n } = useTranslation();
  const status = useRemoteStatus();
  const setEnabled = useRemoteChange((enabled: boolean) =>
    unwrapResult(commands.remoteSetEnabled(enabled)),
  );
  const newCode = useRemoteChange(() => unwrapResult(commands.remoteNewCode()));
  const forget = useRemoteChange((id: string) => unwrapResult(commands.remoteForgetDevice(id)));

  const data = status.data;
  const error = status.error ?? setEnabled.error ?? newCode.error ?? forget.error;
  const dateFormat = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' });

  return (
    <section
      data-testid="remote-card"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <h2 id="remote-title" className="font-medium">
            {t('remote.title')}
          </h2>
          <p className="text-muted text-xs">{t('remote.description')}</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={data?.enabled ?? false}
          aria-labelledby="remote-title"
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
        <div className="flex flex-col gap-3">
          {data.pairQrSvg && data.pairUrl ? (
            <div className="flex flex-col items-center gap-2">
              <img
                data-testid="remote-qr"
                alt={t('remote.qrAlt')}
                src={`data:image/svg+xml;utf8,${encodeURIComponent(data.pairQrSvg)}`}
                className="size-48 rounded-lg bg-white p-1"
              />
              <p className="text-muted text-center text-xs">{t('remote.scan')}</p>
              <code data-testid="remote-pair-url" className="text-center text-xs break-all">
                {data.pairUrl}
              </code>
            </div>
          ) : (
            data.url && <p className="text-muted text-xs">{t('remote.codeUsed')}</p>
          )}
          {data.url && (
            <button
              type="button"
              disabled={newCode.isPending}
              onClick={() => newCode.mutate(undefined)}
              className="text-primary flex items-center gap-2 self-center text-sm disabled:opacity-60"
            >
              <RefreshCw aria-hidden size={14} />
              {t('remote.newCode')}
            </button>
          )}
          <div className="flex flex-col gap-1">
            <h3 className="text-muted text-xs">{t('remote.devices')}</h3>
            {data.devices.length === 0 ? (
              <p className="text-muted text-xs">{t('remote.noDevices')}</p>
            ) : (
              <ul className="flex flex-col">
                {data.devices.map((device) => (
                  <li key={device.id} className="flex h-11 items-center justify-between gap-2">
                    <span className="flex min-w-0 flex-col">
                      <span className="truncate">{device.name}</span>
                      <span className="text-muted text-xs">
                        {t('remote.pairedOn', { date: dateFormat.format(device.pairedAt ?? 0) })}
                      </span>
                    </span>
                    <button
                      type="button"
                      aria-label={t('remote.forget', { name: device.name })}
                      disabled={forget.isPending}
                      onClick={() => forget.mutate(device.id)}
                      className="text-muted grid size-9 place-items-center rounded-full"
                    >
                      <X aria-hidden size={16} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
          <p className="text-muted text-xs">{t('remote.hint')}</p>
        </div>
      )}
      {(data?.error ?? error) && (
        <p role="alert" className="text-primary wrap-anywhere">
          {data?.error ?? error?.message}
        </p>
      )}
    </section>
  );
};
