import type { AppUpdate } from '@raygun/bindings';
import { Download, ExternalLink, FolderGit2, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { useAppUpdate, useAppUpdateActions } from './useAppUpdate';

const BUTTON =
  'flex items-center gap-1.5 rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50';

/** What the main button does for this copy: install it, or open the release page. */
const selfUpdates = (update: AppUpdate) => update.assetName !== null;

/**
 * About → Updates (M46.47): this version, the newest release with its notes, and Update, which
 * downloads and installs it where this copy can (the setup exe, the MSI, Android) or opens the
 * release page (the portable exe). The repository is a tap away.
 */
export const AppUpdateCard: FC = () => {
  const { t } = useTranslation();
  const update = useAppUpdate();
  const { check, install, openPage } = useAppUpdateActions();
  const [notesOpen, setNotesOpen] = useState(false);
  const info = update.data;
  if (!info) return null;
  const busy =
    info.state === 'checking' || info.state === 'downloading' || info.state === 'installing';
  const error = check.error ?? install.error ?? openPage.error;
  return (
    <section
      data-testid="app-update"
      aria-label={t('updater.title')}
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('updater.title')}</h2>
      <p data-testid="update-status">
        {info.available && info.latest
          ? t('updater.available', { latest: info.latest, current: info.current })
          : info.latest
            ? t('updater.newest', { current: info.current })
            : t('updater.notChecked', { current: info.current })}
      </p>
      {info.state === 'downloading' && (
        <div
          role="progressbar"
          aria-label={t('updater.downloading')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round((info.progress ?? 0) * 100)}
          className="bg-background h-1.5 overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full transition-[width]"
            style={{ width: `${(info.progress ?? 0) * 100}%` }}
          />
        </div>
      )}
      {info.state === 'idle' && info.message && (
        <p data-testid="update-note" className="text-muted text-xs">
          {info.message}
        </p>
      )}
      {info.state === 'installing' && <p className="text-muted">{t('updater.installing')}</p>}
      {info.state === 'needsPermission' && (
        <p className="text-muted">{t('updater.needsPermission')}</p>
      )}
      {info.available && info.installKind === 'portable' && (
        <p className="text-muted text-xs">{t('updater.portableHint')}</p>
      )}
      <div className="flex flex-wrap gap-2">
        {info.available ? (
          <button
            type="button"
            disabled={busy || install.isPending}
            onClick={() => install.mutate()}
            className={`${BUTTON} bg-primary text-background`}
          >
            {selfUpdates(info) ? (
              <Download aria-hidden size={15} />
            ) : (
              <ExternalLink aria-hidden size={15} />
            )}
            {selfUpdates(info)
              ? info.state === 'needsPermission'
                ? t('updater.tryAgain')
                : t('updater.install', { latest: info.latest })
              : t('updater.openRelease', { latest: info.latest })}
          </button>
        ) : (
          <button
            type="button"
            disabled={busy || check.isPending}
            onClick={() => check.mutate()}
            className={`${BUTTON} bg-background`}
          >
            <RefreshCw aria-hidden size={15} className={check.isPending ? 'animate-spin' : ''} />
            {t('updater.check')}
          </button>
        )}
        <button
          type="button"
          onClick={() => openPage.mutate(info.repoUrl)}
          className={`${BUTTON} bg-background`}
        >
          <FolderGit2 aria-hidden size={15} />
          {t('updater.repo')}
        </button>
      </div>
      {info.available && info.notes && (
        <div className="flex flex-col gap-1">
          <button
            type="button"
            aria-expanded={notesOpen}
            onClick={() => setNotesOpen(!notesOpen)}
            className="text-primary w-fit text-xs font-semibold"
          >
            {t(notesOpen ? 'updater.hideNotes' : 'updater.showNotes')}
          </button>
          {notesOpen && (
            <pre className="text-muted max-h-64 overflow-y-auto font-sans text-xs whitespace-pre-wrap">
              {info.notes}
            </pre>
          )}
        </div>
      )}
      {(error || (info.state === 'failed' && info.message)) && (
        <SourceIssue compact source="GitHub">
          {error?.message ?? info.message}
        </SourceIssue>
      )}
    </section>
  );
};
