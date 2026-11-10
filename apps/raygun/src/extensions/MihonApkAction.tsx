import type { CatalogEntry } from '@raygun/bindings';
import { Download, LoaderCircle, Play, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { SourceIssue } from '../shell/SourceIssue';
import {
  useMihonApk,
  useMihonRun,
  useMihonRuntime,
  useMihonRuntimeCancel,
  useMihonRuntimeSetup,
} from './useExtensions';

/** `eu.kanade.tachiyomi.extension.<lang>.<name>` from `mihon.<lang>.<name>`. */
const packageOf = (id: string) => `eu.kanade.tachiyomi.extension.${id.slice('mihon.'.length)}`;

/**
 * A Mihon extension Raygun can't convert, while Allow running source code is on: its APK is
 * downloaded and checked against its store, then — where the runtime is set up — run as its own
 * code (tier 2). Running isn't available on every device, which the panel says plainly.
 */
export const MihonApkAction: FC<{ entry: CatalogEntry }> = ({ entry }) => {
  const allowed = useSettings().data?.runSourceCode ?? false;
  // The panel's own hooks only for the entries it's for: a store lists thousands of others.
  if (!allowed || !entry.unsupported || !entry.id.startsWith('mihon.')) return null;
  return <MihonApkPanel entry={entry} />;
};

const MihonApkPanel: FC<{ entry: CatalogEntry }> = ({ entry }) => {
  const { t } = useTranslation();
  const download = useMihonApk();
  const runtime = useMihonRuntime();
  const setup = useMihonRuntimeSetup();
  const cancelSetup = useMihonRuntimeCancel();
  const run = useMihonRun();
  const [tried, setTried] = useState<string | null>(null);

  const state = runtime.data;
  const saved = download.isSuccess;

  const tryIt = async () => {
    setTried(null);
    try {
      const pkg = packageOf(entry.id);
      const listed = JSON.parse(await run.mutateAsync({ pkg, method: 'sources', params: {} }));
      const source = Array.isArray(listed) ? listed[0] : undefined;
      if (!source) {
        setTried(t('extensions.apk.noSources'));
        return;
      }
      const page = JSON.parse(
        await run.mutateAsync({ pkg, method: 'popular', params: { source: source.id, page: 1 } }),
      );
      setTried(t('extensions.apk.ran', { count: page.items?.length ?? 0 }));
    } catch (error) {
      setTried(String(error));
    }
  };

  return (
    <span className="flex flex-col items-start gap-1">
      {!saved ? (
        <button
          type="button"
          disabled={download.isPending}
          onClick={() => download.mutate({ repoId: entry.repoId, id: entry.id })}
          className="text-primary flex items-center gap-1.5 text-xs font-semibold disabled:opacity-60"
        >
          {download.isPending ? (
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
          ) : (
            <Download aria-hidden size={14} />
          )}
          {t('extensions.apk.download')}
        </button>
      ) : (
        <span className="text-muted text-xs">
          {t('extensions.apk.saved', { size: Math.max(1, Math.round(download.data.size / 1024)) })}
        </span>
      )}

      {/* The runtime, once the APK is saved. */}
      {saved && state && !state.supported && (
        <span className="text-muted text-xs">{t('extensions.apk.notAvailable')}</span>
      )}
      {saved && state?.supported && !state.ready && (
        <span className="flex items-center gap-3">
          <button
            type="button"
            disabled={setup.isPending || state.settingUp}
            onClick={() => setup.mutate()}
            className="text-primary flex items-center gap-1.5 text-xs font-semibold disabled:opacity-60"
          >
            {(setup.isPending || state.settingUp) && (
              <LoaderCircle aria-hidden size={14} className="animate-spin" />
            )}
            {state.settingUp ? t('extensions.apk.settingUp') : t('extensions.apk.setup')}
          </button>
          {/* Stops the tools' download at once; nothing of it is kept. */}
          {(setup.isPending || state.settingUp) && (
            <button
              type="button"
              aria-label={t('extensions.apk.cancelSetup')}
              disabled={cancelSetup.isPending}
              onClick={() => cancelSetup.mutate()}
              className="text-muted hover:text-foreground flex items-center gap-1 text-xs font-semibold disabled:opacity-60"
            >
              <X aria-hidden size={14} />
              {t('extensions.apk.cancel')}
            </button>
          )}
        </span>
      )}
      {saved && state?.ready && (
        <button
          type="button"
          disabled={run.isPending}
          onClick={tryIt}
          className="text-primary flex items-center gap-1.5 text-xs font-semibold disabled:opacity-60"
        >
          {run.isPending ? (
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
          ) : (
            <Play aria-hidden size={14} />
          )}
          {t('extensions.apk.tryIt')}
        </button>
      )}
      {tried && (
        <span role="status" className="text-muted text-xs">
          {tried}
        </span>
      )}

      {download.isError && (
        <SourceIssue compact title={t('extensions.apk.failed')}>
          {String(download.error)}
        </SourceIssue>
      )}
      {setup.isError && (
        <SourceIssue compact title={t('extensions.apk.setupFailed')}>
          {String(setup.error)}
        </SourceIssue>
      )}
    </span>
  );
};
