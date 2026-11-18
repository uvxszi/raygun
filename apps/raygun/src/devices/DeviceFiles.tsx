import { commands } from '@raygun/bindings';
import type {
  DeviceFileKind,
  DeviceFilesPeer,
  DeviceFilesPrefs,
  DeviceFilesTransfer,
} from '@raygun/bindings';
import { Send, Square } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PrefAction, PrefGroup, PrefSwitch } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { FILE_KINDS, useDeviceFiles, useFilesChange } from './useDeviceFiles';
const megabytes = (bytes: number | null) => ((bytes ?? 0) / (1024 * 1024)).toFixed(1);

/** The transfer going on: what, how far, and Stop. */
const TransferLine: FC<{ transfer: DeviceFilesTransfer }> = ({ transfer }) => {
  const { t } = useTranslation();
  const cancel = useFilesChange(() => unwrapResult(commands.deviceFilesCancel()));
  const total = transfer.totalBytes ?? 0;
  const share = total > 0 ? (transfer.doneBytes ?? 0) / total : 0;
  return (
    <PrefGroup title={t('devices.files.transferTitle')}>
      <li data-testid="device-files-transfer" className="flex flex-col gap-1 px-1 py-2">
        <span className="font-medium">
          {t(
            transfer.direction === 'sending'
              ? 'devices.files.sendingTo'
              : 'devices.files.receivingFrom',
            { name: transfer.deviceName },
          )}
        </span>
        <span className="text-muted truncate text-sm">
          {t('devices.files.item', {
            name: transfer.itemName,
            index: transfer.index,
            count: transfer.count,
          })}
        </span>
        <progress className="w-full" max={1} value={share} aria-label={transfer.itemName} />
        <span className="text-muted text-xs">
          {t('devices.files.bytes', {
            done: megabytes(transfer.doneBytes),
            total: megabytes(transfer.totalBytes),
          })}
        </span>
      </li>
      {transfer.direction === 'sending' && (
        <PrefAction
          icon={Square}
          title={t('devices.files.stop')}
          disabled={cancel.isPending}
          onClick={() => cancel.mutate(undefined)}
        />
      )}
    </PrefGroup>
  );
};

/** What goes to one linked device. */
const PeerFiles: FC<{ peer: DeviceFilesPeer }> = ({ peer }) => {
  const { t } = useTranslation();
  const setPrefs = useFilesChange((prefs: DeviceFilesPrefs) =>
    unwrapResult(commands.deviceFilesSetPrefs(peer.deviceId, prefs)),
  );
  const sendNow = useFilesChange(() => unwrapResult(commands.deviceFilesSendNow(peer.deviceId)));
  const change = (prefs: Partial<DeviceFilesPrefs>) => setPrefs.mutate({ ...peer.prefs, ...prefs });
  const toggle = (kind: DeviceFileKind, on: boolean) =>
    change({
      kinds: on ? [...peer.prefs.kinds, kind] : peer.prefs.kinds.filter((each) => each !== kind),
    });
  const anyFiles = peer.prefs.kinds.some((kind) => kind !== 'songEdits');
  const problem = setPrefs.error ?? sendNow.error;
  return (
    <PrefGroup title={t('devices.files.sendTo', { name: peer.name })}>
      {FILE_KINDS.map((kind) => (
        <PrefSwitch
          key={kind}
          title={t(`devices.files.kind.${kind}`)}
          summary={kind === 'songEdits' ? t('devices.files.songEditsSummary') : undefined}
          checked={peer.prefs.kinds.includes(kind)}
          disabled={setPrefs.isPending}
          onChange={(on) => toggle(kind, on)}
        />
      ))}
      <PrefSwitch
        title={t('devices.files.wifiOnly')}
        checked={peer.prefs.wifiOnly}
        disabled={setPrefs.isPending}
        onChange={(wifiOnly) => change({ wifiOnly })}
      />
      {anyFiles && (
        <PrefAction
          icon={Send}
          title={t('devices.files.sendNow')}
          summary={
            peer.note ??
            t('devices.files.counts', { delivered: peer.delivered, waiting: peer.waiting })
          }
          disabled={sendNow.isPending}
          onClick={() => sendNow.mutate(undefined)}
        />
      )}
      {problem && (
        <li className="px-1 py-2">
          <Problem title={t('devices.files.failed')} detail={problem.message} />
        </li>
      )}
    </PrefGroup>
  );
};

/**
 * Files to linked devices (M46.57): per device, which downloads, covers and song edits go, and
 * the transfer going on.
 */
export const DeviceFiles: FC = () => {
  const files = useDeviceFiles();
  const data = files.data;
  if (!data || (data.peers.length === 0 && !data.transfer)) return null;
  return (
    <>
      {data.transfer && <TransferLine transfer={data.transfer} />}
      {data.peers.map((peer) => (
        <PeerFiles key={peer.deviceId} peer={peer} />
      ))}
    </>
  );
};
