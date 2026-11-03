import type { LocalFolder } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Folder, FolderPlus, LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalFolderActions, useLocalFolders } from '../localFiles/useLocalFolders';
import { TopBar, TopBarAction } from '../shell/TopBar';

/** "12 songs · 3 comics": what a folder holds, leaving out kinds it has none of. */
const folderSummary = (
  folder: LocalFolder,
  t: (key: string, options?: Record<string, unknown>) => string,
) => {
  const parts = (['songs', 'comics', 'books', 'videos'] as const)
    .filter((kind) => folder.counts[kind] > 0)
    .map((kind) => t(`localFiles.count.${kind}`, { count: folder.counts[kind] }));
  return parts.length > 0 ? parts.join(' · ') : t('localFiles.nothingYet');
};

/**
 * Local Files (M46.27): folders on this device, scanned for songs (into the music library),
 * comics and books (imported) and videos (Library → Videos).
 */
export const LocalFilesPage: FC = () => {
  const { t } = useTranslation();
  const folders = useLocalFolders();
  const { add, remove, rescan } = useLocalFolderActions();
  const list = folders.data ?? [];

  return (
    <section data-testid="local-files-page" className="flex flex-col gap-4">
      <TopBar
        back
        fallback="/browse"
        title={t('localFiles.title')}
        actions={
          <>
            {list.length > 0 && (
              <TopBarAction
                label={t('localFiles.rescan')}
                icon={RefreshCw}
                disabled={rescan.isPending}
                onClick={() => rescan.mutate()}
              />
            )}
            <TopBarAction
              label={t('localFiles.add')}
              icon={FolderPlus}
              disabled={add.isPending}
              onClick={() => add.mutate()}
            />
          </>
        }
      />
      <p className="text-muted text-sm">{t('localFiles.intro')}</p>
      {(add.error ?? remove.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(add.error ?? remove.error)?.message}
        </p>
      )}
      {folders.isSuccess && list.length === 0 && (
        <div className="flex flex-col items-center gap-3 py-10 text-center">
          <Folder aria-hidden size={40} className="text-primary" />
          <p className="font-semibold">{t('localFiles.empty')}</p>
          <button
            type="button"
            disabled={add.isPending}
            onClick={() => add.mutate()}
            className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
          >
            {t('localFiles.add')}
          </button>
        </div>
      )}
      <ul className="flex flex-col gap-2">
        {list.map((folder) => (
          <li
            key={folder.id}
            aria-label={folder.name}
            className="bg-surface flex items-center gap-3 rounded-2xl px-4 py-3"
          >
            <Folder aria-hidden size={22} className="text-primary shrink-0" />
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="truncate font-semibold">{folder.name}</span>
              <span className="text-muted truncate text-xs">{folder.location}</span>
              <span className="text-sm">
                {folder.scanning ? (
                  <span className="text-muted flex items-center gap-1.5">
                    <LoaderCircle aria-hidden size={14} className="animate-spin" />
                    {t('localFiles.scanning')}
                  </span>
                ) : (
                  folderSummary(folder, t)
                )}
              </span>
            </div>
            <button
              type="button"
              aria-label={t('localFiles.remove', { name: folder.name })}
              disabled={remove.isPending}
              onClick={() => remove.mutate(folder.id)}
              className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full"
            >
              <Trash2 aria-hidden size={18} />
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
};

/** Browse → Sources' Local Files block (M46.27): the folders, and a way in. */
export const LocalFilesSection: FC = () => {
  const { t } = useTranslation();
  const folders = useLocalFolders();
  const list = folders.data ?? [];
  return (
    <section aria-label={t('localFiles.title')} className="flex flex-col gap-1 pt-2">
      <h2 className="text-muted px-1 text-xs font-semibold tracking-wide uppercase">
        {t('localFiles.title')}
      </h2>
      <LocalFilesLink
        label={
          list.length > 0 ? t('localFiles.folders', { count: list.length }) : t('localFiles.add')
        }
        summary={
          list.length > 0 ? list.map((folder) => folder.name).join(', ') : t('localFiles.blurb')
        }
      />
    </section>
  );
};

const LocalFilesLink: FC<{ label: string; summary: string }> = ({ label, summary }) => (
  <Link
    to="/local-files"
    className="hover:bg-surface flex items-center gap-3 rounded-xl px-2 py-2.5"
  >
    <span className="bg-surface text-primary grid size-11 shrink-0 place-items-center rounded-xl">
      <Folder aria-hidden size={20} />
    </span>
    <span className="flex min-w-0 flex-col">
      <span className="truncate font-medium">{label}</span>
      <span className="text-muted truncate text-xs">{summary}</span>
    </span>
  </Link>
);
