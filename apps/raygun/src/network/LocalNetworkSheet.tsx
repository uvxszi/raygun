import { commands } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { LoaderCircle, Wifi } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useBackLayer } from '../shell/backLayers';

/**
 * What reaching the local network is for: an addon there, devices there reaching the remote,
 * playing on another device there (Connect, M46.15), or a server there.
 */
export type LocalNetworkReason = 'addon' | 'remote' | 'connect' | 'cast' | 'server';

type LocalNetworkSheetProps = {
  reason: LocalNetworkReason;
  onGranted: () => void;
  onClose: () => void;
};

/**
 * Says why Raygun needs Android 17's local network access before Android's own dialog, which
 * only talks about "nearby devices". A denial says what doesn't work without it. Android is always
 * asked first: only its answer tells whether it stopped asking, and then the way is the app's
 * settings.
 */
export const LocalNetworkSheet: FC<LocalNetworkSheetProps> = ({ reason, onGranted, onClose }) => {
  const { t } = useTranslation();
  useBackLayer(true, onClose);
  const request = useMutation({
    mutationFn: () => unwrapResult(commands.localNetworkRequest()),
    onSuccess: (access) => {
      if (access.granted) onGranted();
    },
  });
  const openSettings = useMutation({
    mutationFn: () => unwrapResult(commands.localNetworkOpenSettings()),
    onSuccess: onClose,
  });
  const denied = request.data && !request.data.granted;
  const mustUseSettings = request.data?.blocked === true;
  const busy = request.isPending || openSettings.isPending;
  const error = request.error ?? openSettings.error;

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="local-network-title"
        className="bg-surface flex w-full max-w-md flex-col gap-4 rounded-t-2xl px-5 pt-5 pb-[calc(1.25rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <div className="flex items-center gap-3">
          <span
            aria-hidden
            className="bg-background text-primary grid size-10 shrink-0 place-items-center rounded-full"
          >
            <Wifi size={20} />
          </span>
          <h2 id="local-network-title" className="text-lg font-semibold">
            {t('localNetwork.title')}
          </h2>
        </div>
        <p className="text-muted text-sm">{t(`localNetwork.why.${reason}`)}</p>
        {denied && (
          <p role="alert" className="text-primary text-sm">
            {t(`localNetwork.denied.${reason}`)}
          </p>
        )}
        {mustUseSettings && <p className="text-muted text-sm">{t('localNetwork.settingsHint')}</p>}
        {error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {error.message}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="bg-background rounded-full px-5 py-2 text-sm disabled:opacity-50"
          >
            {t('localNetwork.notNow')}
          </button>
          <button
            type="button"
            onClick={() => (mustUseSettings ? openSettings.mutate() : request.mutate())}
            disabled={busy}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-70"
          >
            {busy && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
            {t(mustUseSettings ? 'localNetwork.openSettings' : 'localNetwork.allow')}
          </button>
        </div>
      </div>
    </div>
  );
};
