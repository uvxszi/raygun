import { commands } from '@raygun/bindings';
import { FolderPlus, RefreshCw, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddFolderSheet } from './AddFolderForm';
import { useRemoveRoot } from './useLibrary';

/** The music folders Raygun reads, with rescan and add (Settings → Library). */
export const LibraryFolders: FC<{ roots: string[] }> = ({ roots }) => {
  const { t } = useTranslation();
  const removeRoot = useRemoveRoot();
  const [adding, setAdding] = useState(false);
  return (
    <div data-testid="library-folders" className="flex flex-col gap-2 text-sm">
      {roots.length === 0 && <p className="text-muted">{t('library.noFolders')}</p>}
      <ul className="flex flex-col gap-1">
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
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="bg-background flex items-center gap-2 rounded-full px-4 py-2"
        >
          <FolderPlus aria-hidden size={16} />
          {t('library.addFolder')}
        </button>
        <button
          type="button"
          disabled={roots.length === 0}
          onClick={() => void commands.libraryRescan()}
          className="bg-background flex items-center gap-2 rounded-full px-4 py-2 disabled:opacity-50"
        >
          <RefreshCw aria-hidden size={16} />
          {t('library.rescan')}
        </button>
      </div>
      {adding && <AddFolderSheet onClose={() => setAdding(false)} />}
    </div>
  );
};
