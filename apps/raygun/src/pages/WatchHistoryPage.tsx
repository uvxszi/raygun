import type { ContinueWatching } from '@raygun/bindings';
import { Check, Clapperboard, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { groupByDay, useDayLabel } from '../manga/groupByDay';
import type { MenuItem } from '../shell/contextMenuStore';
import { EmptyState, Loading } from '../shell/EmptyState';
import { MenuButton } from '../shell/MenuButton';
import { Picture } from '../shell/Picture';
import { SourceIssue } from '../shell/SourceIssue';
import { useContinueMenu } from '../watch/continueMenu';
import { ContinueSelectionBar, PickCircle } from '../watch/ContinueSelection';
import { ContinueSourcePicker } from '../watch/ContinueSourcePicker';
import { useContinueSelection, usePickGestures } from '../watch/useContinueSelection';
import type { ContinueSelection } from '../watch/useContinueSelection';
import { useResumeVideo, useWatchHistory } from '../watch/useWatch';
import { PageHeader } from './PageHeader';

/** The poster, fading in, or a plain frame when there's none or it can't load (offline). */
const Poster: FC<{ url: string | null }> = ({ url }) => (
  <Picture
    src={url}
    placeholder={
      <span className="bg-background text-muted grid size-full place-items-center">
        <Clapperboard size={18} />
      </span>
    }
    className="aspect-video w-20 shrink-0 rounded"
  />
);

/** A row: a tap resumes it, ⋮ has the rest; a long press, its circle or Ctrl/Shift-click picks it. */
const HistoryRow: FC<{
  item: ContinueWatching;
  selection: ContinueSelection;
  menu: (item: ContinueWatching) => MenuItem[];
  onResume: (key: string) => void;
}> = ({ item, selection, menu, onResume }) => {
  const { t } = useTranslation();
  const selected = selection.isSelected(item.key);
  const pick = (range: boolean) => selection.pick(item.key, range);
  const gestures = usePickGestures(selection.selecting, pick);
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  const name = [item.title, item.subtitle].filter(Boolean).join(' · ');
  return (
    <li
      aria-label={name}
      data-selected={selected || undefined}
      {...gestures}
      className={`group/pick relative flex items-center gap-3 px-4 py-3 [-webkit-touch-callout:none] ${
        selected ? 'bg-primary/15' : ''
      }`}
    >
      <button
        type="button"
        onClick={() => onResume(item.key)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Poster url={item.poster} />
        <span className="flex min-w-0 flex-col gap-1">
          <span className="truncate font-medium">{item.title}</span>
          {item.subtitle && <span className="text-muted truncate text-xs">{item.subtitle}</span>}
          {item.watched ? (
            <span className="text-muted flex items-center gap-1 text-xs">
              <Check aria-hidden size={12} />
              {t('library.videos.watched')}
            </span>
          ) : (
            duration > 0 && (
              <span
                aria-hidden
                className="bg-background block h-1 w-32 overflow-hidden rounded-full"
              >
                <span
                  className="bg-primary block h-full origin-left"
                  style={{ transform: `scaleX(${Math.min(1, position / duration)})` }}
                />
              </span>
            )
          )}
        </span>
      </button>
      <PickCircle
        checked={selected}
        shown={selection.selecting}
        name={name}
        onPick={pick}
        className="absolute top-1/2 left-5 -translate-y-1/2"
      />
      {!selection.selecting && (
        <MenuButton label={t('menu.options', { name })} items={() => menu(item)} />
      )}
    </li>
  );
};

/** Library → Videos → History: everything watched or started, newest first, by day (Aniyomi's). */
export const WatchHistoryPage: FC = () => {
  const { t } = useTranslation();
  const history = useWatchHistory();
  const resume = useResumeVideo();
  const dayLabel = useDayLabel();
  const items = history.data ?? [];
  const selection = useContinueSelection(items.map((item) => item.key));
  const [choosing, setChoosing] = useState<ContinueWatching | null>(null);
  const menu = useContinueMenu('history', setChoosing);

  return (
    <section data-testid="watch-history-page" className="flex min-h-full flex-col gap-4">
      <PageHeader back fallback="/library" title={t('library.videos.history')} />
      {history.error && (
        <SourceIssue compact title={t('library.videos.historyUnreadable')}>
          {history.error.message}
        </SourceIssue>
      )}
      {resume.error && (
        <SourceIssue compact title={t('watch.playFailed')}>
          {resume.error.message}
        </SourceIssue>
      )}
      {history.isPending && <Loading label={t('manga.loading')} />}
      {history.data?.length === 0 && (
        <EmptyState
          icon={Clapperboard}
          title={t('library.videos.historyEmptyTitle')}
          action={{
            label: t('library.findInSearch'),
            icon: Search,
            to: '/browse',
            search: { tab: 'search', type: 'video' },
          }}
        />
      )}
      {groupByDay(items, (item) => item.updatedAt ?? 0).map((group) => (
        <section key={group.day} className="flex flex-col gap-1">
          <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
          <ul className="bg-surface flex flex-col overflow-hidden rounded-xl">
            {group.items.map((item) => (
              <HistoryRow
                key={item.key}
                item={item}
                selection={selection}
                menu={menu}
                onResume={(key) => resume.mutate(key)}
              />
            ))}
          </ul>
        </section>
      ))}
      {selection.selecting && (
        <ContinueSelectionBar
          selection={selection}
          history
          className="sticky bottom-2 z-30 mt-auto"
        />
      )}
      {choosing && <ContinueSourcePicker item={choosing} onClose={() => setChoosing(null)} />}
    </section>
  );
};
