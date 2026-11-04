import type { AppUpdate } from '@raygun/bindings';
import { Download, ExternalLink, FolderGit2, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefAction, PrefGroup } from '../settings/prefs';
import { Problem } from '../settings/Problem';
import { useAppUpdate, useAppUpdateActions } from './useAppUpdate';

/** What the main button does for this copy: install it, or open the release page. */
const selfUpdates = (update: AppUpdate) => update.assetName !== null;

/** "Checked 5 minutes ago", in the reader's language. */
const useCheckedWhen = () => {
  const { t, i18n } = useTranslation();
  const relative = new Intl.RelativeTimeFormat(i18n.language, { numeric: 'auto' });
  return (at: number | null | undefined) => {
    if (!at) return null;
    const minutes = Math.round((at - Date.now()) / 60_000);
    const when =
      Math.abs(minutes) < 60
        ? relative.format(minutes, 'minute')
        : Math.abs(minutes) < 48 * 60
          ? relative.format(Math.round(minutes / 60), 'hour')
          : relative.format(Math.round(minutes / 1440), 'day');
    return t('updater.checked', { when });
  };
};

/**
 * About → Updates (M46.47): this version, the newest release with its notes, and Update, which
 * downloads and installs it where this copy can (the setup exe, the MSI, Android) or opens the
 * release page (the portable exe). The repository is a tap away.
 */
export const AppUpdateCard: FC = () => {
  const { t } = useTranslation();
  const update = useAppUpdate();
  const { check, install, openPage } = useAppUpdateActions();
  const checkedWhen = useCheckedWhen();
  const [notesOpen, setNotesOpen] = useState(false);
  const info = update.data;
  if (!info) return null;
  const busy =
    info.state === 'checking' || info.state === 'downloading' || info.state === 'installing';
  const failure = check.error
    ? { title: t('updater.checkFailed'), detail: check.error.message }
    : install.error
      ? { title: t('updater.installFailed'), detail: install.error.message }
      : openPage.error
        ? { title: t('updater.openFailed'), detail: openPage.error.message }
        : info.state === 'failed' && info.message
          ? { title: t('updater.failed'), detail: info.message }
          : null;
  const status =
    info.available && info.latest
      ? t('updater.available', { latest: info.latest, current: info.current })
      : info.latest
        ? t('updater.newest', { current: info.current })
        : t('updater.notChecked', { current: info.current });

  return (
    <div data-testid="app-update">
      <PrefGroup title={t('updater.title')}>
        <li className="flex flex-col gap-0.5 px-1 py-3">
          <span data-testid="update-status" className="font-medium">
            {status}
          </span>
          {info.state === 'idle' && info.message ? (
            <span data-testid="update-note" className="text-muted text-sm">
              {info.message}
            </span>
          ) : (
            checkedWhen(info.checkedAt) && (
              <span className="text-muted text-sm">{checkedWhen(info.checkedAt)}</span>
            )
          )}
        </li>
        {info.state === 'downloading' && (
          <li className="px-1 pb-2">
            <div
              role="progressbar"
              aria-label={t('updater.downloading')}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round((info.progress ?? 0) * 100)}
              className="bg-surface h-1.5 overflow-hidden rounded-full"
            >
              <div
                className="bg-primary h-full origin-left transition-transform"
                style={{ transform: `scaleX(${info.progress ?? 0})` }}
              />
            </div>
          </li>
        )}
        {info.state === 'installing' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.installing')}</li>
        )}
        {info.state === 'needsPermission' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.needsPermission')}</li>
        )}
        {info.available && info.installKind === 'portable' && (
          <li className="text-muted px-1 py-2 text-sm">{t('updater.portableHint')}</li>
        )}
        {info.available ? (
          <PrefAction
            icon={selfUpdates(info) ? Download : ExternalLink}
            title={
              selfUpdates(info)
                ? info.state === 'needsPermission'
                  ? t('updater.tryAgain')
                  : t('updater.install', { latest: info.latest })
                : t('updater.openRelease', { latest: info.latest })
            }
            disabled={busy || install.isPending}
            onClick={() => install.mutate()}
          />
        ) : (
          <PrefAction
            icon={RefreshCw}
            title={t('updater.check')}
            disabled={busy || check.isPending}
            onClick={() => check.mutate()}
          />
        )}
        {info.available && info.notes && (
          <li className="flex flex-col gap-1 px-1 py-2">
            <button
              type="button"
              aria-expanded={notesOpen}
              onClick={() => setNotesOpen(!notesOpen)}
              className="text-primary w-fit text-sm font-semibold"
            >
              {t(notesOpen ? 'updater.hideNotes' : 'updater.showNotes')}
            </button>
            {notesOpen && (
              <pre className="selectable text-muted max-h-64 overflow-y-auto font-sans text-sm whitespace-pre-wrap">
                {info.notes}
              </pre>
            )}
          </li>
        )}
        {failure && (
          <li className="px-1 py-2">
            <Problem title={failure.title} detail={failure.detail} />
          </li>
        )}
        <PrefAction
          icon={FolderGit2}
          title={t('updater.repo')}
          onClick={() => openPage.mutate(info.repoUrl)}
        />
      </PrefGroup>
    </div>
  );
};
