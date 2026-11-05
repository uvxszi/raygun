import { MonitorSpeaker } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useConnectSession } from './useConnect';

/** "Playing on Living room PC" (M46.15): Spotify's strip while this device plays elsewhere. */
export const PlayingOn: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const session = useConnectSession();
  if (!session) return null;
  return (
    <p
      data-testid="playing-on"
      className={`bg-primary text-background flex items-center gap-2 px-4 py-1.5 text-sm font-semibold ${className}`}
    >
      <MonitorSpeaker aria-hidden size={16} className="shrink-0" />
      <span className="truncate">{t('connect.playingOn', { name: session.deviceName })}</span>
    </p>
  );
};
