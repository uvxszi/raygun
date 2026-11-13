import { commands } from '@raygun/bindings';
import type { ConnectDevice, SyncPeerInfo } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Laptop,
  Link2,
  LoaderCircle,
  Play,
  QrCode,
  RefreshCw,
  ScanLine,
  Smartphone,
  Unlink,
} from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useConnectDevices,
  useConnectSession,
  useConnectStatus,
  useConnectTo,
  useDisconnect,
  useForgetDevice,
} from '../connect/useConnect';
import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { ConfirmSheet } from '../settings/ConfirmSheet';
import { PrefAction, PrefGroup, PrefSwitch } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useRemoteStatus } from '../settings/remoteQueries';
import { ScanLinkCode, ShowLinkCode } from './LinkSheets';
import { useSince } from './since';
import { TAP_BUTTON, TAP_ICON, TAP_NOTE } from './tapTargets';
import { useRemovePeer, useSyncNow, useSyncStatus } from './useDeviceSync';

const KindIcon: FC<{ kind: string }> = ({ kind }) =>
  kind === 'phone' ? <Smartphone aria-hidden size={24} /> : <Laptop aria-hidden size={24} />;

/** A linked device: one Connect knows (it plays there too), or one linked by code (sync only). */
type Linked = { kind: 'connect'; device: ConnectDevice } | { kind: 'sync'; peer: SyncPeerInfo };

const DeviceRow: FC<{
  name: string;
  kind: string;
  detail: string;
  testId: string;
  children: ReactNode;
}> = ({ name, kind, detail, testId, children }) => (
  <li data-testid={testId} className="flex min-h-14 flex-wrap items-center gap-3 px-1 py-3">
    <span className="text-muted shrink-0">
      <KindIcon kind={kind} />
    </span>
    {/* Wide enough for the name: the buttons go under it on a narrow phone. */}
    <span className="flex min-w-[9rem] flex-1 flex-col gap-0.5">
      <span className="truncate text-base font-medium">{name}</span>
      <span className={TAP_NOTE}>{detail}</span>
    </span>
    <span className="flex flex-wrap gap-2">{children}</span>
  </li>
);

/**
 * Your devices (M46.47): Connect, sync and the remote as one. One switch lets other devices find
 * this one; linking asks the other device once, and then the two keep each other in sync (on
 * every change, and every few minutes) and either can play on the other, like Spotify's device
 * picker. Linking is by QR code (one shows it, the other scans it) or with a device found on this
 * network. Devices linked with a code before sync only.
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
  const [linking, setLinking] = useState<'show' | 'scan' | null>(null);
  const [unlinking, setUnlinking] = useState<Linked | null>(null);
  // Raygun on the web (docs/WEB.md) can't be reached by other devices: it links by scanning
  // their code, and syncs when it connects.
  const web = usePlatform() === 'web';
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
        className={`${TAP_BUTTON} bg-background`}
      >
        <RefreshCw aria-hidden size={16} className={running ? 'animate-spin' : ''} />
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
      className={`${TAP_ICON} bg-background`}
    >
      <Unlink aria-hidden size={18} />
    </button>
  );

  return (
    <div data-testid="linked-devices" className="flex flex-col">
      {web && <p className="text-muted px-1 pb-3 text-sm">{t('link.webNote')}</p>}
      {!web && (
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
      )}

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
                  className={`${TAP_BUTTON} bg-primary text-background`}
                >
                  <Play aria-hidden size={16} className="fill-current" />
                  {t('link.playHere')}
                </button>
              ) : (
                <button
                  type="button"
                  disabled={play.isPending}
                  onClick={() => play.mutate(device.id)}
                  className={`${TAP_BUTTON} bg-primary text-background`}
                >
                  <Play aria-hidden size={16} className="fill-current" />
                  {t('link.playThere')}
                </button>
              )}
              {device.syncId && syncButton(device.syncId)}
              {unlinkButton(item)}
            </DeviceRow>
          );
        })}
      </PrefGroup>

      {!web && (
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
                <span className="text-muted flex min-h-11 items-center gap-2 text-sm">
                  <LoaderCircle aria-hidden size={16} className="animate-spin" />
                  {t('link.waiting', { name: device.name })}
                </span>
              ) : (
                <button
                  type="button"
                  disabled={link.isPending}
                  onClick={() => void localNetwork.guard(null, () => link.mutate(device))}
                  className={`${TAP_BUTTON} bg-primary text-background`}
                >
                  <Link2 aria-hidden size={16} />
                  {t('link.link')}
                </button>
              )}
            </DeviceRow>
          ))}
        </PrefGroup>
      )}

      <PrefGroup title={t('link.byCode')}>
        {!web && (
          <PrefAction icon={QrCode} title={t('link.showCode')} onClick={() => setLinking('show')} />
        )}
        <PrefAction icon={ScanLine} title={t('link.scanCode')} onClick={() => setLinking('scan')} />
      </PrefGroup>

      {failure && (
        <div className="px-1 py-2">
          <Problem title={failure.title} detail={failure.detail} />
        </div>
      )}
      {linking === 'show' && <ShowLinkCode onClose={() => setLinking(null)} />}
      {linking === 'scan' && <ScanLinkCode onClose={() => setLinking(null)} />}
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
