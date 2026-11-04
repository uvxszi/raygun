import { commands } from '@raygun/bindings';
import type { HiddenArtist, HiddenItem, Settings } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Eye } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { artistCover } from '../library/coverTargets';
import { useHideActions } from '../music/hidden';
import { SETTINGS_KEY, useSettings } from '../settings/useSettings';
import { TopBar } from '../shell/TopBar';

const newestFirst = <Entry extends { hiddenAt: number }>(list: Entry[]) =>
  [...list].sort((a, b) => b.hiddenAt - a.hiddenAt);

const Group: FC<{
  title: string;
  count: number;
  onShowAll?: () => void;
  busy: boolean;
  empty: string;
  children: ReactNode;
}> = ({ title, count, onShowAll, busy, empty, children }) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col">
      <div className="flex items-center gap-3 px-1 pt-4 pb-1">
        <h2 className="text-primary text-sm font-semibold">
          {title} <span className="text-muted font-normal">{count}</span>
        </h2>
        {onShowAll && count > 1 && (
          <button
            type="button"
            disabled={busy}
            onClick={onShowAll}
            className="text-muted hover:text-foreground ml-auto text-sm font-semibold disabled:opacity-60"
          >
            {t('hidden.showAll')}
          </button>
        )}
      </div>
      {count === 0 ? (
        <p className="text-muted px-1 py-3 text-sm">{empty}</p>
      ) : (
        <ul className="flex flex-col">{children}</ul>
      )}
    </section>
  );
};

const Row: FC<{
  artwork: ReactNode;
  title: string;
  detail?: string | null;
  onShow: () => void;
}> = ({ artwork, title, detail, onShow }) => {
  const { t } = useTranslation();
  return (
    <li className="flex items-center gap-3 rounded-xl px-1 py-2">
      {artwork}
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate font-medium">{title}</span>
        {detail && <span className="text-muted truncate text-sm">{detail}</span>}
      </span>
      <button
        type="button"
        aria-label={t('hidden.showNamed', { name: title })}
        onClick={onShow}
        className="bg-surface hover:bg-surface/70 flex shrink-0 items-center gap-2 rounded-full px-3 py-1.5 text-sm"
      >
        <Eye aria-hidden size={16} />
        {t('hidden.show')}
      </button>
    </li>
  );
};

/**
 * Settings → Hidden: the artists not shown (Don't show) and what Home doesn't show again (Not
 * interested), newest first, each with Show again.
 */
export const HiddenPage: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const hidden = useSettings().data?.hidden;
  const actions = useHideActions();
  const clear = useMutation({
    mutationFn: ({ artists, items }: { artists: boolean; items: boolean }) =>
      unwrapResult(commands.hiddenClear(artists, items)),
    onSuccess: (settings: Settings) => {
      client.setQueryData(SETTINGS_KEY, settings);
      void client.invalidateQueries({ queryKey: ['music'] });
      void client.invalidateQueries({ queryKey: ['recentContexts'] });
    },
  });
  const artists: HiddenArtist[] = newestFirst(hidden?.artists ?? []);
  const items: HiddenItem[] = newestFirst(hidden?.items ?? []);
  return (
    <section data-testid="hidden-page" className="flex max-w-2xl flex-col">
      <TopBar back fallback="/settings/browse" title={t('hidden.title')} />
      <Group
        title={t('hidden.artists')}
        count={artists.length}
        busy={clear.isPending}
        onShowAll={() => clear.mutate({ artists: true, items: false })}
        empty={t('hidden.noArtists')}
      >
        {artists.map((artist) => (
          <Row
            key={artist.name}
            artwork={
              <Artwork
                url={null}
                fallback={artistCover(artist.name)}
                className="size-11 shrink-0 rounded-full"
                iconSize={18}
              />
            }
            title={artist.name}
            onShow={() => actions.showArtist(artist.name)}
          />
        ))}
      </Group>
      <Group
        title={t('hidden.items')}
        count={items.length}
        busy={clear.isPending}
        onShowAll={() => clear.mutate({ artists: false, items: true })}
        empty={t('hidden.noItems')}
      >
        {items.map((item) => (
          <Row
            key={item.key}
            artwork={
              <Artwork
                url={item.artworkUrl}
                className={`size-11 shrink-0 ${item.kind === 'artist' ? 'rounded-full' : 'rounded-md'}`}
                iconSize={18}
              />
            }
            title={item.name}
            detail={[t(`hidden.kinds.${item.kind}`, { defaultValue: '' }), item.subtitle]
              .filter(Boolean)
              .join(' · ')}
            onShow={() => actions.interested(item)}
          />
        ))}
      </Group>
    </section>
  );
};
