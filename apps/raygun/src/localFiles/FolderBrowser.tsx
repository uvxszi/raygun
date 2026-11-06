import type { PermissionKind } from '@raygun/bindings';
import { ChevronRight, Folder, FolderSearch, LoaderCircle, ShieldAlert } from 'lucide-react';
import { Fragment, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useRequestPermission } from '../permissions/usePermissions';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { useLocalBrowse, useLocalFolderActions } from './useLocalFolders';

type FolderBrowserProps = {
  onClose: () => void;
  /** Android's own folder picker, for the folders it allows (SD cards, USB drives). */
  onSystemPicker: () => void;
};

const Crumb: FC<{ current: boolean; onOpen: () => void; children: ReactNode }> = ({
  current,
  onOpen,
  children,
}) =>
  current ? (
    <span aria-current="location" className="truncate font-semibold">
      {children}
    </span>
  ) : (
    <button
      type="button"
      onClick={onOpen}
      className="text-primary hover:bg-background truncate rounded-md px-1 py-0.5"
    >
      {children}
    </button>
  );

/** What the browser can't show yet, and the one thing that shows it. */
const AccessNotice: FC<{ text: string; action: string; busy: boolean; onAllow: () => void }> = ({
  text,
  action,
  busy,
  onAllow,
}) => (
  <div className="bg-background flex flex-col gap-2 rounded-xl px-3 py-2.5 text-sm">
    <span className="flex items-start gap-2">
      <ShieldAlert aria-hidden size={16} className="text-primary mt-0.5 shrink-0" />
      {text}
    </span>
    <button
      type="button"
      disabled={busy}
      onClick={onAllow}
      className="text-primary self-start rounded-full px-1 text-sm font-semibold disabled:opacity-60"
    >
      {action}
    </button>
  </div>
);

/**
 * Local Files' own folder browser (M46.51): the phone's storage from its top, each folder with
 * how many files it holds that Raygun plays or reads, and "Use this folder". Android's folder
 * picker refuses the storage's top, Download and Android/data, so those are added here; the
 * system picker stays for what it allows (SD cards, USB drives).
 */
export const FolderBrowser: FC<FolderBrowserProps> = ({ onClose, onSystemPicker }) => {
  const { t } = useTranslation();
  const [path, setPath] = useState<string | null>(null);
  const listing = useLocalBrowse(path);
  const { addPath } = useLocalFolderActions();
  const request = useRequestPermission();
  const data = listing.data;
  // The folder before stays while the next one loads.
  const loading = listing.isPending || listing.isPlaceholderData;
  const allow = (kind: PermissionKind) =>
    request.mutate(kind, { onSettled: () => void listing.refetch() });

  return (
    <Sheet title={t('localFiles.browser.title')} onClose={onClose}>
      <nav
        aria-label={t('localFiles.browser.trail')}
        className="flex min-w-0 flex-wrap items-center gap-0.5 text-sm"
      >
        <Crumb current={!!data && data.trail.length === 0} onOpen={() => setPath(null)}>
          {t('localFiles.browser.top')}
        </Crumb>
        {data?.trail.map((crumb, index) => (
          <Fragment key={crumb.path}>
            <ChevronRight aria-hidden size={14} className="text-muted shrink-0" />
            <Crumb current={index === data.trail.length - 1} onOpen={() => setPath(crumb.path)}>
              {crumb.name}
            </Crumb>
          </Fragment>
        ))}
        {loading && (
          <LoaderCircle
            role="status"
            aria-label={t('localFiles.browser.loading')}
            size={14}
            className="text-muted ml-1 animate-spin"
          />
        )}
      </nav>

      {data?.view === 'none' && (
        <AccessNotice
          text={t('localFiles.browser.needsMedia')}
          action={t('localFiles.allowAccess')}
          busy={request.isPending}
          onAllow={() => allow('media')}
        />
      )}
      {data?.view === 'media' && data.canAllowAllFiles && (
        <AccessNotice
          text={t('localFiles.browser.mediaOnly')}
          action={t('localFiles.browser.allowAllFiles')}
          busy={request.isPending}
          onAllow={() => allow('allFiles')}
        />
      )}
      {listing.isError && (
        <SourceIssue compact title={t('localFiles.browser.unreadable')}>
          {listing.error.message}
        </SourceIssue>
      )}

      {data && (
        <div className="flex flex-col gap-1">
          {data.items > 0 && (
            <p className="text-muted px-1 text-xs">
              {t('localFiles.browser.here', { count: data.items })}
            </p>
          )}
          {data.folders.length > 0 ? (
            <ul aria-label={t('localFiles.browser.folders')} className="-mx-2 flex flex-col">
              {data.folders.map((folder) => (
                <li key={folder.path}>
                  <button
                    type="button"
                    disabled={loading}
                    onClick={() => setPath(folder.path)}
                    className="hover:bg-background flex w-full items-center gap-3 rounded-xl px-2 py-2.5 text-left"
                  >
                    <Folder aria-hidden size={20} className="text-primary shrink-0" />
                    <span className="min-w-0 flex-1 truncate">{folder.name}</span>
                    {folder.items !== null && (
                      <span className="text-muted shrink-0 text-xs">
                        {t('localFiles.items', { count: folder.items })}
                      </span>
                    )}
                    <ChevronRight aria-hidden size={16} className="text-muted shrink-0" />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            data.view !== 'none' && (
              <p className="text-muted px-1 py-2 text-sm">{t('localFiles.browser.noFolders')}</p>
            )
          )}
        </div>
      )}

      {addPath.error && (
        <SourceIssue compact title={t('localFiles.addFailed')}>
          {addPath.error.message}
        </SourceIssue>
      )}
      <div className="flex flex-col gap-1">
        <button
          type="button"
          disabled={!data || loading || data.added || addPath.isPending}
          onClick={() => data && addPath.mutate(data.path, { onSuccess: onClose })}
          className="bg-primary text-background flex items-center justify-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold disabled:opacity-60"
        >
          {addPath.isPending && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
          {data?.added ? t('localFiles.browser.added') : t('localFiles.browser.use')}
        </button>
        <button
          type="button"
          onClick={onSystemPicker}
          className="text-primary hover:bg-background flex items-center justify-center gap-2 rounded-full px-5 py-2 text-sm"
        >
          <FolderSearch aria-hidden size={16} />
          {t('localFiles.browser.systemPicker')}
        </button>
      </div>
    </Sheet>
  );
};
