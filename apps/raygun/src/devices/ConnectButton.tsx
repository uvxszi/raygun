import type { ConnectDevice } from '@raygun/bindings';
import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Laptop, LoaderCircle, MonitorSpeaker, QrCode, Smartphone, Volume2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useConnectDevices,
  useConnectStatus,
  useConnectTo,
  useDisconnect,
  useForgetDevice,
} from '../connect/useConnect';
import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { Problem } from '../settings/Problem';
import { useRemoteStatus } from '../settings/remoteQueries';
import { Sheet } from '../shell/Sheet';
import { AudioOutputs } from './AudioOutputs';
import { TAP_BUTTON, TAP_ICON, TAP_NAME, TAP_NOTE, TAP_ROW } from './tapTargets';
import { useSyncStatus } from './useDeviceSync';

const HEADING = 'text-muted px-3 pt-4 pb-1 text-sm font-semibold';

const DeviceIcon: FC<{ kind: string; className?: string; size?: number }> = ({
  kind,
  className = '',
  size = 24,
}) =>
  kind === 'phone' ? (
    <Smartphone aria-hidden size={size} className={className} />
  ) : (
    <Laptop aria-hidden size={size} className={className} />
  );

/**
 * Spotify's Connect button (M46.13), beside the song in the player bar. Its sheet plays on another
 * Raygun device (M46.15): this device and your linked devices (tapping one moves playback there
 * at once, tapping this one brings it back), and the switch that lets others find this one.
 */
export const ConnectButton: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const session = useConnectStatus().data?.session ?? null;
  return (
    <>
      <button
        type="button"
        aria-label={t('connect.open')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`grid size-10 shrink-0 place-items-center rounded-full ${
          session ? 'text-primary' : 'text-muted hover:text-foreground'
        } ${className}`}
      >
        {session ? (
          <DeviceIcon kind={session.deviceKind} size={20} />
        ) : (
          <MonitorSpeaker aria-hidden size={20} />
        )}
      </button>
      {anchor && (
        <Sheet
          title={t('connect.title')}
          anchor={anchor}
          popoverWidth={400}
          onClose={() => setAnchor(null)}
        >
          <ConnectSheet onClose={() => setAnchor(null)} />
        </Sheet>
      )}
    </>
  );
};

const ConnectSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const platform = usePlatform();
  const phone = platform === 'android' || platform === 'ios';
  const status = useConnectStatus().data;
  const session = status?.session ?? null;
  const devices = useConnectDevices(true);
  const connectTo = useConnectTo();
  const disconnect = useDisconnect();
  const forget = useForgetDevice();
  const network = useLocalNetworkGate('connect');
  // Linked devices first; ones only heard on the network say they need linking.
  const others = [...(devices.data ?? [])].sort((a, b) => Number(b.paired) - Number(a.paired));
  const error = connectTo.error?.message ?? disconnect.error?.message ?? status?.error;
  const errorTitle = connectTo.error
    ? t('connect.playFailed', {
        name: others.find((device) => device.id === connectTo.variables)?.name ?? '',
      })
    : t('connect.problem');
  const busy = connectTo.isPending || disconnect.isPending;

  const play = (device: ConnectDevice) =>
    void network.guard(`http://${device.address}/`, () => connectTo.mutate(device.id));

  const deviceNote = (device: ConnectDevice) => {
    if (session?.deviceId === device.id) return t('connect.status.playing');
    if (status?.asking === device.id) return t('connect.status.asking', { name: device.name });
    if (!device.found) return t('connect.status.away');
    return device.paired ? t('connect.status.ready') : t('connect.status.notLinked');
  };

  return (
    <div data-testid="connect-sheet" className="-mx-2 flex flex-col gap-1">
      <div data-testid="connect-current" className={`${TAP_ROW} bg-primary/10`}>
        {session ? (
          <DeviceIcon kind={session.deviceKind} className="text-primary shrink-0" />
        ) : (
          <DeviceIcon kind={phone ? 'phone' : 'computer'} className="text-primary shrink-0" />
        )}
        <div className="min-w-0 flex-1">
          <p className="text-primary truncate text-base font-semibold">
            {session ? session.deviceName : status?.thisDevice || t('connect.thisDevice')}
          </p>
          <p className="text-muted text-sm">
            {session ? t('connect.playingThere') : t('connect.playingHere')}
          </p>
        </div>
        {session && (
          <button
            type="button"
            disabled={busy}
            onClick={() => disconnect.mutate(false)}
            className={`${TAP_BUTTON} hover:bg-background`}
          >
            {t('connect.disconnect')}
          </button>
        )}
      </div>
      {/* Where this device's sound comes out (M46.43), while it plays here. */}
      {!session && <AudioOutputs />}

      <h3 className={HEADING}>{t('connect.selectDevice')}</h3>
      <ul data-testid="connect-devices" className="flex flex-col gap-0.5">
        {session && (
          <li>
            <button
              type="button"
              disabled={busy}
              onClick={() => disconnect.mutate(true)}
              className={`${TAP_ROW} hover:bg-background disabled:opacity-50`}
            >
              <DeviceIcon kind={phone ? 'phone' : 'computer'} className="text-muted shrink-0" />
              <span className="min-w-0 flex-1">
                <span className={TAP_NAME}>{status?.thisDevice || t('connect.thisDevice')}</span>
                <span className={TAP_NOTE}>{t('connect.playHere')}</span>
              </span>
            </button>
          </li>
        )}
        {others.map((device) => {
          const asking = status?.asking === device.id;
          const active = session?.deviceId === device.id;
          return (
            <li key={device.id} className="flex items-center gap-1">
              <button
                type="button"
                aria-label={t('connect.playOn', { name: device.name })}
                aria-current={active ? 'true' : undefined}
                disabled={busy || active}
                onClick={() => play(device)}
                className={`${TAP_ROW} hover:bg-background min-w-0 flex-1 disabled:cursor-default`}
              >
                {active ? (
                  <Volume2 aria-hidden size={24} className="text-primary shrink-0" />
                ) : (
                  <DeviceIcon
                    kind={device.kind}
                    className={`shrink-0 ${device.found && device.paired ? '' : 'text-muted'}`}
                  />
                )}
                <span className="min-w-0 flex-1">
                  <span className={`${TAP_NAME} ${active ? 'text-primary' : ''}`}>
                    {device.name}
                  </span>
                  <span className={TAP_NOTE}>{deviceNote(device)}</span>
                </span>
                {asking && (
                  <LoaderCircle
                    aria-hidden
                    size={20}
                    className="text-muted shrink-0 animate-spin"
                  />
                )}
              </button>
              {device.paired && !active && (
                <button
                  type="button"
                  aria-label={t('connect.forget', { name: device.name })}
                  title={t('connect.forget', { name: device.name })}
                  onClick={() => forget.mutate(device.id)}
                  className={`${TAP_ICON} text-muted hover:text-foreground hover:bg-background`}
                >
                  <X aria-hidden size={20} />
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {devices.isSuccess && others.length === 0 && (
        <p className="text-muted px-3 py-2 text-sm">{t('connect.noDevices')}</p>
      )}
      {error && <Problem title={errorTitle} detail={error} className="mx-3" />}
      <AllowHere />
      {/* Linking and syncing are one place (M46.43). */}
      <Link
        to="/settings/devices"
        onClick={onClose}
        className={`${TAP_ROW} hover:bg-background text-base`}
      >
        <QrCode aria-hidden size={24} className="text-muted shrink-0" />
        {t('connect.manage')}
      </Link>
      {network.sheet}
    </div>
  );
};

/**
 * The switch that lets your devices find this one (M46.47): the remote's server, which Connect
 * plays through, and sync, as on the Devices page.
 */
const AllowHere: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const remote = useRemoteStatus();
  const sync = useSyncStatus();
  const setEnabled = useMutation({
    mutationFn: (on: boolean) => unwrapResult(commands.devicesSetSharing(on)),
    onSettled: () => {
      void client.invalidateQueries({ queryKey: ['remote'] });
      void client.invalidateQueries({ queryKey: ['sync'] });
      void client.invalidateQueries({ queryKey: ['connect'] });
    },
  });
  const network = useLocalNetworkGate('remote');
  const enabled = (remote.data?.enabled ?? false) && (sync.data?.enabled ?? false);
  return (
    <div className="flex flex-col gap-1 pt-1">
      {/* The whole row toggles it, like a settings switch. */}
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        aria-labelledby="connect-allow"
        disabled={!remote.data || !sync.data || setEnabled.isPending || network.checking}
        onClick={() => {
          if (enabled) setEnabled.mutate(false);
          else void network.guard(null, () => setEnabled.mutate(true));
        }}
        className={`${TAP_ROW} hover:bg-background justify-between disabled:opacity-60`}
      >
        <span id="connect-allow" className="min-w-0 flex-1 text-base">
          {t('connect.allowHere')}
        </span>
        <span
          aria-hidden
          className={`relative h-8 w-13 shrink-0 rounded-full border-2 transition-colors ${enabled ? 'bg-primary border-primary' : 'bg-surface border-muted/50'}`}
        >
          <span
            className={`absolute top-1/2 -translate-y-1/2 rounded-full transition-all ${enabled ? 'bg-background left-[1.375rem] size-6' : 'bg-muted left-1.5 size-4'}`}
          />
        </span>
      </button>
      {(setEnabled.error ?? remote.data?.error) && (
        <Problem
          title={t('link.shareFailed')}
          detail={setEnabled.error?.message ?? remote.data?.error}
          className="mx-3"
        />
      )}
      {network.sheet}
    </div>
  );
};
