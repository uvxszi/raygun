import type { SongDownload } from '@raygun/bindings';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Link } from '@tanstack/react-router';
import {
  ArrowUpToLine,
  CircleAlert,
  Disc3,
  FileDown,
  GripVertical,
  ListChecks,
  ListMusic,
  Pause,
  Play,
  RefreshCw,
  RotateCw,
  Settings2,
  Trash2,
  X,
} from 'lucide-react';
import { memo, useCallback, useMemo, useState } from 'react';
import type { CSSProperties, FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackMoreButton } from '../collections/TrackMoreButton';
import { StopDownloadButton } from '../downloads/StopDownload';
import { Artwork } from '../library/Artwork';
import { useSettings } from '../settings/useSettings';
import { ConfirmButton } from '../shell/ConfirmButton';
import { useDeviceState } from '../shell/deviceState';
import { EmptyState } from '../shell/EmptyState';
import { useIsDesktop } from '../shell/useIsDesktop';
import { formatBytes } from '../watch/episodeLabel';
import { tidyCredit } from './artists';
import { SaveSongsSheet } from './SaveSongsSheet';
import {
  GLIDE,
  isBusy,
  isFailed,
  sectionsOf,
  shareOf,
  useProblemLine,
  useStateLine,
} from './songDownloadState';
import { chooseDownloadSource } from './sourcePickerStore';
import {
  useSongDownloadActions,
  useSongDownloads,
  useSongDownloadsPaused,
} from './useSongDownloads';

const ICON =
  'text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-50';
const CHIP =
  'bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60';
/** Rows a group shows before the rest are asked for. */
const SHOWN = 50;

/** What went wrong, then the sources' own errors under More info. */
const Issue: FC<{ summary: string; detail: string | null }> = ({ summary, detail }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <span className="text-muted flex flex-col items-start gap-1 text-xs">
      <span className="flex items-start gap-1 wrap-anywhere">
        <CircleAlert aria-hidden size={12} className="mt-0.5 shrink-0" />
        {summary}
      </span>
      {detail && (
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="hover:text-foreground font-semibold"
        >
          {open ? t('issue.less') : t('issue.more')}
        </button>
      )}
      {detail && open && (
        <span className="selectable bg-background block w-full rounded-lg px-2 py-1.5 font-mono whitespace-pre-wrap wrap-anywhere">
          {detail}
        </span>
      )}
    </span>
  );
};

/** How far a download is: a bar scaled, not resized, so it moves without a layout. */
export const ProgressBar: FC<{ share: number; label: string }> = ({ share, label }) => (
  <span
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(share * 100)}
    className="bg-foreground/10 mt-1 block h-1 overflow-hidden rounded-full"
  >
    <span
      className={`bg-primary block h-full w-full origin-left ${GLIDE}`}
      style={{ transform: `scaleX(${share})` }}
    />
  </span>
);

/** Battery saver holds the queue, unless downloads may run in it (Settings → Downloads). */
const useDownloadsHeld = () => {
  const { device } = useDeviceState();
  const allowed = useSettings().data?.downloadsInBatterySaver ?? false;
  return device.powerSave && !allowed;
};

type Drag = {
  setNodeRef: (node: HTMLElement | null) => void;
  style: CSSProperties;
  handle: Record<string, unknown>;
  dragging: boolean;
};

type RowProps = {
  item: SongDownload;
  paused: boolean;
  onSave?: (id: string) => void;
  /** Puts it first in the queue. */
  onFirst?: (id: string) => void;
  drag?: Drag;
};

/** One download. Rows change only when their own download does. */
const Row: FC<RowProps> = memo(({ item, paused, onSave, onFirst, drag }: RowProps) => {
  const { t } = useTranslation();
  const line = useStateLine();
  const problemLine = useProblemLine();
  const { remove, retry, redownload } = useSongDownloadActions();
  const held = useDownloadsHeld();
  const share = shareOf(item);
  const failed = isFailed(item);
  const busy = isBusy(item);
  const refusal = retry.error ?? remove.error ?? redownload.error;
  // Stopping a song downloading again keeps the copy it had.
  const cancelLabel = item.replacing
    ? 'songs.cancelAgain'
    : busy
      ? 'songs.cancelOne'
      : 'songs.remove';
  return (
    <li
      ref={drag?.setNodeRef}
      style={drag?.style}
      data-testid="song-download"
      aria-label={item.title}
      className={`flex items-center gap-3 rounded-xl px-2 py-2 ${drag?.dragging ? 'bg-surface relative z-10 shadow-lg' : ''}`}
    >
      {drag && (
        <button
          type="button"
          aria-label={t('songs.drag', { title: item.title })}
          {...drag.handle}
          style={{ touchAction: 'none' }}
          className="text-muted -mr-2 grid size-8 shrink-0 cursor-grab place-items-center rounded-full active:cursor-grabbing"
        >
          <GripVertical aria-hidden size={16} />
        </button>
      )}
      <Artwork url={item.artworkUrl} className="size-12 shrink-0 rounded-md" iconSize={20} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-medium">{item.title}</span>
        {item.artist && (
          <span className="text-muted truncate text-sm">{tidyCredit(item.artist)}</span>
        )}
        <span className="text-muted truncate text-xs">{line(item, paused, held)}</span>
        {(failed || item.state === 'done') && item.error && (
          <Issue
            summary={
              item.state === 'done' ? t('songs.againFailed', { why: item.error }) : item.error
            }
            detail={item.errorDetail}
          />
        )}
        {item.state === 'done' && item.problem && (
          <Issue summary={problemLine(item.problem)} detail={item.problem.detail} />
        )}
        {share !== null && <ProgressBar share={share} label={item.title} />}
        {refusal && (
          <span role="alert" className="text-primary text-xs wrap-anywhere">
            {refusal.message}
          </span>
        )}
      </div>
      {item.state === 'queued' && onFirst && (
        <button
          type="button"
          aria-label={t('songs.downloadNext', { title: item.title })}
          data-hint={t('songs.downloadNext', { title: item.title })}
          onClick={() => onFirst(item.id)}
          className={ICON}
        >
          <ArrowUpToLine aria-hidden size={18} />
        </button>
      )}
      {item.state === 'done' && item.problem && (
        <button
          type="button"
          disabled={redownload.isPending}
          onClick={() => redownload.mutate([item.id])}
          className="bg-primary text-background flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-60"
        >
          <RefreshCw aria-hidden size={14} />
          {t('songs.downloadAgain')}
        </button>
      )}
      {failed && (
        <button
          type="button"
          aria-label={t('songs.chooseSource', { title: item.title })}
          data-hint={t('songs.chooseSource', { title: item.title })}
          onClick={() => chooseDownloadSource(item)}
          className={ICON}
        >
          <ListMusic aria-hidden size={18} />
        </button>
      )}
      {failed && (
        <button
          type="button"
          aria-label={t('songs.retry', { title: item.title })}
          data-hint={t('songs.retry', { title: item.title })}
          disabled={retry.isPending}
          onClick={() => retry.mutate(item.id)}
          className={ICON}
        >
          <RotateCw aria-hidden size={18} />
        </button>
      )}
      {item.state === 'done' && onSave && (
        <button
          type="button"
          aria-label={t('songs.saveOne', { title: item.title })}
          data-hint={t('songs.saveOne', { title: item.title })}
          onClick={() => onSave(item.id)}
          className={ICON}
        >
          <FileDown aria-hidden size={18} />
        </button>
      )}
      {busy ? (
        <StopDownloadButton
          label={t(cancelLabel, { title: item.title })}
          name={item.title}
          disabled={remove.isPending}
          onStop={() => remove.mutate(item.id)}
          className={ICON}
        >
          <X aria-hidden size={18} />
        </StopDownloadButton>
      ) : (
        <ConfirmButton
          label={t(cancelLabel, { title: item.title })}
          confirm={t('songs.removeConfirm')}
          icon={Trash2}
          disabled={remove.isPending}
          onConfirm={() => remove.mutate(item.id)}
          idleClassName={ICON}
        />
      )}
      {/* A saved song's own menu, as anywhere it's listed: play, its album and artist, copy… */}
      {item.state === 'done' && (
        <TrackMoreButton
          track={{
            title: item.title,
            artist: item.artist,
            album: item.album,
            durationMs: item.durationMs,
            artworkUrl: item.artworkUrl,
            source: null,
          }}
        />
      )}
    </li>
  );
});
Row.displayName = 'SongDownloadRow';

/** A queued download that can be dragged to another place in the queue. */
const QueuedRow: FC<Omit<RowProps, 'drag'>> = memo((props: Omit<RowProps, 'drag'>) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: props.item.id,
  });
  const drag = useMemo<Drag>(
    () => ({
      setNodeRef,
      style: { transform: CSS.Transform.toString(transform), transition },
      handle: { ...attributes, ...listeners },
      dragging: isDragging,
    }),
    [attributes, listeners, setNodeRef, transform, transition, isDragging],
  );
  return <Row {...props} drag={drag} />;
});
QueuedRow.displayName = 'QueuedSongDownloadRow';

const Group: FC<{ label: string; count: number; aside?: ReactNode; children: ReactNode }> = ({
  label,
  count,
  aside,
  children,
}) => (
  <section aria-label={label} className="flex flex-col gap-1">
    <div className="flex flex-wrap items-center gap-2 px-2">
      <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
        {label} · {count}
      </h3>
      {aside}
    </div>
    {children}
  </section>
);

/** The rest of a long group, on request. */
const ShowAll: FC<{ hidden: number; onShow: () => void; label: string }> = ({
  hidden,
  onShow,
  label,
}) =>
  hidden > 0 ? (
    <button
      type="button"
      onClick={onShow}
      className="text-primary self-start rounded-full px-3 py-2 text-sm font-semibold"
    >
      {label}
    </button>
  ) : null;

/** Downloading now, then the queue in the order it runs; the queue can be reordered. */
const Queue: FC<{ running: SongDownload[]; queued: SongDownload[]; paused: boolean }> = ({
  running,
  queued,
  paused,
}) => {
  const { t } = useTranslation();
  const { reorder } = useSongDownloadActions();
  const [all, setAll] = useState(false);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const shown = all ? queued : queued.slice(0, SHOWN);
  const ids = shown.map((item) => item.id);
  const order = queued.map((item) => item.id);
  const move = reorder.mutate;
  const first = useCallback((id: string) => move([id]), [move]);
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    move(arrayMove(order, order.indexOf(String(active.id)), order.indexOf(String(over.id))));
  };
  return (
    <>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          <ul aria-label={t('songs.sections.active')} className="flex flex-col">
            {running.map((item) => (
              <Row key={item.id} item={item} paused={paused} />
            ))}
            {shown.map((item, index) => (
              <QueuedRow
                key={item.id}
                item={item}
                paused={paused}
                onFirst={index === 0 ? undefined : first}
              />
            ))}
          </ul>
        </SortableContext>
      </DndContext>
      <ShowAll
        hidden={queued.length - shown.length}
        onShow={() => setAll(true)}
        label={t('songs.moreWaiting', { count: queued.length - shown.length })}
      />
    </>
  );
};

/**
 * Downloads → Music (M46.10): songs downloading now and waiting (in the order they run, to be
 * reordered, paused or cancelled), the ones that didn't download (why, and what can be done), and
 * the ones on this device, which save as files. `compact` (the All tab) leaves the finished ones
 * as a line leading here.
 */
export const SongDownloadsList: FC<{ compact?: boolean }> = ({ compact = false }) => {
  const { t, i18n } = useTranslation();
  const downloads = useSongDownloads();
  const paused = useSongDownloadsPaused().data ?? false;
  const { setPaused, retryFailed, removeMany, check, redownload } = useSongDownloadActions();
  const [saving, setSaving] = useState<string[] | null>(null);
  const [allDone, setAllDone] = useState(false);
  const items = useMemo(() => downloads.data ?? [], [downloads.data]);
  const { running, queued, failed, attention, done } = useMemo(() => sectionsOf(items), [items]);
  const checked = check.data;
  const saveOne = useCallback((id: string) => setSaving([id]), []);
  const doneBytes = done.reduce((sum, item) => sum + (item.totalBytes ?? 0), 0);
  const doneLine = [
    t('songs.count', { count: done.length }),
    doneBytes > 0 ? formatBytes(doneBytes, i18n.language) : null,
  ]
    .filter(Boolean)
    .join(' · ');
  // Phones have no right click: songs' menus open from ⋮.
  const desktop = useIsDesktop();

  if (downloads.isSuccess && items.length === 0) {
    return (
      <EmptyState
        icon={Disc3}
        title={t('songs.emptyTitle')}
        body={t(desktop ? 'songs.emptyHint' : 'songs.emptyHintPhone')}
      />
    );
  }
  const waiting = running.length + queued.length;
  const shownDone = allDone ? done : done.slice(0, SHOWN);
  return (
    <div data-testid="song-downloads" className="flex flex-col gap-4">
      {(waiting > 0 || failed.length > 0) && (
        <div className="flex flex-wrap items-center gap-2">
          {waiting > 0 && (
            <button
              type="button"
              disabled={setPaused.isPending}
              onClick={() => setPaused.mutate(!paused)}
              className={CHIP}
            >
              {paused ? (
                <Play aria-hidden size={16} className="text-primary" />
              ) : (
                <Pause aria-hidden size={16} className="text-primary" />
              )}
              {t(paused ? 'songs.resume' : 'songs.pause')}
            </button>
          )}
          {failed.length > 0 && (
            <button
              type="button"
              disabled={retryFailed.isPending}
              onClick={() => retryFailed.mutate()}
              className={CHIP}
            >
              <RotateCw aria-hidden size={16} className="text-primary" />
              {t('songs.retryAll', { count: failed.length })}
            </button>
          )}
          {failed.length > 0 && (
            <button
              type="button"
              disabled={removeMany.isPending}
              onClick={() => removeMany.mutate(failed.map((item) => item.id))}
              className={CHIP}
            >
              <Trash2 aria-hidden size={16} />
              {t('songs.clearFailed', { count: failed.length })}
            </button>
          )}
          {queued.length > 1 && (
            <button
              type="button"
              disabled={removeMany.isPending}
              onClick={() => removeMany.mutate(queued.map((item) => item.id))}
              className={CHIP}
            >
              <X aria-hidden size={16} />
              {t('songs.cancelWaiting', { count: queued.length })}
            </button>
          )}
        </div>
      )}
      {waiting > 0 && (
        <Group label={t('songs.sections.active')} count={waiting}>
          <Queue running={running} queued={queued} paused={paused} />
        </Group>
      )}
      {failed.length > 0 && (
        <Group label={t('songs.sections.failed')} count={failed.length}>
          <ul aria-label={t('songs.sections.failed')} className="flex flex-col">
            {failed.map((item) => (
              <Row key={item.id} item={item} paused={paused} />
            ))}
          </ul>
        </Group>
      )}
      {attention.length > 0 && !compact && (
        <Group
          label={t('songs.sections.attention')}
          count={attention.length}
          aside={
            attention.length > 1 && (
              <button
                type="button"
                disabled={redownload.isPending}
                onClick={() => redownload.mutate(attention.map((item) => item.id))}
                className={`${CHIP} ml-auto`}
              >
                <RefreshCw aria-hidden size={16} className="text-primary" />
                {t('songs.downloadAllAgain', { count: attention.length })}
              </button>
            )
          }
        >
          <ul aria-label={t('songs.sections.attention')} className="flex flex-col">
            {attention.map((item) => (
              <Row key={item.id} item={item} paused={paused} />
            ))}
          </ul>
        </Group>
      )}
      {done.length > 0 && compact && (
        <Link
          to="/downloads"
          search={{ tab: 'music' }}
          className="text-muted hover:text-foreground self-start px-2 text-sm"
        >
          {t('songs.onDevice', { songs: doneLine })}
        </Link>
      )}
      {(done.length > 0 || attention.length > 0) && !compact && (
        <Group
          label={t('songs.sections.done')}
          count={done.length}
          aside={
            <span className="text-muted ml-auto flex items-center gap-1 text-xs">
              {doneBytes > 0 && formatBytes(doneBytes, i18n.language)}
              <Link
                to="/settings/downloads"
                aria-label={t('songs.settings')}
                data-hint={t('songs.settings')}
                className="hover:text-foreground grid size-8 place-items-center rounded-full"
              >
                <Settings2 aria-hidden size={16} />
              </Link>
            </span>
          }
        >
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={check.isPending}
              onClick={() => check.mutate(null)}
              className={CHIP}
            >
              <ListChecks aria-hidden size={16} className="text-primary" />
              {check.isPending ? t('songs.checking') : t('songs.check')}
            </button>
            {done.length > 1 && (
              <button
                type="button"
                onClick={() => setSaving(done.map((item) => item.id))}
                className={CHIP}
              >
                <FileDown aria-hidden size={16} />
                {t('songs.saveAll', { count: done.length })}
              </button>
            )}
            {checked && !check.isPending && (
              <span role="status" className="text-muted text-sm">
                {checked.problems > 0
                  ? t('songs.checkedProblems', { count: checked.problems })
                  : t('songs.checkedFine', { count: checked.checked })}
              </span>
            )}
            {check.isError && (
              <span role="alert" className="text-primary text-sm wrap-anywhere">
                {check.error.message}
              </span>
            )}
          </div>
          <ul aria-label={t('songs.title')} className="flex flex-col">
            {shownDone.map((item) => (
              <Row key={item.id} item={item} paused={paused} onSave={saveOne} />
            ))}
          </ul>
          <ShowAll
            hidden={done.length - shownDone.length}
            onShow={() => setAllDone(true)}
            label={t('songs.moreDone', { count: done.length - shownDone.length })}
          />
        </Group>
      )}
      {saving && <SaveSongsSheet ids={saving} onClose={() => setSaving(null)} />}
    </div>
  );
};
