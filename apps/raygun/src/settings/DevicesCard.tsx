import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSyncStatus } from '../devices/useDeviceSync';

/** Device sync at a glance, linking to the devices page. */
export const DevicesCard: FC = () => {
  const { t } = useTranslation();
  const status = useSyncStatus();
  const data = status.data;

  return (
    <Link
      to="/settings/devices"
      data-testid="devices-card"
      className="bg-surface flex items-center gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="text-muted">{t('devices.title')}</span>
        <span>
          {!data
            ? ''
            : !data.enabled
              ? t('devices.off')
              : data.peers.length === 0
                ? t('devices.noPeersShort')
                : data.peers.map((peer) => peer.name).join(', ')}
        </span>
      </span>
      <span className="text-muted flex items-center gap-1">
        {t('devices.manage')}
        <ChevronRight aria-hidden size={18} />
      </span>
    </Link>
  );
};
