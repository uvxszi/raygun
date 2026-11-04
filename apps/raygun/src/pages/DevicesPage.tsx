import type { SyncKind, SyncStatus } from '@raygun/bindings';
import { Copy, LoaderCircle, RefreshCw, Undo2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useJoinDevice,
  useOpenPairing,
  useRemovePeer,
  useRenameDevice,
  useSetSyncKinds,
  useSyncNow,
  useSyncStatus,
  useUndoLastSync,
} from '../devices/useDeviceSync';
import { PageHeader } from './PageHeader';
import { TextField } from '../shell/TextField';
import { LinkedDevices } from '../devices/LinkedDevices';
import { RemoteCard } from '../settings/RemoteCard';

const KINDS: SyncKind[] = ['settings', 'library', 'history'];
const CARD = 'bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm';

const ErrorText: FC<{ error: Error | null | undefined }> = ({ error }) =>
  error ? (
    <p role="alert" className="text-primary text-sm wrap-anywhere">
      {error.message}
    </p>
  ) : null;

const ThisDevice: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t } = useTranslation();
  const rename = useRenameDevice();
  const [name, setName] = useState<string | null>(null);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (name !== null) rename.mutate(name, { onSuccess: () => setName(null) });
  };

  return (
    <div className="flex flex-col gap-1">
      <h3 className="text-muted text-xs">{t('devices.thisDevice')}</h3>
      {name === null ? (
        <div className="flex items-center justify-between gap-2">
          <span data-testid="device-name">{status.deviceName}</span>
          <button
            type="button"
            onClick={() => setName(status.deviceName)}
            className="text-primary text-xs"
          >
            {t('devices.rename')}
          </button>
        </div>
      ) : (
        <form onSubmit={submit} className="flex items-center gap-2">
          <TextField
            value={name}
            onChange={setName}
            aria-label={t('devices.deviceName')}
            maxLength={40}
            className="bg-background flex-1 rounded-lg pr-0.5 pl-3"
            inputClassName="py-2"
          />
          <button
            type="submit"
            disabled={!name.trim() || rename.isPending}
            className="bg-primary text-background rounded-full px-4 py-2 font-semibold disabled:opacity-60"
          >
            {t('devices.save')}
          </button>
        </form>
      )}
      <p className="text-muted font-mono text-xs">
        {t('devices.key', { fingerprint: status.fingerprint })}
      </p>
      <ErrorText error={rename.error} />
    </div>
  );
};

const PairDevice: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t } = useTranslation();
  const open = useOpenPairing();
  const [copied, setCopied] = useState(false);
  const text = status.pairingText;

  return (
    <section className={CARD} aria-labelledby="pair-title">
      <h2 id="pair-title" className="font-medium">
        {t('devices.pairTitle')}
      </h2>
      {text && status.pairingQrSvg ? (
        <div className="flex flex-col items-center gap-2">
          <img
            alt={t('devices.qrAlt')}
            src={`data:image/svg+xml;utf8,${encodeURIComponent(status.pairingQrSvg)}`}
            className="size-44 rounded-lg bg-white p-1"
          />
          <p className="text-muted text-center text-xs">{t('devices.pairHint')}</p>
          <code
            data-testid="pairing-text"
            className="bg-background w-full rounded-lg p-2 text-xs break-all"
          >
            {text}
          </code>
          <button
            type="button"
            onClick={() => void navigator.clipboard?.writeText(text).then(() => setCopied(true))}
            className="text-primary flex items-center gap-1 text-xs"
          >
            <Copy aria-hidden size={14} />
            {copied ? t('devices.copied') : t('devices.copyText')}
          </button>
        </div>
      ) : (
        <p className="text-muted text-xs">{t('devices.pairExplain')}</p>
      )}
      <button
        type="button"
        disabled={open.isPending}
        onClick={() => {
          setCopied(false);
          open.mutate(undefined);
        }}
        className="bg-background self-start rounded-full px-4 py-2 disabled:opacity-60"
      >
        {text ? t('devices.newCode') : t('devices.showCode')}
      </button>
      <ErrorText error={open.error} />
    </section>
  );
};

const JoinDevice: FC = () => {
  const { t } = useTranslation();
  const join = useJoinDevice();
  const [text, setText] = useState('');

  const submit = (event: FormEvent) => {
    event.preventDefault();
    join.mutate(text.trim(), { onSuccess: () => setText('') });
  };

  return (
    <form onSubmit={submit} className={CARD} aria-labelledby="join-title">
      <h2 id="join-title" className="font-medium">
        {t('devices.joinTitle')}
      </h2>
      <p className="text-muted text-xs">{t('devices.joinHint')}</p>
      <textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        aria-label={t('devices.pairingText')}
        placeholder="raygun-pair:…"
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        rows={3}
        className="bg-background rounded-lg px-3 py-2 font-mono text-xs outline-none"
      />
      <button
        type="submit"
        disabled={!text.trim() || join.isPending}
        className="bg-primary text-background flex items-center gap-2 self-start rounded-full px-5 py-2 font-semibold disabled:opacity-60"
      >
        {join.isPending && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
        {t('devices.join')}
      </button>
      <ErrorText error={join.error} />
    </form>
  );
};

const PairedDevices: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t, i18n } = useTranslation();
  const syncNow = useSyncNow();
  const remove = useRemovePeer();
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <section className={CARD} aria-labelledby="peers-title">
      <div className="flex items-center justify-between">
        <h2 id="peers-title" className="font-medium">
          {t('devices.paired')}
        </h2>
        {status.peers.length > 1 && (
          <button
            type="button"
            disabled={syncNow.isPending}
            onClick={() => syncNow.mutate(null)}
            className="text-primary text-xs"
          >
            {t('devices.syncAll')}
          </button>
        )}
      </div>
      {status.peers.length === 0 ? (
        <p className="text-muted text-xs">{t('devices.noPeers')}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {status.peers.map((peer) => (
            <li key={peer.deviceId} data-testid="peer" className="flex items-center gap-2">
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{peer.name}</span>
                <span className="text-muted font-mono text-xs">{peer.fingerprint}</span>
                <span className="text-muted text-xs">
                  {peer.lastSyncAt
                    ? t('devices.lastSync', { time: time.format(peer.lastSyncAt) })
                    : t('devices.neverSynced')}
                </span>
              </span>
              <button
                type="button"
                aria-label={t('devices.syncWith', { name: peer.name })}
                disabled={syncNow.isPending}
                onClick={() => syncNow.mutate(peer.deviceId)}
                className="text-primary grid size-9 place-items-center rounded-full disabled:opacity-50"
              >
                <RefreshCw
                  aria-hidden
                  size={18}
                  className={
                    syncNow.isPending && syncNow.variables === peer.deviceId ? 'animate-spin' : ''
                  }
                />
              </button>
              <button
                type="button"
                aria-label={t('devices.remove', { name: peer.name })}
                disabled={remove.isPending}
                onClick={() => remove.mutate(peer.deviceId)}
                className="text-muted grid size-9 place-items-center rounded-full"
              >
                <X aria-hidden size={18} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <ErrorText error={syncNow.error ?? remove.error} />
    </section>
  );
};

const WhatSyncs: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t } = useTranslation();
  const setKinds = useSetSyncKinds();
  const toggle = (kind: SyncKind) => {
    const kinds = status.kinds.includes(kind)
      ? status.kinds.filter((existing) => existing !== kind)
      : [...status.kinds, kind];
    setKinds.mutate(kinds);
  };
  return (
    <section className={CARD} aria-labelledby="kinds-title">
      <h2 id="kinds-title" className="font-medium">
        {t('devices.whatSyncs')}
      </h2>
      {KINDS.map((kind) => (
        <label key={kind} className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={status.kinds.includes(kind)}
            disabled={setKinds.isPending}
            onChange={() => toggle(kind)}
            className="accent-primary size-4"
          />
          {t(`devices.kind.${kind}`)}
        </label>
      ))}
      <ErrorText error={setKinds.error} />
    </section>
  );
};

const SyncLog: FC<{ status: SyncStatus }> = ({ status }) => {
  const { t, i18n } = useTranslation();
  const undo = useUndoLastSync();
  const time = new Intl.DateTimeFormat(i18n.language, { timeStyle: 'short', dateStyle: 'short' });
  const undoable = status.log.some((session) => !session.undone && session.changed > 0);
  if (status.log.length === 0) return null;

  return (
    <section className={CARD} aria-labelledby="log-title">
      <div className="flex items-center justify-between">
        <h2 id="log-title" className="font-medium">
          {t('devices.log')}
        </h2>
        {undoable && (
          <button
            type="button"
            disabled={undo.isPending}
            onClick={() => undo.mutate(undefined)}
            className="text-primary flex items-center gap-1 text-xs"
          >
            <Undo2 aria-hidden size={14} />
            {t('devices.undoLast')}
          </button>
        )}
      </div>
      <ul className="flex flex-col gap-2">
        {status.log.map((session) => (
          <li key={session.id} data-testid="sync-session" className="flex flex-col">
            <span className="flex justify-between gap-2">
              <span className="truncate">{session.peerName}</span>
              <span className="text-muted shrink-0 text-xs">
                {time.format(session.startedAt ?? 0)}
              </span>
            </span>
            <span className={`text-xs ${session.error ? 'text-primary' : 'text-muted'}`}>
              {session.error ??
                (session.finished
                  ? t('devices.sessionSummary', {
                      sent: session.sent,
                      received: session.received,
                      changed: session.changed,
                    })
                  : t('devices.syncing'))}
              {session.undone && ` · ${t('devices.undone')}`}
            </span>
          </li>
        ))}
      </ul>
      <ErrorText error={undo.error} />
    </section>
  );
};

/**
 * Devices (M21; one link since M46.47): your devices first (linking, playing on one, syncing),
 * then this device's name, what syncs and the log, linking with a code, and the web remote.
 */
export const DevicesPage: FC = () => {
  const { t } = useTranslation();
  const status = useSyncStatus();
  const data = status.data;

  return (
    <section data-testid="devices-page" className="flex max-w-2xl flex-col gap-4">
      <PageHeader back fallback="/more" title={t('devices.title')} />
      <LinkedDevices />
      <section className={CARD}>
        {data && <ThisDevice status={data} />}
        {(data?.error || status.error) && (
          <p role="alert" className="text-primary wrap-anywhere">
            {data?.error ?? status.error?.message}
          </p>
        )}
      </section>
      {data?.enabled && (
        <>
          <WhatSyncs status={data} />
          <SyncLog status={data} />
          <details className={CARD}>
            <summary className="cursor-pointer font-medium">{t('link.withCode')}</summary>
            <p className="text-muted mt-2 text-xs">{t('link.withCodeHint')}</p>
            <div className="mt-3 flex flex-col gap-4">
              <PairedDevices status={data} />
              <PairDevice status={data} />
              <JoinDevice />
            </div>
          </details>
        </>
      )}
      <RemoteCard browserOnly />
    </section>
  );
};
