import type { ServerInfo, ServerSyncPrefs } from '@raygun/bindings';
import { ListMusic, LoaderCircle, RefreshCw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefSwitch } from '../settings/prefs';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import {
  useServerSync,
  useSetServerSync,
  useSyncServerNow,
  useTakeServerQueue,
} from './useServers';

/** The switches, in the order the sheet lists them. */
const SWITCHES: (keyof ServerSyncPrefs)[] = ['favorites', 'playlists', 'scrobble', 'playQueue'];

/**
 * What a Subsonic or Navidrome server keeps in step with Raygun, as Feishin and Navic keep theirs:
 * stars and Favorites, playlists, plays counted on the server, and the play queue (with a button
 * to go on from the queue another app left there).
 */
export const ServerSyncSheet: FC<{ server: ServerInfo; onClose: () => void }> = ({
  server,
  onClose,
}) => {
  const { t } = useTranslation();
  const status = useServerSync(server.id);
  const set = useSetServerSync(server.id);
  const now = useSyncServerNow(server.id);
  const take = useTakeServerQueue(server.id);
  const prefs = status.data?.prefs;
  const syncing = set.isPending || now.isPending || status.data?.running === true;
  const lists = prefs ? prefs.favorites || prefs.playlists : false;
  const report = status.data?.report;
  const error = set.error?.message ?? now.error?.message ?? status.data?.error ?? null;

  return (
    <Sheet title={t('servers.sync.title', { name: server.name })} onClose={onClose}>
      {status.isError && <SourceIssue compact>{t('servers.sync.loadFailed')}</SourceIssue>}
      {prefs && (
        <div className="flex flex-col gap-3 text-sm">
          <ul className="flex flex-col">
            {SWITCHES.map((name) => (
              <PrefSwitch
                key={name}
                title={t(`servers.sync.${name}`)}
                summary={t(`servers.sync.${name}Note`)}
                checked={prefs[name]}
                disabled={set.isPending}
                onChange={(on) => set.mutate({ ...prefs, [name]: on })}
              />
            ))}
          </ul>
          {lists && (
            <div className="flex items-center gap-3 px-1">
              <span className="text-muted flex min-w-0 flex-1 flex-col text-xs">
                <span>
                  {status.data?.syncedAt
                    ? t('servers.sync.syncedAt', {
                        when: new Date(status.data.syncedAt).toLocaleString(),
                      })
                    : t('servers.sync.never')}
                </span>
                {report && (
                  <span>
                    {t('servers.sync.report', {
                      starred: report.starred,
                      favorited: report.favorited,
                      playlists: report.playlistsMade + report.playlistsChanged,
                    })}
                  </span>
                )}
                {report && report.notOnServer > 0 && (
                  <span>{t('servers.sync.notOnServer', { count: report.notOnServer })}</span>
                )}
              </span>
              <button
                type="button"
                disabled={syncing}
                onClick={() => now.mutate(undefined)}
                className="bg-surface flex shrink-0 items-center gap-1.5 rounded-full px-4 py-2 font-medium disabled:opacity-60"
              >
                {syncing ? (
                  <LoaderCircle aria-hidden size={16} className="animate-spin" />
                ) : (
                  <RefreshCw aria-hidden size={16} />
                )}
                {syncing ? t('servers.sync.syncing') : t('servers.sync.now')}
              </button>
            </div>
          )}
          {error && (
            <SourceIssue compact source={server.name}>
              {error}
            </SourceIssue>
          )}
          <button
            type="button"
            disabled={take.isPending}
            onClick={() => take.mutate()}
            className="bg-surface flex items-center gap-1.5 self-start rounded-full px-4 py-2 font-medium disabled:opacity-60"
          >
            {take.isPending ? (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            ) : (
              <ListMusic aria-hidden size={16} />
            )}
            {t('servers.sync.takeQueue')}
          </button>
          {take.data !== undefined && (
            <p role="status" className="text-muted px-1 text-xs">
              {t('servers.sync.queueTaken', { count: take.data })}
            </p>
          )}
          {take.error && (
            <SourceIssue compact source={server.name}>
              {take.error.message}
            </SourceIssue>
          )}
        </div>
      )}
    </Sheet>
  );
};
