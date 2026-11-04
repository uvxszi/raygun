import { commands } from '@raygun/bindings';
import type { ConnectDevice, SyncPeerInfo } from '@raygun/bindings';
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
import type { FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAddDevice,
  useConnectDevices,
  useConnectSession,
  useConnectStatus,
  useConnectTo,
  useDisconnect,
  useForgetDevice,
} from '../connect/useConnect';
import { unwrapResult } from '../lib/commandResult';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { ConfirmSheet } from '../settings/ConfirmSheet';
import { PrefAction, PrefGroup, PrefSwitch } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useRemoteStatus } from '../settings/remoteQueries';
import { Sheet } from '../shell/Sheet';
import { TextField } from '../shell/TextField';
import { useSince } from './since';
import { useRemovePeer, useSyncNow, useSyncStatus } from './useDeviceSync';

const BUTTON =
  'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-50';

const KindIcon: FC<{ kind: string }> = ({ kind }) =>
  kind === 'phone' ? <Smartphone aria-hidden size={20} /> : <Laptop aria-hidden size={20} />;

/** A linked device: one Connect knows (it plays there too), or one linked by code (sync only). */
type Linked = { kind: 'connect'; device: ConnectDevice } | { kind: 'sync'; peer: SyncPeerInfo };

const DeviceRow: FC<{
  name: string;
  kind: string;
  detail: string;
  testId: string;
  children: ReactNode;
}> = ({ name, kind, detail, testId, children }) => (
  <li data-testid={testId} className="flex flex-wrap items-center gap-3 px-1 py-2.5">
    <span className="text-muted">
      <KindIcon kind={kind} />
    </span>
    {/* Wide enough for the name: the buttons go under it on a narrow phone. */}
    <span className="flex min-w-[9rem] flex-1 flex-col">
      <span className="truncate font-medium">{name}</span>
      <span className="text-muted truncate text-xs">{detail}</span>
    </span>
    <span className="flex flex-wrap gap-1.5">{children}</span>
  </li>
);

/** Adds a device by the address it shows, where devices don't find each other (guest Wi-Fi). */
const AddByAddress: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const add = useAddDevice();
  const network = useLocalNetworkGate('connect');
  const [address, setAddress] = useState('');
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const text = address.trim();
    if (!text) return;
    const url = /^https?:\/\//.test(text) ? text : `http://${text}`;
    void network.guard(url, () => add.mutate(text, { onSuccess: onClose }));
  };
  return (
    <Sheet title={t('link.addTitle')} onClose={onClose}>
      <form onSubmit={submit} className="flex flex-col gap-3 text-sm">
        <TextField
          autoFocus
          type="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          value={address}
          onChange={setAddress}
          aria-label={t('link.address')}
          placeholder={t('link.addressHint')}
          className="bg-background rounded-xl pr-0.5 pl-4"
          inputClassName="py-3"
        />
        {add.error && <Problem title={t('link.addFailed')} detail={add.error.message} />}
        <button
          type="submit"
          disabled={!address.trim() || add.isPending}
          className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          {t('link.add')}
        </button>
      </form>
      {network.sheet}
    </Sheet>
  );
};

/**
 * Your devices (M46.47): Connect, sync and the remote as one. One switch lets other devices find
 * this one; linking asks the other device once, and then the two keep each other in sync (on
 * every change, and every few minutes) and either can play on the other, like Spotify's device
 * picker. Devices linked with a code before sync only.
 */
export const LinkedDevices: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const remote = useRemoteStatus();
  const sync = useSyncStatus();
  const devices = useConnectDevices(true);
  const status = useConnectStatus().data;
  const session = useConnectSession();
  const localNetwork = useLocalNetworkGate('remote');
  const since = useSince();
  const [adding, setAdding] = useState(false);
  const [unlinking, setUnlinking] = useState<Linked | null>(null);
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
    mutationFn: (device: ConnectDevice) => unwrapResult(commands.connectLink(device.id)),
    onSettled: refresh,
  });
  const play = useConnectTo();
  const playHere = useDisconnect();
  const forget = useForgetDevice();
  const removePeer = useRemovePeer();
  const syncNow = useSyncNow();

  const all = devices.data ?? [];
  const connectLinked = all.filter((device) => device.syncId);
  const linkedSyncIds = new Set(connectLinked.map((device) => device.syncId));
  const linked: Linked[] = [
    ...connectLinked.map((device) => ({ kind: 'connect' as const, device })),
    // Linked with a code (M21): sync only, Connect doesn't know them.
    ...(sync.data?.peers ?? [])
      .filter((peer) => !linkedSyncIds.has(peer.deviceId))
      .map((peer) => ({ kind: 'sync' as const, peer })),
  ];
  const nearby = all.filter((device) => !device.syncId);
  const asking = status?.asking;
  const nameOf = (item: Linked) => (item.kind === 'connect' ? item.device.name : item.peer.name);
  // A device by its Connect or sync id, for what went wrong with it.
  const deviceName = (id: string | null | undefined) =>
    all.find((device) => device.id === id || device.syncId === id)?.name ??
    sync.data?.peers.find((peer) => peer.deviceId === id)?.name ??
    t('link.thatDevice');

  // What went wrong last, said for the device it was about.
  const failure = share.error
    ? { title: t('link.shareFailed'), detail: share.error.message }
    : link.error
      ? {
          title: t('link.linkFailed', { name: link.variables?.name ?? '' }),
          detail: link.error.message,
        }
      : play.error
        ? {
            title: t('link.playFailed', { name: deviceName(play.variables) }),
            detail: play.error.message,
          }
        : syncNow.error
          ? {
              title: t('link.syncFailed', { name: deviceName(syncNow.variables) }),
              detail: syncNow.error.message,
            }
          : (forget.error ?? removePeer.error)
            ? {
                title: t('link.unlinkFailed'),
                detail: (forget.error ?? removePeer.error)?.message,
              }
            : null;

  const syncButton = (syncId: string) => {
    const running = syncNow.isPending && syncNow.variables === syncId;
    return (
      <button
        type="button"
        disabled={syncNow.isPending}
        onClick={() => syncNow.mutate(syncId)}
        className={`${BUTTON} bg-background`}
      >
        <RefreshCw aria-hidden size={13} className={running ? 'animate-spin' : ''} />
        {t('link.syncNow')}
      </button>
    );
  };
  const unlinkButton = (item: Linked) => (
    <button
      type="button"
      aria-label={t('link.unlink', { name: nameOf(item) })}
      title={t('link.unlink', { name: nameOf(item) })}
      disabled={forget.isPending || removePeer.isPending}
      onClick={() => setUnlinking(item)}
      className={`${BUTTON} bg-background`}
    >
      <Unlink aria-hidden size={13} />
    </button>
  );

  return (
    <div data-testid="linked-devices" className="flex flex-col">
      <PrefGroup>
        <PrefSwitch
          title={t('link.sharing')}
          checked={sharing}
          disabled={share.isPending || localNetwork.checking || !remote.data || !sync.data}
          onChange={(on) => {
            if (!on) share.mutate(false);
            else void localNetwork.guard(null, () => share.mutate(true));
          }}
        />
      </PrefGroup>

      <PrefGroup title={t('link.linked')}>
        {linked.length === 0 && (
          <li className="text-muted px-1 py-2 text-sm">{t('link.noneLinked')}</li>
        )}
        {linked.map((item) => {
          if (item.kind === 'sync') {
            return (
              <DeviceRow
                key={`sync:${item.peer.deviceId}`}
                testId="linked-device"
                name={item.peer.name}
                kind="computer"
                detail={[since(item.peer.lastSyncAt), t('link.syncOnly')].join(' · ')}
              >
                {syncButton(item.peer.deviceId)}
                {unlinkButton(item)}
              </DeviceRow>
            );
          }
          const { device } = item;
          const playingThere = session?.deviceId === device.id;
          return (
            <DeviceRow
              key={device.id}
              testId="linked-device"
              name={device.name}
              kind={device.kind}
              detail={[
                playingThere ? t('link.playingThere') : since(device.lastSyncAt),
                device.found ? t('link.here') : t('link.away'),
              ].join(' · ')}
            >
              {playingThere ? (
                <button
                  type="button"
                  disabled={playHere.isPending}
                  onClick={() => playHere.mutate(true)}
                  className={`${BUTTON} bg-primary text-background`}
                >
                  <Play aria-hidden size={13} className="fill-current" />
                  {t('link.playHere')}
                </button>
              ) : (
                <button
                  type="button"
                  disabled={play.isPending}
                  onClick={() => play.mutate(device.id)}
                  className={`${BUTTON} bg-primary text-background`}
                >
                  <Play aria-hidden size={13} className="fill-current" />
                  {t('link.playThere')}
                </button>
              )}
              {device.syncId && syncButton(device.syncId)}
              {unlinkButton(item)}
            </DeviceRow>
          );
        })}
      </PrefGroup>

      <PrefGroup title={t('link.nearby')}>
        {nearby.length === 0 && (
          <li className="text-muted px-1 py-2 text-sm">
            {sharing ? t('link.noneNearby') : t('link.turnOnToFind')}
          </li>
        )}
        {nearby.map((device) => (
          <DeviceRow
            key={device.id}
            testId="nearby-device"
            name={device.name}
            kind={device.kind}
            detail={device.found ? device.address : t('link.away')}
          >
            {asking === device.id ? (
              <span className="text-muted flex items-center gap-1.5 text-xs">
                <LoaderCircle aria-hidden size={13} className="animate-spin" />
                {t('link.waiting', { name: device.name })}
              </span>
            ) : (
              <button
                type="button"
                disabled={link.isPending}
                onClick={() => void localNetwork.guard(null, () => link.mutate(device))}
                className={`${BUTTON} bg-primary text-background`}
              >
                <Link2 aria-hidden size={13} />
                {t('link.link')}
              </button>
            )}
          </DeviceRow>
        ))}
        <PrefAction icon={Plus} title={t('link.addTitle')} onClick={() => setAdding(true)} />
      </PrefGroup>

      {failure && (
        <div className="px-1 py-2">
          <Problem title={failure.title} detail={failure.detail} />
        </div>
      )}
      {adding && <AddByAddress onClose={() => setAdding(false)} />}
      {unlinking && (
        <ConfirmSheet
          title={t('link.unlinkQuestion', { name: nameOf(unlinking) })}
          detail={t('link.unlinkDetail')}
          confirm={t('link.unlinkConfirm')}
          onConfirm={() =>
            unlinking.kind === 'connect'
              ? forget.mutate(unlinking.device.id, { onSettled: refresh })
              : removePeer.mutate(unlinking.peer.deviceId)
          }
          onClose={() => setUnlinking(null)}
        />
      )}
      {localNetwork.sheet}
    </div>
  );
};
