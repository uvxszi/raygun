import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { Download, ExternalLink } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Problem } from '../settings/Problem';
import { Sheet } from '../shell/Sheet';
import { Markdown } from './Markdown';
import { APP_UPDATE_KEY, useAppUpdate, useAppUpdateActions } from './useAppUpdate';
import { useOpenNotesLink } from './useReleaseNotes';

const BUTTON =
  'flex min-h-11 w-full items-center justify-center gap-2 rounded-full px-4 text-sm font-semibold disabled:opacity-50';

/**
 * A newer Raygun an automatic check found (`AppUpdate::prompt`), offered over whatever is open
 * (not over About, whose card shows it): Download and install where this copy replaces itself
 * (the setup exe, the MSI, Android's installer), else the download page; Later (offered again a
 * day later) and Skip this version (never offered again on this device). Closing it is Later.
 * Once Download and install is pressed it shows the download until the installer takes over.
 */
export const UpdateOffer: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const update = useAppUpdate().data;
  const { install } = useAppUpdateActions();
  const openLink = useOpenNotesLink();
  const onAbout = useRouterState({ select: (state) => state.location.pathname === '/about' });
  // Install was pressed here: the sheet follows the download, whatever `prompt` says.
  const [following, setFollowing] = useState(false);
  const answer = useMutation({
    mutationFn: (choice: 'later' | 'skip') =>
      unwrapResult(choice === 'later' ? commands.appUpdateLater() : commands.appUpdateSkip()),
    onSuccess: (next) => client.setQueryData(APP_UPDATE_KEY, next),
  });
  if (!update?.available || !update.latest) return null;
  const { state } = update;
  const busy = state === 'checking' || state === 'downloading' || state === 'installing';
  const stillFollowing =
    following && (busy || state === 'needsPermission' || state === 'failed' || install.isPending);
  if (!stillFollowing && (!update.prompt || onAbout || answer.isPending)) return null;
  const selfUpdates = update.assetName !== null;
  const failure = install.error?.message ?? (state === 'failed' ? update.message : null);
  const close = () => {
    if (stillFollowing) setFollowing(false);
    else answer.mutate('later');
  };

  return (
    <Sheet title={t('updater.offerTitle', { latest: update.latest })} onClose={close}>
      <div data-testid="update-offer" className="flex flex-col gap-3 pb-2">
        <p className="text-muted text-sm">{t('updater.offerText', { current: update.current })}</p>
        {update.notes && (
          <div className="bg-background max-h-56 overflow-y-auto rounded-2xl px-4 py-2 text-sm">
            <Markdown source={update.notes} onOpenLink={openLink} />
          </div>
        )}
        {update.installKind === 'portable' && (
          <p className="text-muted text-sm">{t('updater.portableHint')}</p>
        )}
        {state === 'downloading' && (
          <div
            role="progressbar"
            aria-label={t('updater.downloading')}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round((update.progress ?? 0) * 100)}
            className="bg-background h-1.5 overflow-hidden rounded-full"
          >
            <div
              className="bg-primary h-full origin-left transition-transform"
              style={{ transform: `scaleX(${update.progress ?? 0})` }}
            />
          </div>
        )}
        {state === 'installing' && <p className="text-sm">{t('updater.installing')}</p>}
        {state === 'needsPermission' && <p className="text-sm">{t('updater.needsPermission')}</p>}
        {failure && <Problem title={t('updater.installFailed')} detail={failure} />}
        <button
          type="button"
          disabled={busy || install.isPending}
          onClick={() => {
            setFollowing(true);
            install.mutate();
          }}
          className={`${BUTTON} bg-primary text-background`}
        >
          {selfUpdates ? (
            <Download aria-hidden size={18} />
          ) : (
            <ExternalLink aria-hidden size={18} />
          )}
          {!selfUpdates
            ? t('updater.offerOpen')
            : state === 'needsPermission'
              ? t('updater.tryAgain')
              : t('updater.offerInstall')}
        </button>
        {!stillFollowing && (
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => answer.mutate('later')}
              className={`${BUTTON} bg-background`}
            >
              {t('updater.later')}
            </button>
            <button
              type="button"
              onClick={() => answer.mutate('skip')}
              className={`${BUTTON} bg-background`}
            >
              {t('updater.skip')}
            </button>
          </div>
        )}
      </div>
    </Sheet>
  );
};
