import type { MangaUpdateItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { BellDot } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../shell/EmptyState';
import { chapterUpdateLink } from './chapterUpdateLink';
import { DownloadButton } from './DownloadButton';
import { groupByDay, useDayLabel } from './groupByDay';
import { bookWords } from './bookWords';
import { MangaCover } from './MangaCover';
import { useMangaUpdates } from './useManga';
import { SourceIssue } from '../shell/SourceIssue';

/** A chapter an update found: its title and chapter, read greyed out, with its download. */
export const ChapterUpdateRow: FC<{ item: MangaUpdateItem }> = ({ item }) => {
  const link = chapterUpdateLink(item);
  return (
    <li className="flex items-center gap-3">
      <Link
        to={link.to}
        search={link.search}
        data-testid="update-row"
        className={`flex min-w-0 flex-1 items-center gap-3 py-2 ${item.chapter.read ? 'opacity-50' : ''}`}
      >
        <MangaCover
          url={item.coverUrl}
          book={bookWords(item.medium, item.title)}
          className="w-10 shrink-0 rounded"
        />
        <span className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium">{item.title}</span>
          <span className="text-muted truncate text-xs">{item.chapter.name}</span>
        </span>
      </Link>
      <DownloadButton chapter={item.chapter} />
    </li>
  );
};

/** Updates → Chapters: comic and book chapters that library updates found, by day. */
export const ChapterUpdates: FC = () => {
  const { t } = useTranslation();
  const updates = useMangaUpdates();
  const dayLabel = useDayLabel();

  return (
    <div data-testid="chapter-updates" className="flex flex-col gap-4">
      {updates.error && (
        <SourceIssue
          compact
          title={t('manga.updatesUnreadable')}
          onRetry={() => void updates.refetch()}
        >
          {updates.error.message}
        </SourceIssue>
      )}
      {updates.data?.length === 0 && <EmptyState icon={BellDot} title={t('manga.noUpdates')} />}
      {updates.data &&
        groupByDay(updates.data).map((group) => (
          <section key={group.day} className="flex flex-col gap-1">
            <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
            <ul className="flex flex-col">
              {group.items.map((item) => (
                <ChapterUpdateRow key={item.chapter.id} item={item} />
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
};
