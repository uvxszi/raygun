import { Link } from '@tanstack/react-router';
import { ArrowDownToLine, Bell, FileDown } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { POPOVER_DOWNLOADS } from '../downloads/barDownloads';
import {
  useChapterDownloadMenu,
  useFileDownloadMenu,
  useSongDownloadMenu,
  useVideoDownloadMenu,
} from '../downloads/downloadMenus';
import { chapterUpdateLink } from '../manga/chapterUpdateLink';
import { bookWords } from '../manga/bookWords';
import { MangaCover } from '../manga/MangaCover';
import { useMangaDownloads, useMangaLiveEvents, useMangaUpdates } from '../manga/useManga';
import { Artwork } from '../library/Artwork';
import { useSongDownloads, useSongDownloadsPaused } from '../music/useSongDownloads';
import { useSettings } from '../settings/useSettings';
import { episodeLabel } from '../watch/episodeLabel';
import { useVideoDownloads, useVideoUpdates, videoTitleLink } from '../watch/useVideoLibrary';
import { useFileDownloads } from '../websites/useFileDownloads';
import type { MenuItem } from './contextMenuStore';
import { MenuButton } from './MenuButton';
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
      picture: (
        <MangaCover
          url={item.coverUrl}
          book={bookWords(item.medium, item.title)}
          className="w-full rounded"
        />
      ),
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

/** A download in the bar's list: its row, whether it's under way, and its own menu. */
type DownloadRow = MiniRow & { busy: boolean; menu: () => MenuItem[] };

/** The downloads, each with its ⋮ (the menu Downloads gives it); the list scrolls. */
const DownloadList: FC<{ rows: DownloadRow[]; empty: string }> = ({ rows, empty }) => {
  const { t } = useTranslation();
  if (rows.length === 0) return <p className="text-muted px-3 py-4 text-sm">{empty}</p>;
  return (
    <ul
      aria-label={t('manga.downloads')}
      className="flex max-h-[min(60dvh,26rem)] flex-col overflow-y-auto"
    >
      {rows.map((row) => (
        <li
          key={row.key}
          data-testid="bar-download"
          className="hover:bg-background flex items-center gap-3 rounded-lg py-1.5 pr-1 pl-2"
        >
          <span className="w-9 shrink-0">{row.picture}</span>
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-sm font-medium">{row.title}</span>
            <span className="text-muted truncate text-xs">{row.detail}</span>
          </span>
          <MenuButton
            label={t('downloads.popover.options', { name: row.title })}
            items={row.menu}
            className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full"
            iconSize={16}
          />
        </li>
      ))}
    </ul>
  );
};

/**
 * Downloads under way first, then the latest, every kind together (M46.37): as many as Settings →
 * Downloads says (20 at first), each with its own menu (play or open, pause or resume, stop
 * asked first, try again, show in folder, delete), and Open Downloads under them.
 */
const DownloadsPopover: FC<{ anchor: HTMLElement; onClose: () => void }> = ({
  anchor,
  onClose,
}) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const shown = useSettings().data?.downloadsPopover ?? POPOVER_DOWNLOADS;
  const songs = useSongDownloads();
  const songsPaused = useSongDownloadsPaused().data ?? false;
  const chapters = useMangaDownloads();
  const videos = useVideoDownloads();
  const files = useFileDownloads();
  const songMenu = useSongDownloadMenu();
  const chapterMenu = useChapterDownloadMenu();
  const videoMenu = useVideoDownloadMenu();
  const fileMenu = useFileDownloadMenu();
  const stateText = (state: string) => t(`barPopover.state.${state}`, { defaultValue: state });
  const busy = (state: string) => state === 'downloading' || state === 'queued';
  const chaptersPaused = chapters.data?.paused ?? false;
  const videosPaused = videos.data?.paused ?? false;
  // Saved songs taken off the downloads' list stay off it here too.
  const listedSongs = (songs.data ?? []).filter((song) => !song.unlisted);
  const rows: DownloadRow[] = [
    ...listedSongs.map((song) => ({
      key: `s-${song.id}`,
      picture: <Artwork url={song.artworkUrl} className="w-full rounded" iconSize={16} />,
      title: song.title,
      detail: [t('barPopover.song'), song.artist, stateText(song.state)]
        .filter(Boolean)
        .join(' · '),
      at: song.finishedAt ?? song.queuedAt ?? 0,
      busy: busy(song.state),
      menu: () => songMenu(song, songsPaused),
    })),
    ...(chapters.data?.items ?? []).map((item, index) => ({
      key: `m-${item.chapterId}`,
      picture: <MangaCover url={null} className="w-full rounded" />,
      title: item.title,
      detail: [item.chapter, stateText(item.download.state)].join(' · '),
      // The queue has no times; it's in its own order.
      at: Number.MAX_SAFE_INTEGER - index,
      busy: busy(item.download.state),
      menu: () => chapterMenu(item, chaptersPaused),
    })),
    ...(videos.data?.items ?? []).map((item) => ({
      key: `v-${item.id}`,
      picture: <MangaCover url={item.poster} video className="w-full rounded" />,
      title: item.title,
      detail: [item.subtitle, stateText(item.state)].filter(Boolean).join(' · '),
      at: item.at ?? 0,
      busy: busy(item.state),
      menu: () => videoMenu(item, videosPaused),
    })),
    ...(files.data ?? []).map((file) => ({
      key: `f-${file.id}`,
      picture: (
        <span className="bg-background text-primary grid aspect-square w-full place-items-center rounded">
          <FileDown aria-hidden size={16} />
        </span>
      ),
      title: file.name,
      detail: [file.site, stateText(file.state)].filter(Boolean).join(' · '),
      at: file.createdAt ?? 0,
      busy: busy(file.state),
      menu: () => fileMenu(file),
    })),
  ]
    .sort((a, b) => Number(b.busy) - Number(a.busy) || b.at - a.at)
    .slice(0, shown);
  return (
    <Frame
      anchor={anchor}
      title={t('manga.downloads')}
      open={t('barPopover.openDownloads')}
      to="/downloads"
      onClose={onClose}
    >
      <DownloadList rows={rows} empty={t('barPopover.noDownloads')} />
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
