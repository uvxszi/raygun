import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { Download, EyeOff, Heart, ListPlus } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SavedTrackRow } from '../collections/SavedTrackRow';
import {
  COLLECTIONS,
  HOME_HISTORY_LIMIT,
  fromSavedTrack,
  playCollection,
  useClearHistory,
  useHistory,
} from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { useHidden } from '../music/hidden';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import { downloadSongs } from '../music/useSongDownloads';
import { PickTarget } from '../select/PickTarget';
import { usePickList } from '../select/usePickList';
import { useSettings } from '../settings/useSettings';
import { ContinuePickBar } from './ContinuePickBar';
import type { PickAction } from './ContinuePickBar';
import { HomeSection } from './HomeSection';

/**
 * What was played last, as rows (hidden artists' songs and Not interested ones stay out). Rows
 * are picked with a hold, a right or Ctrl-click or their circle; the bar takes them out of
 * Recently played, favorites them, puts them in a playlist or downloads them.
 */
export const RecentlyPlayedRow: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const clear = useClearHistory();
  const hidden = useHidden();
  // Listening history off (M46.41): nothing of it shows.
  const keep = useSettings().data?.privacy.history !== false;
  const entries = (history.data ?? [])
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => !hidden.hidesSong(entry.track));
  const picks = usePickList(entries.map(({ entry }) => entry.track.trackKey));
  if (!keep || entries.length === 0) return null;

  const picked = entries
    .map(({ entry }) => entry.track)
    .filter((track) => picks.has(track.trackKey));
  const count = picked.length;
  const tracks = () => picked.map(fromSavedTrack);
  const refresh = () => client.invalidateQueries({ queryKey: [COLLECTIONS] });
  const actions: PickAction[] = [
    {
      label: t('home.continue.removeRecent'),
      icon: EyeOff,
      run: async () => {
        try {
          await unwrapResult(commands.historyRemove(picked.map((track) => track.trackKey)));
        } finally {
          await refresh();
        }
        return t('home.continue.removed', { count });
      },
    },
    {
      label: t('search.select.favorite'),
      icon: Heart,
      run: async () => {
        try {
          await unwrapResult(commands.favoritesSetMany(tracks(), true));
        } finally {
          await refresh();
        }
        return t('search.select.favorited', { count });
      },
    },
    {
      label: t('search.select.addToPlaylist'),
      icon: ListPlus,
      run: () => {
        const chosen = tracks();
        useMenuSheetsStore.getState().addToPlaylist({
          title: t('search.select.playlistTitle', { count: chosen.length }),
          load: () => Promise.resolve(chosen),
        });
        return Promise.resolve(null);
      },
    },
    {
      label: t('search.select.download'),
      icon: Download,
      run: async () => {
        await downloadSongs(tracks());
        return t('search.select.queued', { count });
      },
    },
  ];

  return (
    <HomeSection
      id={id}
      title={t('home.recentlyPlayed')}
      testId="recently-played"
      extra={
        <button
          type="button"
          disabled={clear.isPending}
          onClick={() => clear.mutate()}
          className="text-muted hover:text-foreground px-1 text-sm font-semibold"
        >
          {t('home.clearHistory')}
        </button>
      }
    >
      {picks.selecting && (
        <ContinuePickBar
          label={t('home.continue.actions', { name: t('home.recentlyPlayed') })}
          count={count}
          allPicked={count === entries.length}
          onSelectAll={picks.selectAll}
          onClose={picks.clear}
          actions={actions}
        />
      )}
      <ul className="flex flex-col">
        {entries.map(({ entry, index }) => (
          <li key={entry.track.trackKey}>
            <PickTarget
              pick={picks.state(entry.track.trackKey)}
              label={t('bulk.pick', { name: entry.track.title })}
              place="row"
              testId="recent-pick"
            >
              <SavedTrackRow
                track={entry.track}
                onPlay={() =>
                  void playCollection({
                    collection: { kind: 'history' },
                    startIndex: index,
                    shuffle: false,
                  })
                }
              />
            </PickTarget>
          </li>
        ))}
      </ul>
    </HomeSection>
  );
};
