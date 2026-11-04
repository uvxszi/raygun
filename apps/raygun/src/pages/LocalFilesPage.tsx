import type { LocalFolder } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  CircleAlert,
  Folder,
  FolderPlus,
  FolderX,
  LoaderCircle,
  RefreshCw,
  ShieldAlert,
  Trash2,
} from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalFolderActions, useLocalFolders } from '../localFiles/useLocalFolders';
import { useMediaGate } from '../permissions/useMediaGate';
import { ConfirmButton } from '../shell/ConfirmButton';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar, TopBarAction } from '../shell/TopBar';

const KINDS = ['songs', 'comics', 'books', 'videos'] as const;
type Kind = (typeof KINDS)[number];

/** Where each kind goes in the library. */
const KIND_LINKS: Record<Kind, Record<string, string>> = {
  songs: { view: 'tracks' },
  comics: { media: 'manga' },
  books: { media: 'novel' },
  videos: { media: 'video' },
};

const ROW_BUTTON =
  'text-primary hover:bg-background shrink-0 rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-60';

/** "12 songs · 3 comics": what a folder holds, leaving out kinds it has none of. */
const folderSummary = (
  folder: LocalFolder,
  t: (key: string, options?: Record<string, unknown>) => string,
) => {
  const parts = KINDS.filter((kind) => folder.counts[kind] > 0).map((kind) =>
    t(`localFiles.count.${kind}`, { count: folder.counts[kind] }),
  );
  if (parts.length > 0) return parts.join(' · ');
  return t(folder.scannedAt === null ? 'localFiles.nothingYet' : 'localFiles.nothingFound');
};

/** A line under a folder: what's wrong with it and the one thing to do. */
const Notice: FC<{ icon: typeof CircleAlert; children: ReactNode; action?: ReactNode }> = ({
  icon: Icon,
  children,
  action,
}) => (
  <span className="text-muted flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
    <span className="flex items-center gap-1.5">
      <Icon aria-hidden size={14} className="text-primary shrink-0" />
      {children}
    </span>
    {action}
  </span>
);

type Actions = ReturnType<typeof useLocalFolderActions>;

const FolderRow: FC<{ folder: LocalFolder; actions: Actions }> = ({ folder, actions }) => {
  const { t } = useTranslation();
  const { add, remove, rescan, allowAccess } = actions;
  const { progress } = folder;
  return (
    <li
      aria-label={folder.name}
      className="bg-surface flex items-start gap-3 rounded-2xl px-4 py-3"
    >
      <Folder aria-hidden size={22} className="text-primary mt-0.5 shrink-0" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-semibold">{folder.name}</span>
        <span className="text-muted truncate text-xs">{folder.location}</span>
        {folder.scanning ? (
          <span role="status" className="text-muted flex items-center gap-1.5 text-sm">
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
            {progress === null
              ? t('localFiles.listing')
              : progress.total > 0
                ? t('localFiles.adding', { done: progress.done, total: progress.total })
                : t('localFiles.scanning')}
          </span>
        ) : (
          <span className="text-sm">{folderSummary(folder, t)}</span>
        )}
        {folder.missing && (
          <Notice
            icon={FolderX}
            action={
              <button
                type="button"
                disabled={add.isPending}
                onClick={() => add.mutate()}
                className={ROW_BUTTON}
              >
                {t('localFiles.pickAgain')}
              </button>
            }
          >
            {t('localFiles.missing')}
          </Notice>
        )}
        {folder.needsAccess && (
          <Notice
            icon={ShieldAlert}
            action={
              <button
                type="button"
                disabled={allowAccess.isPending}
                onClick={() => allowAccess.mutate()}
                className={ROW_BUTTON}
              >
                {t('localFiles.allowAccess')}
              </button>
            }
          >
            {t('localFiles.needsAccess')}
          </Notice>
        )}
        {!folder.scanning && folder.counts.failed > 0 && (
          <Notice
            icon={CircleAlert}
            action={
              <button
                type="button"
                disabled={rescan.isPending}
                onClick={() => rescan.mutate(folder.id)}
                className={ROW_BUTTON}
              >
                {t('localFiles.tryAgain')}
              </button>
            }
          >
            {t('localFiles.failed', { count: folder.counts.failed })}
          </Notice>
        )}
        {folder.counts.unplayable > 0 && (
          <Notice icon={CircleAlert}>
            {t('localFiles.unplayable', { count: folder.counts.unplayable })}
          </Notice>
        )}
      </div>
      <button
        type="button"
        aria-label={t('localFiles.rescanOne', { name: folder.name })}
        disabled={folder.scanning || rescan.isPending}
        onClick={() => rescan.mutate(folder.id)}
        className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-40"
      >
        <RefreshCw aria-hidden size={18} />
      </button>
      <ConfirmButton
        label={t('localFiles.remove', { name: folder.name })}
        confirm={t('localFiles.removeConfirm')}
        icon={Trash2}
        disabled={remove.isPending}
        onConfirm={() => remove.mutate(folder.id)}
      />
    </li>
  );
};

/**
 * Local Files (M46.27): folders on this device, scanned for songs (into the music library),
 * comics and books (imported) and videos (Library → Videos).
 */
export const LocalFilesPage: FC = () => {
  const { t } = useTranslation();
  const folders = useLocalFolders();
  const actions = useLocalFolderActions();
  const { add, remove, rescan, allowAccess } = actions;
  const mediaGate = useMediaGate();
  const list = folders.data ?? [];
  const totals = KINDS.map(
    (kind) => [kind, list.reduce((sum, folder) => sum + folder.counts[kind], 0)] as const,
  ).filter(([, count]) => count > 0);
  const failure = [
    [add.error, 'localFiles.addFailed'],
    [remove.error, 'localFiles.removeFailed'],
    [rescan.error, 'localFiles.scanFailed'],
    [allowAccess.error, 'localFiles.accessFailed'],
    [folders.error, 'localFiles.loadFailed'],
  ].find(([error]) => error) as [Error, string] | undefined;

  return (
    <section data-testid="local-files-page" className="flex flex-col gap-4">
      <TopBar
        back
        fallback="/library"
        title={t('localFiles.title')}
        actions={
          <>
            {list.length > 0 && (
              <TopBarAction
                label={t('localFiles.rescan')}
                icon={RefreshCw}
                disabled={rescan.isPending || list.every((folder) => folder.scanning)}
                onClick={() => rescan.mutate(null)}
              />
            )}
            <TopBarAction
              label={t('localFiles.add')}
              icon={FolderPlus}
              disabled={add.isPending}
              onClick={() => void mediaGate.guard(() => add.mutate())}
            />
          </>
        }
      />
      {totals.length > 0 && (
        <nav aria-label={t('localFiles.goTo')} className="flex flex-wrap gap-2">
          {totals.map(([kind, count]) => (
            <Link
              key={kind}
              to="/library"
              search={KIND_LINKS[kind] as never}
              className="bg-surface flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm"
            >
              {t(`localFiles.kinds.${kind}`)}
              <span className="text-muted text-xs">{count}</span>
            </Link>
          ))}
        </nav>
      )}
      {mediaGate.sheet}
      {failure && (
        <SourceIssue compact title={t(failure[1])}>
          {failure[0].message}
        </SourceIssue>
      )}
      {folders.isSuccess && list.length === 0 && (
        <div className="flex flex-col items-center">
          <EmptyState
            icon={Folder}
            title={t('localFiles.empty')}
            action={{
              label: t('localFiles.add'),
              icon: FolderPlus,
              onClick: () => void mediaGate.guard(() => add.mutate()),
            }}
          />
          <ul
            aria-label={t('localFiles.holds')}
            className="-mt-8 flex flex-wrap justify-center gap-2 px-6"
          >
            {KINDS.map((kind) => (
              <li key={kind} className="bg-surface text-muted rounded-full px-3 py-1 text-xs">
                {t(`localFiles.holdsKinds.${kind}`)}
              </li>
            ))}
          </ul>
        </div>
      )}
      <ul className="flex flex-col gap-2">
        {list.map((folder) => (
          <FolderRow key={folder.id} folder={folder} actions={actions} />
        ))}
      </ul>
    </section>
  );
};
