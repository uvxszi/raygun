import type { CatalogEntry } from '@raygun/bindings';
import { Download, LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { SourceIssue } from '../shell/SourceIssue';
import { useMihonApk } from './useExtensions';

/**
 * A Mihon extension Raygun can't convert, while Allow running source code is on: its APK can be
 * downloaded and checked against its store. Running it isn't available on this device yet,
 * which the answer says.
 */
export const MihonApkAction: FC<{ entry: CatalogEntry }> = ({ entry }) => {
  const { t } = useTranslation();
  const allowed = useSettings().data?.runSourceCode ?? false;
  const download = useMihonApk();
  if (!allowed || !entry.unsupported || !entry.id.startsWith('mihon.')) return null;
  if (download.isSuccess) {
    return (
      <span role="status" className="text-muted text-xs">
        {t('extensions.apk.saved', { size: Math.max(1, Math.round(download.data.size / 1024)) })}{' '}
        {t('extensions.apk.notAvailable')}
      </span>
    );
  }
  return (
    <span className="flex flex-col items-start gap-1">
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
      {download.isError && (
        <SourceIssue compact title={t('extensions.apk.failed')}>
          {String(download.error)}
        </SourceIssue>
      )}
    </span>
  );
};
