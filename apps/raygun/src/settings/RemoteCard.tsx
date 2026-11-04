import { commands } from '@raygun/bindings';
import { RefreshCw, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';
import { useRemoteChange, useRemoteStatus } from './remoteQueries';

/**
 * Control from a web browser (M20): any phone or computer on this network scans the code and
 * controls playback from its browser. The server runs while devices may find this one (the
 * Devices page's switch, M46.47).
 */
export const RemoteCard: FC = () => {
  const { t, i18n } = useTranslation();
  const status = useRemoteStatus();
  const newCode = useRemoteChange(() => unwrapResult(commands.remoteNewCode()));
  const forget = useRemoteChange((id: string) => unwrapResult(commands.remoteForgetDevice(id)));
  const data = status.data;
  const dateFormat = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' });
  const failure = newCode.error ?? forget.error ?? status.error;

  return (
    <div data-testid="remote-card">
      <PrefGroup title={t('link.browserTitle')}>
        {!data?.enabled && <li className="text-muted px-1 py-2 text-sm">{t('link.browserOff')}</li>}
        {data?.enabled && data.pairQrSvg && data.pairUrl && (
          <li className="flex flex-col items-center gap-2 px-1 py-3">
            <img
              data-testid="remote-qr"
              alt={t('remote.qrAlt')}
              src={`data:image/svg+xml;utf8,${encodeURIComponent(data.pairQrSvg)}`}
              className="size-44 rounded-lg bg-white p-1"
            />
            <code
              data-testid="remote-pair-url"
              className="selectable text-muted text-center text-xs break-all"
            >
              {data.pairUrl}
            </code>
          </li>
        )}
        {data?.enabled && !data.pairUrl && data.url && (
          <li className="text-muted px-1 py-2 text-sm">{t('remote.codeUsed')}</li>
        )}
        {data?.enabled && data.url && (
          <PrefAction
            icon={RefreshCw}
            title={t('remote.newCode')}
            disabled={newCode.isPending}
            onClick={() => newCode.mutate(undefined)}
          />
        )}
        {data?.enabled && (
          <>
            {data.devices.length === 0 ? (
              <li className="text-muted px-1 py-2 text-sm">{t('remote.noDevices')}</li>
            ) : (
              data.devices.map((device) => (
                <li key={device.id} className="flex items-center gap-2 px-1 py-2">
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-medium">{device.name}</span>
                    <span className="text-muted text-xs">
                      {t('remote.pairedOn', { date: dateFormat.format(device.pairedAt ?? 0) })}
                    </span>
                  </span>
                  <button
                    type="button"
                    aria-label={t('remote.forget', { name: device.name })}
                    disabled={forget.isPending}
                    onClick={() => forget.mutate(device.id)}
                    className="text-muted hover:bg-surface grid size-10 place-items-center rounded-full"
                  >
                    <X aria-hidden size={16} />
                  </button>
                </li>
              ))
            )}
          </>
        )}
        {(data?.error ?? failure) && (
          <li className="px-1 py-2">
            <Problem title={t('remote.problem')} detail={data?.error ?? failure?.message} />
          </li>
        )}
      </PrefGroup>
    </div>
  );
};
