import type { ServerInfo } from '@raygun/bindings';
import { KeyRound, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { SourceIssue } from '../shell/SourceIssue';
import { matchesSearch } from '../shell/useListSearch';
import { ServerSheet } from './ServerSheet';
import { BRINGS, hostOf, useRemoveServer, useServers } from './useServers';

const ServerRow: FC<{ server: ServerInfo; onSignIn: () => void }> = ({ server, onSignIn }) => {
  const { t } = useTranslation();
  const remove = useRemoveServer();
  const brings = [
    ...new Set(
      server.provides.map((kind) => BRINGS[kind]).filter((key): key is string => key !== undefined),
    ),
  ];
  return (
    <li data-testid="server-row" className="flex flex-col gap-1.5 py-1.5">
      <div className="flex items-center gap-1">
        <div className="flex min-w-0 flex-1 items-center gap-4 px-1">
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

/**
 * Browse → Sources → Servers: servers people own (Jellyfin, Emby, Subsonic, Audiobookshelf,
 * OPDS catalogues), added by address and sign-in. Their films, music, books and comics then show
 * with every other source's.
 */
export const ServersSection: FC<{ query?: string }> = ({ query = '' }) => {
  const { t } = useTranslation();
  const servers = useServers();
  const [editing, setEditing] = useState<ServerInfo | 'new' | null>(null);
  const shown = servers.data?.filter((server) =>
    matchesSearch(query, [server.name, server.address]),
  );
  const title = t('servers.title');

  return (
    <section aria-label={title} className="flex flex-col">
      <div className="flex items-center justify-between gap-2 px-1 pt-2 pb-1">
        <h2 className="text-muted text-sm font-medium">{title}</h2>
        <button
          type="button"
          onClick={() => setEditing('new')}
          className="text-primary flex items-center gap-1 rounded-full px-3 py-1.5 text-sm font-medium"
        >
          <Plus aria-hidden size={16} />
          {t('servers.add')}
        </button>
      </div>
      {servers.error && (
        <SourceIssue compact title={t('servers.loadFailed')}>
          {servers.error.message}
        </SourceIssue>
      )}
      {servers.data?.length === 0 && (
        <p className="text-muted px-1 pb-2 text-sm">{t('servers.empty')}</p>
      )}
      {shown && (
        <ul className="flex flex-col">
          {shown.map((server) => (
            <ServerRow key={server.id} server={server} onSignIn={() => setEditing(server)} />
          ))}
        </ul>
      )}
      {editing && (
        <ServerSheet
          server={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </section>
  );
};
