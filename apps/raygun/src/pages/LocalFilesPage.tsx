import type { LocalFilesAdded, LocalFolder } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  CircleAlert,
  FilePlus,
  Files,
  Folder,
  FolderPlus,
  FolderX,
  LoaderCircle,
  RefreshCw,
  ShieldAlert,
  Trash2,
} from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { FolderBrowser } from '../localFiles/FolderBrowser';
import { useLocalFolderActions, useLocalFolders } from '../localFiles/useLocalFolders';
import { useMediaGate } from '../permissions/useMediaGate';
import { ConfirmButton } from '../shell/ConfirmButton';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useIsDesktop } from '../shell/useIsDesktop';

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

/** A folder's name as the page shows it: Raygun's own folder of added files has a fixed one. */
const shownName = (folder: LocalFolder, t: (key: string) => string) =>
  folder.access === 'added' ? t('localFiles.addedFolder') : folder.name;

const FolderRow: FC<{ folder: LocalFolder; actions: Actions; onPickAgain: () => void }> = ({
  folder,
  actions,
  onPickAgain,
}) => {
  const { t } = useTranslation();
  const { add, remove, rescan, allowAccess } = actions;
  const { progress } = folder;
  const name = shownName(folder, t);
  const Icon = folder.access === 'added' ? Files : Folder;
  return (
    <li aria-label={name} className="bg-surface flex items-start gap-3 rounded-2xl px-4 py-3">
      <Icon aria-hidden size={22} className="text-primary mt-0.5 shrink-0" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-semibold">{name}</span>
        <span className="text-muted truncate text-xs">
          {folder.access === 'added' ? t('localFiles.addedWhere') : folder.location}
        </span>
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
              folder.access !== 'added' && (
                <button
                  type="button"
                  disabled={add.isPending}
                  onClick={onPickAgain}
                  className={ROW_BUTTON}
                >
                  {t('localFiles.pickAgain')}
                </button>
              )
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
        aria-label={t('localFiles.rescanOne', { name })}
        disabled={folder.scanning || rescan.isPending}
        onClick={() => rescan.mutate(folder.id)}
        className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-40"
      >
        <RefreshCw aria-hidden size={18} />
      </button>
      <ConfirmButton
        label={t('localFiles.remove', { name })}
        // Raygun's own copies go with their folder.
        confirm={t(
          folder.access === 'added' ? 'localFiles.removeCopies' : 'localFiles.removeConfirm',
        )}
        icon={Trash2}
        disabled={remove.isPending}
        onConfirm={() => remove.mutate(folder.id)}
      />
    </li>
  );
};

/** What Add files did: each kind it added, linked to where it went, and what it couldn't add. */
const AddedNotice: FC<{ added: LocalFilesAdded }> = ({ added }) => {
  const { t } = useTranslation();
  const kinds = KINDS.filter((kind) => added[kind] > 0);
  return (
    <>
      {kinds.length > 0 && (
        <p role="status" className="flex flex-wrap items-center gap-2 text-sm">
          {t('localFiles.addedFiles')}
          {kinds.map((kind) => (
            <Link
              key={kind}
              to="/library"
              search={KIND_LINKS[kind] as never}
              className="bg-surface rounded-full px-3 py-1 text-xs"
            >
              {t(`localFiles.count.${kind}`, { count: added[kind] })}
            </Link>
          ))}
        </p>
      )}
      {added.failed.length > 0 && (
        <SourceIssue compact title={t('localFiles.failed', { count: added.failed.length })}>
          {added.failed.map((failure) => `${failure.name}: ${failure.error}`).join('\n')}
        </SourceIssue>
      )}
    </>
  );
};

/**
 * Local Files (M46.27): folders on this device, scanned for songs (into the music library),
 * comics and books (imported) and videos (Library → Videos). On Android a folder is chosen in
 * Raygun's own folder browser (M46.51), since Android's picker refuses Download and the storage's
 * top; Add files takes single files from anywhere.
 */
export const LocalFilesPage: FC = () => {
  const { t } = useTranslation();
  const folders = useLocalFolders();
  const actions = useLocalFolderActions();
  const { add, addFiles, remove, rescan, allowAccess } = actions;
  const mediaGate = useMediaGate();
  // Desktops keep their own folder dialog; phones (and a platform not known yet) browse.
  const desktop = useIsDesktop();
  const [browsing, setBrowsing] = useState(false);
  const list = folders.data ?? [];
  const totals = KINDS.map(
    (kind) => [kind, list.reduce((sum, folder) => sum + folder.counts[kind], 0)] as const,
  ).filter(([, count]) => count > 0);
  const failure = [
    [add.error, 'localFiles.addFailed'],
    [addFiles.error, 'localFiles.addFilesFailed'],
    [remove.error, 'localFiles.removeFailed'],
    [rescan.error, 'localFiles.scanFailed'],
    [allowAccess.error, 'localFiles.accessFailed'],
    [folders.error, 'localFiles.loadFailed'],
  ].find(([error]) => error) as [Error, string] | undefined;

  const addFolder = () => void mediaGate.guard(() => (desktop ? add.mutate() : setBrowsing(true)));
  const pickAgain = (folder: LocalFolder) =>
    folder.access === 'path' && !desktop ? setBrowsing(true) : add.mutate();

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
              label={t('localFiles.addFiles')}
              icon={FilePlus}
              disabled={addFiles.isPending}
              onClick={() => addFiles.mutate()}
            />
            <TopBarAction
              label={t('localFiles.add')}
              icon={FolderPlus}
              disabled={add.isPending}
              onClick={addFolder}
            />
          </>
        }
      />
      {browsing && (
        <FolderBrowser
          onClose={() => setBrowsing(false)}
          onSystemPicker={() => {
            setBrowsing(false);
            add.mutate();
          }}
        />
      )}
      {addFiles.isPending && (
        <p role="status" className="text-muted flex items-center gap-1.5 text-sm">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('localFiles.addingFiles')}
        </p>
      )}
      {addFiles.data && <AddedNotice added={addFiles.data} />}
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
            action={{ label: t('localFiles.add'), icon: FolderPlus, onClick: addFolder }}
          />
          <button
            type="button"
            disabled={addFiles.isPending}
            onClick={() => addFiles.mutate()}
            className="text-primary hover:bg-surface -mt-12 mb-6 flex items-center gap-2 rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
          >
            <FilePlus aria-hidden size={16} />
            {t('localFiles.addFiles')}
          </button>
          <ul
            aria-label={t('localFiles.holds')}
            className="flex flex-wrap justify-center gap-2 px-6"
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
          <FolderRow
            key={folder.id}
            folder={folder}
            actions={actions}
            onPickAgain={() => pickAgain(folder)}
          />
        ))}
      </ul>
    </section>
  );
};
