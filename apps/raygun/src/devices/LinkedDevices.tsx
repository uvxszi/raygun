import { commands } from '@raygun/bindings';
import type { ConnectDevice } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Laptop,
  Link2,
  LoaderCircle,
  Play,
  Plus,
  RefreshCw,
  Smartphone,
  Unlink,
} from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAddDevice,
  useConnectDevices,
  useConnectStatus,
  useConnectTo,
  useForgetDevice,
} from '../connect/useConnect';
import { unwrapResult } from '../lib/commandResult';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { useRemoteStatus } from '../settings/remoteQueries';
import { SourceIssue } from '../shell/SourceIssue';
import { useSyncNow, useSyncStatus } from './useDeviceSync';

const BUTTON =
  'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-50';

const KindIcon: FC<{ kind: string }> = ({ kind }) =>
  kind === 'phone' ? <Smartphone aria-hidden size={20} /> : <Laptop aria-hidden size={20} />;

/** "Synced 3 minutes ago", in the reader's language. */
const useSince = () => {
  const { t, i18n } = useTranslation();
  const relative = new Intl.RelativeTimeFormat(i18n.language, { numeric: 'auto' });
  return (at: number | null | undefined) => {
    if (!at) return t('link.notSynced');
    const minutes = Math.round((at - Date.now()) / 60_000);
    const when =
      Math.abs(minutes) < 60
        ? relative.format(minutes, 'minute')
        : Math.abs(minutes) < 48 * 60
          ? relative.format(Math.round(minutes / 60), 'hour')
          : relative.format(Math.round(minutes / 1440), 'day');
    return t('link.synced', { when });
  };
};

/**
 * Your devices (M46.47): Connect, sync and the remote as one. One switch lets other devices find
 * this one; linking asks the other device once, and then the two keep each other in sync (on
 * every change, and every few minutes) and either can play on the other, like Spotify's device
 * picker.
 */
export const LinkedDevices: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const remote = useRemoteStatus();
  const sync = useSyncStatus();
  const devices = useConnectDevices(true);
  const status = useConnectStatus().data;
  const localNetwork = useLocalNetworkGate('remote');
  const since = useSince();
  const [address, setAddress] = useState('');
  const sharing = (remote.data?.enabled ?? false) && (sync.data?.enabled ?? false);

  const refresh = () => {
    void client.invalidateQueries({ queryKey: ['connect'] });
    void client.invalidateQueries({ queryKey: ['remote'] });
    void client.invalidateQueries({ queryKey: ['sync'] });
  };
  const share = useMutation({
    mutationFn: (on: boolean) => unwrapResult(commands.devicesSetSharing(on)),
    onSettled: refresh,
  });
  const link = useMutation({
    mutationFn: (id: string) => unwrapResult(commands.connectLink(id)),
    onSettled: refresh,
  });
  const play = useConnectTo();
  const forget = useForgetDevice();
  const add = useAddDevice();
  const syncNow = useSyncNow();

  const all = devices.data ?? [];
  const linked = all.filter((device) => device.syncId);
  const nearby = all.filter((device) => !device.syncId);
  const asking = status?.asking;
  const error =
    share.error ?? link.error ?? play.error ?? forget.error ?? add.error ?? syncNow.error;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (address.trim()) add.mutate(address.trim(), { onSuccess: () => setAddress('') });
  };

  const row = (device: ConnectDevice, actions: React.ReactNode, detail: string) => (
    <li
      key={device.id}
      data-testid={device.syncId ? 'linked-device' : 'nearby-device'}
      className="flex flex-wrap items-center gap-3 py-2"
    >
      <span className="text-muted">
        <KindIcon kind={device.kind} />
      </span>
      {/* Wide enough for the name: the buttons go under it on a phone. */}
      <span className="flex min-w-[10rem] flex-1 flex-col">
        <span className="truncate font-medium">{device.name}</span>
        <span className="text-muted truncate text-xs">{detail}</span>
      </span>
      <span className="flex flex-wrap gap-1.5">{actions}</span>
    </li>
  );

  return (
    <section
      data-testid="linked-devices"
      aria-labelledby="linked-title"
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <div className="flex items-center justify-between gap-4">
        <div className="flex flex-col gap-0.5">
          <h2 id="linked-title" className="font-medium">
            {t('link.title')}
          </h2>
          <p className="text-muted text-xs">{t('link.description')}</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={sharing}
          aria-label={t('link.sharing')}
          disabled={share.isPending || localNetwork.checking || !remote.data || !sync.data}
          onClick={() => {
            if (sharing) share.mutate(false);
            else void localNetwork.guard(null, () => share.mutate(true));
          }}
          className={`relative h-7 w-12 shrink-0 rounded-full transition-colors disabled:opacity-60 ${sharing ? 'bg-primary' : 'bg-background'}`}
        >
          <span
            aria-hidden
            className={`bg-foreground absolute top-1 size-5 rounded-full transition-all ${sharing ? 'left-6' : 'left-1'}`}
          />
        </button>
      </div>
      <p className="text-muted text-xs">{t(sharing ? 'link.sharingOn' : 'link.sharingOff')}</p>

      <div className="flex flex-col">
        <h3 className="text-muted text-xs font-semibold">{t('link.linked')}</h3>
        {linked.length === 0 ? (
          <p className="text-muted py-2 text-xs">{t('link.noneLinked')}</p>
        ) : (
          <ul className="divide-background flex flex-col divide-y">
            {linked.map((device) =>
              row(
                device,
                <>
                  <button
                    type="button"
                    disabled={play.isPending}
                    onClick={() => play.mutate(device.id)}
                    className={`${BUTTON} bg-primary text-background`}
                  >
                    <Play aria-hidden size={13} className="fill-current" />
                    {t('link.playThere')}
                  </button>
                  <button
                    type="button"
                    disabled={syncNow.isPending}
                    onClick={() => device.syncId && syncNow.mutate(device.syncId)}
                    className={`${BUTTON} bg-background`}
                  >
                    <RefreshCw
                      aria-hidden
                      size={13}
                      className={syncNow.isPending ? 'animate-spin' : ''}
                    />
                    {t('link.syncNow')}
                  </button>
                  <button
                    type="button"
                    aria-label={t('link.unlink', { name: device.name })}
                    disabled={forget.isPending}
                    onClick={() => forget.mutate(device.id)}
                    className={`${BUTTON} bg-background`}
                  >
                    <Unlink aria-hidden size={13} />
                  </button>
                </>,
                [since(device.lastSyncAt), device.found ? t('link.here') : t('link.away')].join(
                  ' · ',
                ),
              ),
            )}
          </ul>
        )}
      </div>

      <div className="flex flex-col">
        <h3 className="text-muted text-xs font-semibold">{t('link.nearby')}</h3>
        {nearby.length === 0 ? (
          <p className="text-muted py-2 text-xs">
            {sharing ? t('link.noneNearby') : t('link.turnOnToFind')}
          </p>
        ) : (
          <ul className="divide-background flex flex-col divide-y">
            {nearby.map((device) =>
              row(
                device,
                asking === device.id ? (
                  <span className="text-muted flex items-center gap-1.5 text-xs">
                    <LoaderCircle aria-hidden size={13} className="animate-spin" />
                    {t('link.waiting', { name: device.name })}
                  </span>
                ) : (
                  <button
                    type="button"
                    disabled={link.isPending}
                    onClick={() => void localNetwork.guard(null, () => link.mutate(device.id))}
                    className={`${BUTTON} bg-primary text-background`}
                  >
                    <Link2 aria-hidden size={13} />
                    {t('link.link')}
                  </button>
                ),
                device.found ? device.address : t('link.away'),
              ),
            )}
          </ul>
        )}
        <form onSubmit={submit} className="mt-1 flex items-center gap-2">
          <input
            value={address}
            onChange={(event) => setAddress(event.target.value)}
            aria-label={t('link.address')}
            placeholder={t('link.addressHint')}
            className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
          />
          <button
            type="submit"
            disabled={!address.trim() || add.isPending}
            className={`${BUTTON} bg-background py-2`}
          >
            <Plus aria-hidden size={13} />
            {t('link.add')}
          </button>
        </form>
      </div>
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
      {localNetwork.sheet}
    </section>
  );
};
