import { commands } from '@raygun/bindings';
import type { ContinueReadingItem } from '@raygun/bindings';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { BookmarkMinus, CheckCheck, Download, EyeOff, Pin } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { bookWords } from '../manga/bookWords';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useTitleMenuItems } from '../music/cardMenus';
import { PickTarget } from '../select/PickTarget';
import { usePickList } from '../select/usePickList';
import { refreshLibraries } from '../select/bulkActions';
import { CardMenu } from '../shell/CardMenu';
import { useRail } from '../shell/useRail';
import { ContinuePickBar } from './ContinuePickBar';
import type { PickAction } from './ContinuePickBar';
import { HomeCarousel, HomeSection } from './HomeSection';

/** Most titles in Continue reading. */
const CONTINUE_READING = 12;

const CONTINUE_READING_KEY = ['continueReading'];

/**
 * Comics and books read lately, newest first (Rust leaves out the ones taken out of the row until
 * they're read again). Asked again whenever either library changes.
 */
const useContinueReading = () => {
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  return useQuery({
    queryKey: [...CONTINUE_READING_KEY, manga.dataUpdatedAt, novels.dataUpdatedAt],
    queryFn: () => unwrapResult(commands.continueReading(CONTINUE_READING)),
    placeholderData: keepPreviousData,
  });
};

/** The picked titles' ids, by medium. */
const byMedium = (items: ContinueReadingItem[]) => {
  const groups = new Map<Medium, string[]>();
  for (const item of items) {
    const medium = item.medium as Medium;
    groups.set(medium, [...(groups.get(medium) ?? []), item.entry.titleId]);
  }
  return [...groups];
};

/**
 * Home's Continue reading (Mihon's History, as a row). Cards are picked with a hold, a right or
 * Ctrl-click or their circle; the bar then takes them out of the row (their places kept), marks
 * them read, downloads their next chapter, pins them to quick access or takes them out of the
 * library.
 */
export const ContinueReadingRow: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const menu = useTitleMenuItems();
  const rail = useRail();
  const reading = useContinueReading();
  const items = reading.data ?? [];
  const picks = usePickList(items.map((item) => item.entry.titleId));
  if (items.length === 0) return null;

  const picked = items.filter((item) => picks.has(item.entry.titleId));
  const media = byMedium(picked);
  const refresh = () =>
    Promise.all([
      refreshLibraries(
        client,
        media.map(([medium]) => medium),
      ),
      client.invalidateQueries({ queryKey: CONTINUE_READING_KEY }),
    ]);
  const each = async (work: (medium: Medium, ids: string[]) => Promise<unknown>) => {
    try {
      for (const [medium, ids] of media) await work(medium, ids);
    } finally {
      await refresh();
    }
  };
  const count = picked.length;
  const actions: PickAction[] = [
    {
      label: t('home.continue.remove'),
      icon: EyeOff,
      run: async () => {
        await unwrapResult(
          commands.continueHide(picked.map((item) => `reading:${item.entry.titleId}`)),
        );
        await refresh();
        return t('home.continue.removed', { count });
      },
    },
    {
      label: t('home.continue.markRead'),
      icon: CheckCheck,
      run: async () => {
        await each((medium, ids) => unwrapResult(commands.libraryMarkRead(medium, ids, true)));
        return t('home.continue.markedRead', { count });
      },
    },
    {
      label: t('home.continue.downloadNext'),
      icon: Download,
      run: async () => {
        let queued = 0;
        await each(async (medium, ids) => {
          queued += await unwrapResult(commands.libraryDownloadNext(medium, ids, 1));
        });
        return queued > 0
          ? t('search.select.queued', { count: queued })
          : t('search.select.nothingQueued');
      },
    },
    {
      label: t('rail.pin'),
      icon: Pin,
      run: () => {
        rail.setPinned(
          picked.map((item) => ({
            kind: item.medium,
            to: ROUTES[item.medium as Medium].title,
            search: { source: item.entry.sourceId, id: item.entry.remoteId },
            name: item.entry.title,
            artworkUrl: item.entry.coverUrl,
          })),
          true,
        );
        return Promise.resolve(t('home.continue.pinned', { count }));
      },
    },
    {
      label: t('menu.removeFromLibrary'),
      icon: BookmarkMinus,
      confirm: t('home.continue.removeFromLibraryAsk', { count }),
      run: async () => {
        await each((medium, ids) => unwrapResult(commands.libraryRemove(medium, ids, true, false)));
        return t('home.continue.removedFromLibrary', { count });
      },
    },
  ];

  return (
    <HomeSection id={id} title={t('home.continueReading')} testId="continue-reading">
      {picks.selecting && (
        <ContinuePickBar
          label={t('home.continue.actions', { name: t('home.continueReading') })}
          count={count}
          allPicked={count === items.length}
          onSelectAll={picks.selectAll}
          onClose={picks.clear}
          actions={actions}
        />
      )}
      <HomeCarousel>
        {items.map(({ medium: kind, entry }) => {
          const medium = kind as Medium;
          return (
            <li key={entry.titleId} className="w-28 shrink-0">
              <PickTarget
                pick={picks.state(entry.titleId)}
                label={t('bulk.pick', { name: entry.title })}
                testId="continue-reading-card"
              >
                <Link
                  to={ROUTES[medium].title}
                  search={{ source: entry.sourceId, id: entry.remoteId }}
                  aria-label={entry.title}
                  className="relative flex flex-col gap-1.5"
                >
                  {!picks.selecting && (
                    <CardMenu
                      label={t('menu.options', { name: entry.title })}
                      items={() =>
                        menu({
                          medium,
                          sourceId: entry.sourceId,
                          id: entry.remoteId,
                          title: entry.title,
                          coverUrl: entry.coverUrl,
                        })
                      }
                    />
                  )}
                  <span className="relative">
                    <MangaCover
                      url={entry.coverUrl}
                      book={bookWords(medium, entry.title, entry.authors)}
                      className="w-full rounded-xl"
                    />
                    {entry.unread > 0 && (
                      <span className="bg-primary text-background absolute right-1.5 bottom-1.5 rounded-md px-1.5 text-xs font-bold">
                        {entry.unread}
                      </span>
                    )}
                  </span>
                  <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
                </Link>
              </PickTarget>
            </li>
          );
        })}
      </HomeCarousel>
    </HomeSection>
  );
};
