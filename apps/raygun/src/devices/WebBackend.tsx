import { MonitorSmartphone } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlatform } from '../lib/usePlatform';
import { PrefAction, PrefGroup, PrefSwitch } from '../settings/prefs';
import { remoteControls, useRemoteState } from '../web/remoteState';
import { useSyncStatus } from './useDeviceSync';

/**
 * Raygun on the web (docs/WEB.md): which linked app answers for the page's sources, searches and
 * downloads, with a switch to use it and a way to pick another.
 */
export const WebBackendCard: FC = () => {
  const { t } = useTranslation();
  const web = usePlatform() === 'web';
  const state = useRemoteState();
  const peers = useSyncStatus().data?.peers ?? [];
  if (!web || !state.available) return null;
  const controls = remoteControls();
  const others = peers.filter((peer) => peer.deviceId !== state.deviceId && peer.address);
  const summary = state.connected
    ? t('devices.webBackend.inUse')
    : state.connecting
      ? t('devices.webBackend.connecting')
      : state.error
        ? t('devices.webBackend.unreachable', { reason: state.error })
        : state.enabled
          ? t('devices.webBackend.waiting')
          : undefined;
  return (
    <PrefGroup title={t('devices.webBackend.title')} summary={t('devices.webBackend.summary')}>
      <PrefSwitch
        title={t('devices.webBackend.use', { name: state.device ?? '' })}
        summary={summary}
        checked={state.enabled}
        onChange={(on) => controls?.setEnabled(on)}
      />
      {state.enabled &&
        others.map((peer) => (
          <PrefAction
            key={peer.deviceId}
            icon={MonitorSmartphone}
            title={t('devices.webBackend.useInstead', { name: peer.name })}
            onClick={() => controls?.choose(peer.deviceId)}
          />
        ))}
    </PrefGroup>
  );
};

/** "via Laptop": shown where sources answer while a linked app answers for the page. */
export const WebBackendVia: FC = () => {
  const { t } = useTranslation();
  const state = useRemoteState();
  if (!state.connected || !state.device) return null;
  return (
    <p
      data-testid="web-backend-via"
      className="text-muted flex items-center gap-1.5 self-start px-1 text-xs"
    >
      <MonitorSmartphone size={14} aria-hidden />
      {t('devices.webBackend.via', { name: state.device })}
    </p>
  );
};
