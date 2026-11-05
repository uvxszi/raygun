import type { ServerInfo } from '@raygun/bindings';
import { KeyRound, Trash2 } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { SourceIssue } from '../shell/SourceIssue';
import { BRINGS, hostOf, useRemoveServer } from './useServers';

/**
 * A connected server (Sources → Servers): its kind, address and user, what it brings, and why it
 * isn't working when it isn't. `leading` and `trailing` hold the row's selection box and switch.
 */
export const ServerRow: FC<{
  server: ServerInfo;
  onSignIn: () => void;
  /** Turned off: listed dimmed, asked for nothing. */
  off?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
}> = ({ server, onSignIn, off = false, leading, trailing }) => {
  const { t } = useTranslation();
  const remove = useRemoveServer();
  const brings = [
    ...new Set(
      server.provides.map((kind) => BRINGS[kind]).filter((key): key is string => key !== undefined),
    ),
  ];
  return (
    <li data-testid="server-row" aria-label={server.name} className="flex flex-col gap-1.5 py-1.5">
      <div className="flex items-center gap-1">
        {leading}
        <div className={`flex min-w-0 flex-1 items-center gap-4 px-1 ${off ? 'opacity-50' : ''}`}>
          <LetterAvatar name={server.name} />
          <span className="flex min-w-0 flex-col">
            <span className="truncate font-medium">{server.name}</span>
            <span className="text-muted truncate text-xs">
              {[t(`servers.kinds.${server.kind}`), hostOf(server.address), server.username]
                .filter(Boolean)
                .join(' · ')}
            </span>
          </span>
        </div>
        {trailing}
        <OverflowMenu
          label={t('servers.menu', { name: server.name })}
          items={[
            { label: t('servers.signInAgain'), icon: KeyRound, onSelect: onSignIn },
            {
              label: t('servers.remove'),
              icon: Trash2,
              confirm: t('servers.removeConfirm', { name: server.name }),
              onSelect: () => remove.mutate(server.id),
            },
          ]}
        />
      </div>
      {brings.length > 0 && (
        <ul aria-label={t('servers.bringsLabel')} className="flex flex-wrap gap-1.5 pl-14">
          {brings.map((key) => (
            <li key={key} className="bg-surface text-muted rounded-full px-2.5 py-0.5 text-xs">
              {t(key)}
            </li>
          ))}
        </ul>
      )}
      {(server.error ?? remove.error) && (
        <SourceIssue compact source={server.name} className="ml-14">
          {remove.error?.message ?? server.error}
        </SourceIssue>
      )}
    </li>
  );
};
