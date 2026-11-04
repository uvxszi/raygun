import { Link } from '@tanstack/react-router';
import { ArrowDownToLine, Bell } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { chapterUpdateLink } from '../manga/chapterUpdateLink';
import { MangaCover } from '../manga/MangaCover';
import { useMangaDownloads, useMangaLiveEvents, useMangaUpdates } from '../manga/useManga';
import { Artwork } from '../library/Artwork';
import { useSongDownloads } from '../music/useSongDownloads';
import { episodeLabel } from '../watch/episodeLabel';
import { useVideoDownloads, useVideoUpdates, videoTitleLink } from '../watch/useVideoLibrary';
import { Popover } from './Popover';

/** Rows a bar popover shows before Open… (M46.37). */
const SHOWN = 8;

/** One row of a bar popover: a picture, two lines, and where it leads. */
type MiniRow = {
  key: string;
  picture: ReactNode;
  title: string;
  detail: string;
  at: number;
  link?: { to: string; search: Record<string, unknown> };
};

const MiniList: FC<{ rows: MiniRow[]; empty: string; onOpen: () => void }> = ({
  rows,
  empty,
  onOpen,
}) => {
  if (rows.length === 0) return <p className="text-muted px-3 py-4 text-sm">{empty}</p>;
  return (
    <ul className="flex max-h-[min(60dvh,26rem)] flex-col overflow-y-auto">
      {rows.slice(0, SHOWN).map((row) => {
        const body = (
          <>
            <span className="w-9 shrink-0">{row.picture}</span>
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="truncate text-sm font-medium">{row.title}</span>
              <span className="text-muted truncate text-xs">{row.detail}</span>
            </span>
          </>
        );
        return (
          <li key={row.key}>
            {row.link ? (
              <Link
                to={row.link.to}
                search={row.link.search as never}
                onClick={onOpen}
                className="hover:bg-background flex items-center gap-3 rounded-lg px-2 py-1.5"
              >
                {body}
              </Link>
            ) : (
              <div className="flex items-center gap-3 px-2 py-1.5">{body}</div>
            )}
          </li>
        );
      })}
    </ul>
  );
};

/** The popover's frame: its heading, the list, and Open the full page. */
const Frame: FC<{
  anchor: HTMLElement;
  title: string;
  open: string;
  to: string;
  onClose: () => void;
  children: ReactNode;
}> = ({ anchor, title, open, to, onClose, children }) => (
  <Popover anchor={anchor} label={title} onClose={onClose} width={360}>
    <div className="flex flex-col gap-1 p-1">
      <h2 className="px-2 pt-1 text-sm font-semibold">{title}</h2>
      {children}
      <Link
        to={to}
        onClick={onClose}
        className="bg-background hover:bg-primary/20 mt-1 rounded-full px-4 py-2 text-center text-sm font-semibold"
      >
        {open}
      </Link>
    </div>
  </Popover>
);

/** New chapters and episodes, newest first (M46.37: the bell's popover). */
const UpdatesPopover: FC<{ anchor: HTMLElement; onClose: () => void }> = ({ anchor, onClose }) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const chapters = useMangaUpdates();
  const episodes = useVideoUpdates();
  const rows: MiniRow[] = [
    ...(chapters.data ?? []).map((item) => ({
      key: `c-${item.chapter.id}`,
      picture: <MangaCover url={item.coverUrl} className="w-full rounded" />,
      title: item.title,
      detail: item.chapter.name,
      at: item.foundAt ?? 0,
      link: chapterUpdateLink(item),
    })),
    ...(episodes.data ?? []).map((item) => ({
      key: `e-${item.episodeId}`,
      picture: <MangaCover url={item.coverUrl} video className="w-full rounded" />,
      title: item.title,
      detail: episodeLabel(item, true, item.title),
      at: item.foundAt ?? item.date ?? 0,
      link: videoTitleLink(item.sourceId, item.remoteId),
    })),
  ].sort((a, b) => b.at - a.at);
  return (
    <Frame
      anchor={anchor}
      title={t('manga.updates')}
      open={t('barPopover.openUpdates')}
      to="/updates"
      onClose={onClose}
    >
      <MiniList rows={rows} empty={t('barPopover.noUpdates')} onOpen={onClose} />
    </Frame>
  );
};

/** Downloads under way first, then the latest done, every kind together (M46.37). */
const DownloadsPopover: FC<{ anchor: HTMLElement; onClose: () => void }> = ({
  anchor,
  onClose,
}) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const songs = useSongDownloads();
  const chapters = useMangaDownloads();
  const videos = useVideoDownloads();
  const stateText = (state: string) => t(`barPopover.state.${state}`, { defaultValue: state });
  const busy = (state: string) => state === 'downloading' || state === 'queued';
  const rows: (MiniRow & { busy: boolean })[] = [
    ...(songs.data ?? []).map((song, index) => ({
      key: `s-${song.id}`,
      picture: <Artwork url={song.artworkUrl} className="w-full rounded" iconSize={16} />,
      title: song.title,
      detail: [t('barPopover.song'), song.artist, stateText(song.state)]
        .filter(Boolean)
        .join(' · '),
      // Newest first already: keep that order among songs.
      at: -index,
      busy: busy(song.state),
    })),
    ...(chapters.data?.items ?? []).map((item, index) => ({
      key: `m-${item.chapterId}`,
      picture: <MangaCover url={null} className="w-full rounded" />,
      title: item.title,
      detail: [item.chapter, stateText(item.download.state)].join(' · '),
      at: -index,
      busy: busy(item.download.state),
    })),
    ...(videos.data?.items ?? []).map((item, index) => ({
      key: `v-${item.id}`,
      picture: <MangaCover url={item.poster} video className="w-full rounded" />,
      title: item.title,
      detail: [item.subtitle, stateText(item.state)].filter(Boolean).join(' · '),
      at: -index,
      busy: busy(item.state),
    })),
  ].sort((a, b) => Number(b.busy) - Number(a.busy) || b.at - a.at);
  return (
    <Frame
      anchor={anchor}
      title={t('manga.downloads')}
      open={t('barPopover.openDownloads')}
      to="/downloads"
      onClose={onClose}
    >
      <MiniList rows={rows} empty={t('barPopover.noDownloads')} onOpen={onClose} />
    </Frame>
  );
};

/**
 * The desktop bar's Updates and Downloads (M46.37): each opens a popover beside it with a short
 * list and a way to the full page, instead of leaving the page.
 */
export const BarPopoverButtons: FC<{ className: string }> = ({ className }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState<{ which: 'updates' | 'downloads'; anchor: HTMLElement } | null>(
    null,
  );
  const toggle = (which: 'updates' | 'downloads', anchor: HTMLElement) =>
    setOpen((current) => (current?.which === which ? null : { which, anchor }));
  return (
    <>
      <button
        type="button"
        aria-label={t('nav.updates')}
        data-hint={t('nav.updates')}
        aria-expanded={open?.which === 'updates'}
        aria-haspopup="dialog"
        onClick={(event) => toggle('updates', event.currentTarget)}
        className={className}
      >
        <Bell aria-hidden size={19} />
      </button>
      <button
        type="button"
        aria-label={t('nav.downloads')}
        data-hint={t('nav.downloads')}
        aria-expanded={open?.which === 'downloads'}
        aria-haspopup="dialog"
        onClick={(event) => toggle('downloads', event.currentTarget)}
        className={className}
      >
        <ArrowDownToLine aria-hidden size={19} />
      </button>
      {open?.which === 'updates' && (
        <UpdatesPopover anchor={open.anchor} onClose={() => setOpen(null)} />
      )}
      {open?.which === 'downloads' && (
        <DownloadsPopover anchor={open.anchor} onClose={() => setOpen(null)} />
      )}
    </>
  );
};
