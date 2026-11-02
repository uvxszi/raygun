import { Link } from '@tanstack/react-router';
import { Laptop, MonitorSpeaker, QrCode, RefreshCw, Smartphone } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlatform } from '../lib/usePlatform';
import { Sheet } from '../shell/Sheet';
import { useSyncNow, useSyncStatus } from './useDeviceSync';

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

/** "5 minutes ago", "yesterday": when a device last synced. */
const ago = (at: number) => {
  const minutes = Math.round((at - Date.now()) / 60_000);
  if (minutes > -60) return relative.format(minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (hours > -24) return relative.format(hours, 'hour');
  return relative.format(Math.round(hours / 24), 'day');
};

const ROW = 'flex items-center gap-3 rounded-xl px-2 py-2.5';

/**
 * Spotify's Connect button (M46.13), beside the song in the player bar: this device, the devices
 * paired with it and when each last synced, a sync now, and the way to set up pairing or to
 * control Raygun from another device.
 */
export const ConnectButton: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const status = useSyncStatus();
  const syncNow = useSyncNow();
  const platform = usePlatform();
  const phone = platform === 'android' || platform === 'ios';
  const enabled = status.data?.enabled ?? false;
  const peers = status.data?.peers ?? [];
  const close = () => setAnchor(null);
  return (
    <>
      <button
        type="button"
        aria-label={t('connect.open')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`grid size-10 shrink-0 place-items-center rounded-full ${
          enabled && peers.length > 0 ? 'text-primary' : 'text-muted hover:text-foreground'
        } ${className}`}
      >
        <MonitorSpeaker aria-hidden size={20} />
      </button>
      {anchor && (
        <Sheet title={t('connect.title')} anchor={anchor} popoverWidth={340} onClose={close}>
          <div data-testid="connect-sheet" className="-mx-2 flex flex-col gap-1">
            <div className={`${ROW} bg-primary/10`}>
              {phone ? (
                <Smartphone aria-hidden size={22} className="text-primary" />
              ) : (
                <Laptop aria-hidden size={22} className="text-primary" />
              )}
              <div className="min-w-0 flex-1">
                <p className="text-primary truncate font-semibold">
                  {status.data?.deviceName || t('connect.thisDevice')}
                </p>
                <p className="text-muted text-xs">{t('connect.playingHere')}</p>
              </div>
            </div>

            {!enabled ? (
              <div className="flex flex-col gap-2 px-2 py-3">
                <p className="text-muted text-sm">{t('connect.syncOff')}</p>
                <Link
                  to="/settings/devices"
                  onClick={close}
                  className="bg-primary text-background self-start rounded-full px-4 py-2 text-sm font-semibold"
                >
                  {t('connect.setUp')}
                </Link>
              </div>
            ) : (
              <>
                <h3 className="text-muted px-2 pt-3 text-xs font-semibold tracking-wide uppercase">
                  {t('connect.devices')}
                </h3>
                {peers.length === 0 && (
                  <p className="text-muted px-2 py-1 text-sm">{t('connect.noPeers')}</p>
                )}
                <ul className="flex flex-col">
                  {peers.map((peer) => (
                    <li key={peer.deviceId} className={ROW}>
                      <Laptop aria-hidden size={20} className="text-muted" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium">{peer.name}</p>
                        <p className="text-muted text-xs">
                          {peer.lastSyncAt
                            ? t('connect.synced', { when: ago(peer.lastSyncAt) })
                            : t('connect.neverSynced')}
                        </p>
                      </div>
                      <button
                        type="button"
                        aria-label={t('connect.syncWith', { name: peer.name })}
                        disabled={syncNow.isPending}
                        onClick={() => syncNow.mutate(peer.deviceId)}
                        className="hover:bg-background grid size-9 place-items-center rounded-full disabled:opacity-50"
                      >
                        <RefreshCw
                          aria-hidden
                          size={16}
                          className={syncNow.isPending ? 'animate-spin' : ''}
                        />
                      </button>
                    </li>
                  ))}
                </ul>
                {status.data?.error && (
                  <p role="alert" className="text-primary px-2 text-xs wrap-anywhere">
                    {status.data.error}
                  </p>
                )}
                <Link
                  to="/settings/devices"
                  onClick={close}
                  className="text-muted hover:text-foreground px-2 py-1 text-sm"
                >
                  {t('connect.manage')}
                </Link>
              </>
            )}

            <Link
              to="/settings/connections"
              onClick={close}
              className={`${ROW} hover:bg-background text-sm`}
            >
              <QrCode aria-hidden size={20} className="text-muted" />
              {t('connect.remote')}
            </Link>
          </div>
        </Sheet>
      )}
    </>
  );
};
