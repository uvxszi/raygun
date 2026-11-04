import type { ConnectDevice } from '@raygun/bindings';
import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Laptop, LoaderCircle, MonitorSpeaker, Plus, QrCode, Smartphone, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAddDevice,
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
import { TextField } from '../shell/TextField';
import { AudioOutputs } from './AudioOutputs';
import { useSyncStatus } from './useDeviceSync';

const ROW = 'flex items-center gap-3 rounded-xl px-2 py-2.5';
const HEADING = 'text-muted px-2 pt-3 text-xs font-semibold tracking-wide uppercase';

const DeviceIcon: FC<{ kind: string; className?: string; size?: number }> = ({
  kind,
  className = '',
  size = 20,
}) =>
  kind === 'phone' ? (
    <Smartphone aria-hidden size={size} className={className} />
  ) : (
    <Laptop aria-hidden size={size} className={className} />
  );

/**
 * Spotify's Connect button (M46.13), beside the song in the player bar. Its sheet plays on another
 * Raygun device (M46.15): this device, the others found on the network or added by address, and
 * the switch that lets others play on this one; then device sync and the browser remote.
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
          popoverWidth={360}
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
  const [adding, setAdding] = useState(false);
  const others = devices.data ?? [];
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
    return device.paired ? t('connect.status.ready') : t('connect.status.asksFirst');
  };

  return (
    <div data-testid="connect-sheet" className="-mx-2 flex flex-col gap-1">
      <div className={`${ROW} bg-primary/10`}>
        {session ? (
          <DeviceIcon kind={session.deviceKind} size={22} className="text-primary" />
        ) : (
          <DeviceIcon kind={phone ? 'phone' : 'computer'} size={22} className="text-primary" />
        )}
        <div className="min-w-0 flex-1">
          <p className="text-primary truncate font-semibold">
            {session ? session.deviceName : status?.thisDevice || t('connect.thisDevice')}
          </p>
          <p className="text-muted text-xs">
            {session ? t('connect.playingThere') : t('connect.playingHere')}
          </p>
        </div>
        {session && (
          <button
            type="button"
            disabled={busy}
            onClick={() => disconnect.mutate(false)}
            className="hover:bg-background rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-50"
          >
            {t('connect.disconnect')}
          </button>
        )}
      </div>
      {/* Where this device's sound comes out (M46.43), while it plays here. */}
      {!session && <AudioOutputs />}

      <h3 className={HEADING}>{t('connect.selectDevice')}</h3>
      <ul data-testid="connect-devices" className="flex flex-col">
        {session && (
          <li>
            <button
              type="button"
              disabled={busy}
              onClick={() => disconnect.mutate(true)}
              className={`${ROW} hover:bg-background w-full text-left disabled:opacity-50`}
            >
              <DeviceIcon kind={phone ? 'phone' : 'computer'} className="text-muted" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">
                  {status?.thisDevice || t('connect.thisDevice')}
                </span>
                <span className="text-muted block text-xs">{t('connect.playHere')}</span>
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
                className={`${ROW} hover:bg-background min-w-0 flex-1 text-left disabled:cursor-default`}
              >
                <DeviceIcon
                  kind={device.kind}
                  className={active ? 'text-primary' : device.found ? '' : 'text-muted'}
                />
                <span className="min-w-0 flex-1">
                  <span
                    className={`block truncate text-sm font-medium ${active ? 'text-primary' : ''}`}
                  >
                    {device.name}
                  </span>
                  <span className="text-muted block text-xs">{deviceNote(device)}</span>
                </span>
                {asking && (
                  <LoaderCircle aria-hidden size={16} className="text-muted animate-spin" />
                )}
              </button>
              {device.paired && !active && (
                <button
                  type="button"
                  aria-label={t('connect.forget', { name: device.name })}
                  onClick={() => forget.mutate(device.id)}
                  className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full"
                >
                  <X aria-hidden size={14} />
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {devices.isSuccess && others.length === 0 && (
        <p className="text-muted px-2 py-1 text-sm">{t('connect.noDevices')}</p>
      )}
      {error && <Problem title={errorTitle} detail={error} className="mx-2" />}
      {adding ? (
        <AddDevice onDone={() => setAdding(false)} />
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className={`${ROW} hover:bg-background text-muted text-left text-sm`}
        >
          <Plus aria-hidden size={18} />
          {t('connect.add')}
        </button>
      )}
      <AllowHere />
      {/* Pairing, syncing and the browser remote are one place (M46.43). */}
      <Link
        to="/settings/devices"
        onClick={onClose}
        className={`${ROW} hover:bg-background text-sm`}
      >
        <QrCode aria-hidden size={20} className="text-muted" />
        {t('connect.manage')}
      </Link>
      {network.sheet}
    </div>
  );
};

/** Adds a device by the address it shows, where mDNS doesn't reach (guest Wi-Fi, an emulator). */
const AddDevice: FC<{ onDone: () => void }> = ({ onDone }) => {
  const { t } = useTranslation();
  const [address, setAddress] = useState('');
  const add = useAddDevice();
  const network = useLocalNetworkGate('connect');
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const text = address.trim();
    if (!text) return;
    const url = /^https?:\/\//.test(text) ? text : `http://${text}`;
    void network.guard(url, () => add.mutate(text, { onSuccess: onDone }));
  };
  return (
    <form onSubmit={submit} className="flex flex-col gap-2 px-2 py-2">
      <label className="flex flex-col gap-1 text-xs">
        <span className="text-muted">{t('connect.addressLabel')}</span>
        <TextField
          autoFocus
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          value={address}
          placeholder="192.168.1.20:8820"
          onChange={setAddress}
          aria-label={t('connect.addressLabel')}
          className="bg-background rounded-lg pr-0.5 pl-3"
          inputClassName="py-2 text-sm"
        />
      </label>
      {add.error && <Problem title={t('link.addFailed')} detail={add.error.message} />}
      <div className="flex justify-end gap-2">
        <button type="button" onClick={onDone} className="rounded-full px-3 py-1.5 text-sm">
          {t('connect.cancel')}
        </button>
        <button
          type="submit"
          disabled={add.isPending || address.trim() === ''}
          className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-50"
        >
          {t('connect.addButton')}
        </button>
      </div>
      {network.sheet}
    </form>
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
  const address = remote.data?.url?.replace(/^https?:\/\//, '');
  return (
    <div className="flex flex-col gap-1 px-2 py-2">
      <div className="flex items-center justify-between gap-3">
        <span id="connect-allow" className="text-sm">
          {t('connect.allowHere')}
        </span>
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
          className={`relative h-6 w-10 shrink-0 rounded-full transition-colors disabled:opacity-60 ${enabled ? 'bg-primary' : 'bg-background'}`}
        >
          <span
            aria-hidden
            className={`bg-foreground absolute top-1 size-4 rounded-full transition-all ${enabled ? 'left-5' : 'left-1'}`}
          />
        </button>
      </div>
      {enabled && address && (
        <p className="text-muted text-xs">{t('connect.allowHereAddress', { address })}</p>
      )}
      {(setEnabled.error ?? remote.data?.error) && (
        <Problem
          title={t('link.shareFailed')}
          detail={setEnabled.error?.message ?? remote.data?.error}
        />
      )}
      {network.sheet}
    </div>
  );
};
