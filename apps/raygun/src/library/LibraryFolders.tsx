import { commands } from '@raygun/bindings';
import { RefreshCw, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddFolderForm } from './AddFolderForm';
import { useRemoveRoot } from './useLibrary';

/** Folder management, tucked away below the library. */
export const LibraryFolders: FC<{ roots: string[] }> = ({ roots }) => {
  const { t } = useTranslation();
  const removeRoot = useRemoveRoot();
  return (
    <details className="bg-surface rounded-xl px-5 py-3 text-sm">
      <summary className="cursor-pointer font-medium">
        {t('library.folders', { count: roots.length })}
      </summary>
      <ul className="mt-3 flex flex-col gap-1">
        {roots.map((root) => (
          <li key={root} className="flex items-center gap-2">
            <span className="min-w-0 flex-1 truncate">{root}</span>
            <button
              type="button"
              aria-label={t('library.removeFolder', { path: root })}
              onClick={() => removeRoot.mutate(root)}
              className="text-muted"
            >
              <X aria-hidden size={18} />
            </button>
          </li>
        ))}
      </ul>
      <div className="mt-3 flex flex-col gap-3">
        <button
          type="button"
          onClick={() => void commands.libraryRescan()}
          className="bg-background flex w-fit items-center gap-2 rounded-lg px-4 py-2"
        >
          <RefreshCw aria-hidden size={16} />
          {t('library.rescan')}
        </button>
        <AddFolderForm />
      </div>
    </details>
  );
};
