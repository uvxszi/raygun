import { Link } from '@tanstack/react-router';
import { MonitorSmartphone } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlatform } from '../lib/usePlatform';
import { useRemoteConnected } from '../web/remoteState';

/**
 * Whether what only the Raygun app can do is out of reach here: Raygun on the web (docs/WEB.md)
 * without a linked app answering for it.
 */
export const useNeedsApp = () => {
  const web = usePlatform() === 'web';
  const remote = useRemoteConnected();
  return web && !remote;
};

/** Says that this needs the app, with the way to link one (Devices). */
export const NeedsAppNote: FC<{ compact?: boolean }> = ({ compact = false }) => {
  const { t } = useTranslation();
  return (
    <div
      role="note"
      className={`bg-surface text-muted flex items-start gap-3 rounded-xl ${compact ? 'px-3 py-2 text-sm' : 'px-4 py-3'}`}
    >
      <MonitorSmartphone aria-hidden size={compact ? 16 : 20} className="mt-0.5 shrink-0" />
      <span className="flex flex-col gap-1">
        <span>{t('web.needsApp')}</span>
        <Link to="/settings/devices" className="text-primary font-medium">
          {t('web.linkApp')}
        </Link>
      </span>
    </div>
  );
};

/**
 * Its children where they can work; on Raygun on the web without a linked app, the note saying
 * they need the app (or nothing, with `hide`).
 */
export const NeedsApp: FC<{ children: ReactNode; hide?: boolean; compact?: boolean }> = ({
  children,
  hide = false,
  compact,
}) => {
  const needs = useNeedsApp();
  if (!needs) return children;
  return hide ? null : <NeedsAppNote compact={compact} />;
};
