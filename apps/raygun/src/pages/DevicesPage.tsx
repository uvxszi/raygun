import type { SyncKind, SyncStatus } from '@raygun/bindings';
import { Undo2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LinkedDevices } from '../devices/LinkedDevices';
import {
  useRenameDevice,
  useSetSyncKinds,
  useSyncStatus,
  useUndoLastSync,
} from '../devices/useDeviceSync';
import { PrefAction, PrefGroup, PrefSwitch, PrefText } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useFoundPref } from '../settings/useSettingsSearch';
import { TopBar } from '../shell/TopBar';

const KINDS: SyncKind[] = ['settings', 'library', 'history'];

/** This device's name, as the others show it, and what linked devices keep the same. */
const ThisDevice: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t } = useTranslation();
  const rename = useRenameDevice();
  const setKinds = useSetSyncKinds();
  const toggle = (kind: SyncKind, on: boolean) =>
    setKinds.mutate(
      on ? [...status.kinds, kind] : status.kinds.filter((existing) => existing !== kind),
    );
  return (
    <>
      <PrefGroup title={t('devices.thisDevice')}>
        <PrefText
          title={t('devices.deviceName')}
          value={status.deviceName}
          check={(text) => (text ? null : t('devices.nameNeeded'))}
          disabled={rename.isPending}
          onChange={(name) => rename.mutate(name)}
        />
        <li className="hidden" data-testid="device-name">
          {status.deviceName}
        </li>
        {rename.error && (
          <li className="px-1 py-2">
            <Problem title={t('devices.renameFailed')} detail={rename.error.message} />
          </li>
        )}
      </PrefGroup>
      <PrefGroup title={t('devices.whatSyncs')}>
        {KINDS.map((kind) => (
          <PrefSwitch
            key={kind}
            title={t(`devices.kind.${kind}`)}
            checked={status.kinds.includes(kind)}
            disabled={setKinds.isPending}
            onChange={(on) => toggle(kind, on)}
          />
        ))}
        {setKinds.error && (
          <li className="px-1 py-2">
            <Problem title={t('devices.kindsFailed')} detail={setKinds.error.message} />
          </li>
        )}
      </PrefGroup>
    </>
  );
};

/** What syncing did lately, and taking the last one back. */
const SyncHistory: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t, i18n } = useTranslation();
  const undo = useUndoLastSync();
  const time = new Intl.DateTimeFormat(i18n.language, { timeStyle: 'short', dateStyle: 'short' });
  const undoable = status.log.some((session) => !session.undone && session.changed > 0);
  if (status.log.length === 0) return null;
  return (
    <PrefGroup title={t('devices.log')}>
      {status.log.map((session) => (
        <li key={session.id} data-testid="sync-session" className="flex flex-col px-1 py-2">
          <span className="flex justify-between gap-2">
            <span className="truncate font-medium">{session.peerName}</span>
            <span className="text-muted shrink-0 text-xs">
              {time.format(session.startedAt ?? 0)}
            </span>
          </span>
          <span className="text-muted text-sm">
            {session.error
              ? t('devices.sessionFailed', { reason: session.error })
              : session.finished
                ? t('devices.sessionSummary', {
                    sent: session.sent,
                    received: session.received,
                    changed: session.changed,
                  })
                : t('devices.syncing')}
            {session.undone && ` · ${t('devices.undone')}`}
          </span>
        </li>
      ))}
      {undoable && (
        <PrefAction
          icon={Undo2}
          title={t('devices.undoLast')}
          disabled={undo.isPending}
          onClick={() => undo.mutate(undefined)}
        />
      )}
      {undo.error && (
        <li className="px-1 py-2">
          <Problem title={t('devices.undoFailed')} detail={undo.error.message} />
        </li>
      )}
    </PrefGroup>
  );
};

/**
 * Devices (M21; one link since M46.47): your devices first (letting them find this one, linking
 * by QR code or on this network, playing on one, syncing), then this device's name and what
 * syncs, and what syncing did.
 */
export const DevicesPage: FC = () => {
  const { t } = useTranslation();
  const status = useSyncStatus();
  const data = status.data;
  useFoundPref();

  return (
    <section data-testid="devices-page" className="flex max-w-2xl flex-col">
      <TopBar back fallback="/settings" title={t('devices.title')} />
      <LinkedDevices />
      {data && <ThisDevice status={data} />}
      {(data?.error || status.error) && (
        <div className="px-1 py-2">
          <Problem title={t('devices.syncProblem')} detail={data?.error ?? status.error?.message} />
        </div>
      )}
      {data && <SyncHistory status={data} />}
    </section>
  );
};
