import type { SyncKind, SyncStatus } from '@raygun/bindings';
import { Copy, KeyRound, LoaderCircle, Undo2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { LinkedDevices } from '../devices/LinkedDevices';
import {
  useJoinDevice,
  useOpenPairing,
  useRenameDevice,
  useSetSyncKinds,
  useSyncStatus,
  useUndoLastSync,
} from '../devices/useDeviceSync';
import { PrefAction, PrefGroup, PrefSwitch, PrefText } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { RemoteCard } from '../settings/RemoteCard';
import { useFoundPref } from '../settings/useSettingsSearch';
import { Sheet } from '../shell/Sheet';
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

/** For devices that can't find each other: a code shown on one, pasted on the other. */
const LinkWithCode: FC<{ status: SyncStatus; onClose: () => void }> = ({ status, onClose }) => {
  const { t } = useTranslation();
  const open = useOpenPairing();
  const join = useJoinDevice();
  const [copied, setCopied] = useState(false);
  const [text, setText] = useState('');
  const code = status.pairingText;
  const submit = (event: FormEvent) => {
    event.preventDefault();
    join.mutate(text.trim(), { onSuccess: () => setText('') });
  };
  return (
    <Sheet title={t('link.withCode')} onClose={onClose}>
      <div data-testid="link-with-code" className="flex flex-col gap-5 text-sm">
        <section aria-labelledby="pair-title" className="flex flex-col gap-2">
          <h3 id="pair-title" className="font-medium">
            {t('devices.pairTitle')}
          </h3>
          {code && status.pairingQrSvg && (
            <div className="flex flex-col items-center gap-2">
              <img
                alt={t('devices.qrAlt')}
                src={`data:image/svg+xml;utf8,${encodeURIComponent(status.pairingQrSvg)}`}
                className="size-44 rounded-lg bg-white p-1"
              />
              <code
                data-testid="pairing-text"
                className="selectable bg-background w-full rounded-lg p-2 text-xs break-all"
              >
                {code}
              </code>
              <button
                type="button"
                onClick={() =>
                  void navigator.clipboard?.writeText(code).then(() => setCopied(true))
                }
                className="text-primary flex items-center gap-1 text-xs font-semibold"
              >
                <Copy aria-hidden size={14} />
                {copied ? t('devices.copied') : t('devices.copyText')}
              </button>
              <p className="text-muted text-center text-xs">{t('devices.pairHint')}</p>
            </div>
          )}
          <button
            type="button"
            disabled={open.isPending}
            onClick={() => {
              setCopied(false);
              open.mutate(undefined);
            }}
            className="bg-background self-start rounded-full px-4 py-2 font-semibold disabled:opacity-60"
          >
            {code ? t('devices.newCode') : t('devices.showCode')}
          </button>
          {open.error && <Problem title={t('devices.codeFailed')} detail={open.error.message} />}
        </section>
        <form onSubmit={submit} aria-labelledby="join-title" className="flex flex-col gap-2">
          <h3 id="join-title" className="font-medium">
            {t('devices.joinTitle')}
          </h3>
          <textarea
            value={text}
            onChange={(event) => setText(event.target.value)}
            aria-label={t('devices.pairingText')}
            placeholder={t('devices.pairingTextHint')}
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            rows={3}
            className="bg-background placeholder:text-muted rounded-lg px-3 py-2 font-mono text-xs outline-none"
          />
          <button
            type="submit"
            disabled={!text.trim() || join.isPending}
            className="bg-primary text-background flex items-center gap-2 self-start rounded-full px-5 py-2 font-semibold disabled:opacity-60"
          >
            {join.isPending && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
            {t('devices.join')}
          </button>
          {join.error && <Problem title={t('devices.joinFailed')} detail={join.error.message} />}
          {join.isSuccess && (
            <p role="status" className="text-muted text-xs">
              {t('devices.joined')}
            </p>
          )}
        </form>
        <p className="text-muted font-mono text-xs">
          {t('devices.key', { fingerprint: status.fingerprint })}
        </p>
      </div>
    </Sheet>
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
 * Devices (M21; one link since M46.47): your devices first (letting them find this one, linking,
 * playing on one, syncing), then this device's name and what syncs, linking with a code where
 * devices don't find each other, the browser remote and what syncing did.
 */
export const DevicesPage: FC = () => {
  const { t } = useTranslation();
  const status = useSyncStatus();
  const data = status.data;
  const [withCode, setWithCode] = useState(false);
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
      <PrefGroup title={t('devices.moreWays')}>
        <PrefAction
          icon={KeyRound}
          title={t('link.withCode')}
          summary={data?.enabled ? undefined : t('link.turnOnFirst')}
          disabled={!data?.enabled}
          onClick={() => setWithCode(true)}
        />
      </PrefGroup>
      <RemoteCard />
      {data && <SyncHistory status={data} />}
      {withCode && data && <LinkWithCode status={data} onClose={() => setWithCode(false)} />}
    </section>
  );
};
