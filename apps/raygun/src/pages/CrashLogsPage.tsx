import { commands } from '@raygun/bindings';
import type { CrashRecord } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { FolderOpen, Save, Share2, ShieldAlert, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefAction, PrefGroup, PrefInfo } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { Sheet } from '../shell/Sheet';
import { TopBar } from '../shell/TopBar';

const KINDS = [
  'panic',
  'java',
  'renderer',
  'native',
  'anr',
  'memory',
  'exit',
  'start',
  'page',
  'ui',
] as const;

/** A record's kind as people read it. */
const kindKey = (kind: string) =>
  (KINDS as readonly string[]).includes(kind) ? `crash.kinds.${kind}` : 'crash.kinds.other';

const unwrap = async <T,>(
  result: Promise<{ status: 'ok'; data: T } | { status: 'error'; error: string }>,
): Promise<T> => {
  const answer = await result;
  if (answer.status === 'error') throw new Error(answer.error);
  return answer.data;
};

/** One record, read in full, to share or save. */
const RecordSheet: FC<{ record: CrashRecord; onClose: () => void }> = ({ record, onClose }) => {
  const { t } = useTranslation();
  const text = useQuery({
    queryKey: ['crashLog', record.id],
    queryFn: () => unwrap(commands.crashLog(record.id)),
  });
  const [note, setNote] = useState<string | null>(null);
  const share = useMutation({
    mutationFn: () => unwrap(commands.crashLogShare(record.id)),
    onError: (error) => setNote(String(error)),
  });
  const save = useMutation({
    mutationFn: () => unwrap(commands.crashLogSave(record.id)),
    onSuccess: (place) => setNote(t('crash.savedIn', { place })),
    onError: (error) => setNote(String(error)),
  });
  return (
    <Sheet title={t(kindKey(record.kind))} onClose={onClose}>
      <div className="flex flex-col gap-3">
        <pre
          data-testid="crash-log-text"
          className="selectable bg-surface/70 max-h-[55dvh] overflow-auto rounded-xl p-3 font-mono text-xs break-words whitespace-pre-wrap"
        >
          {text.data ?? (text.error ? String(text.error) : '…')}
        </pre>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => share.mutate()}
            className="bg-primary flex flex-1 items-center justify-center gap-2 rounded-full px-4 py-2.5 font-semibold text-white"
          >
            <Share2 aria-hidden size={16} />
            {t('crash.share')}
          </button>
          <button
            type="button"
            onClick={() => save.mutate()}
            className="bg-surface flex flex-1 items-center justify-center gap-2 rounded-full px-4 py-2.5 font-semibold"
          >
            <Save aria-hidden size={16} />
            {t('crash.save')}
          </button>
        </div>
        {note && (
          <p role="status" className="text-muted text-sm">
            {note}
          </p>
        )}
      </div>
    </Sheet>
  );
};

/**
 * About → Crash logs: what went wrong, kept on this device only (`crash_log.rs`), to read, share
 * or save; the sources held back after crashes; the logs folder.
 */
export const CrashLogsPage: FC = () => {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const search: { id?: string } = useSearch({ strict: false });
  const logs = useQuery({ queryKey: ['crashLogs'], queryFn: () => unwrap(commands.crashLogs()) });
  const appInfo = useQuery({ queryKey: ['appInfo'], queryFn: commands.appInfo });
  const android = appInfo.data?.platform === 'android';
  const [confirmClear, setConfirmClear] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ['crashLogs'] });

  const clear = useMutation({
    mutationFn: () => unwrap(commands.crashLogsClear()),
    onSettled: refresh,
  });
  const turnOn = useMutation({
    mutationFn: (id: string) => unwrap(commands.extensionHoldBack(id, false)),
    onSettled: refresh,
  });
  const saveAll = useMutation({
    mutationFn: () => unwrap(commands.logsSaveAll()),
    onSuccess: (place) => setNote(t('crash.savedIn', { place })),
    onError: (error) => setNote(String(error)),
  });
  const openFolder = useMutation({
    mutationFn: () => unwrap(commands.logsOpenFolder()),
    onError: (error) => setNote(String(error)),
  });

  const records = logs.data?.records ?? [];
  const open = records.find((record) => record.id === search.id) ?? null;
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  const show = (id: string | undefined) =>
    void navigate({ to: '/about/crashes', search: { id }, replace: true });

  return (
    <section data-testid="crash-logs-page" className="flex max-w-2xl flex-col">
      <TopBar back fallback="/about" title={t('crash.logs')} />
      {logs.data?.safeStart.active && (
        <PrefGroup>
          <PrefInfo title={t('crash.safeStart')} summary={t('crash.safeStartSummary')} />
        </PrefGroup>
      )}
      {(logs.data?.heldBack.length ?? 0) > 0 && (
        <PrefGroup title={t('crash.heldBack')} summary={t('crash.heldBackSummary')}>
          {logs.data?.heldBack.map((source) => (
            <PrefAction
              key={source.id}
              icon={ShieldAlert}
              title={source.name}
              summary={t('crash.turnOn')}
              onClick={() => turnOn.mutate(source.id)}
            />
          ))}
        </PrefGroup>
      )}
      <PrefGroup title={t('crash.logs')}>
        {logs.error && (
          <li className="px-1 py-2">
            <Problem title={t('crash.logs')} detail={String(logs.error)} onRetry={refresh} />
          </li>
        )}
        {logs.data && records.length === 0 && (
          <li className="text-muted px-1 py-3 text-sm">{t('crash.none')}</li>
        )}
        {records.map((record) => (
          <li key={record.id}>
            <button
              type="button"
              data-testid="crash-record"
              onClick={() => show(record.id)}
              className="hover:bg-surface/60 flex w-full flex-col items-start gap-0.5 rounded-xl px-1 py-3 text-left"
            >
              <span className="font-medium">{t(kindKey(record.kind))}</span>
              <span className="text-muted text-xs">{time.format(new Date(record.at ?? 0))}</span>
              {record.summary && (
                <span className="text-muted line-clamp-2 text-sm break-all">{record.summary}</span>
              )}
            </button>
          </li>
        ))}
      </PrefGroup>
      <PrefGroup>
        {android ? (
          <PrefAction icon={Save} title={t('crash.saveAll')} onClick={() => saveAll.mutate()} />
        ) : (
          <PrefAction
            icon={FolderOpen}
            title={t('crash.openFolder')}
            summary={logs.data?.folder}
            onClick={() => openFolder.mutate()}
          />
        )}
        {records.length > 0 && (
          <PrefAction
            icon={Trash2}
            danger
            title={confirmClear ? t('crash.clearAllConfirm') : t('crash.clearAll')}
            onClick={() => {
              if (!confirmClear) {
                setConfirmClear(true);
                return;
              }
              setConfirmClear(false);
              clear.mutate();
            }}
          />
        )}
        {note && (
          <li role="status" className="text-muted px-1 py-2 text-sm">
            {note}
          </li>
        )}
      </PrefGroup>
      {open && <RecordSheet record={open} onClose={() => show(undefined)} />}
    </section>
  );
};
