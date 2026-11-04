import { Link } from '@tanstack/react-router';
import { FolderPlus, FolderSearch, Library, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddFolderSheet } from './AddFolderForm';
import { useConnectDeviceMusic } from './useLibrary';

const PRIMARY =
  'bg-primary text-background flex items-center gap-2 rounded-full px-6 py-3 font-semibold disabled:opacity-50';
const SECONDARY =
  'text-primary flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium disabled:opacity-50';

/**
 * Shown until the music library has a folder: find the device's music, or add a folder by path.
 * `empty` when nothing else is in the library either: then finding something in Browse leads.
 */
export const FirstRunCard: FC<{ empty?: boolean }> = ({ empty = false }) => {
  const { t } = useTranslation();
  const connect = useConnectDeviceMusic();
  const [adding, setAdding] = useState(false);
  const Icon = empty ? Library : FolderSearch;
  return (
    <section
      data-testid="library-first-run"
      className="flex flex-col items-center gap-4 px-6 py-12 text-center"
    >
      <Icon aria-hidden size={56} className="text-primary" />
      <p className="font-semibold">
        {t(empty ? 'library.emptyTitle' : 'library.deviceMusicTitle')}
      </p>
      {empty && (
        <Link to="/browse" search={{ tab: 'search', type: 'music' }} className={PRIMARY}>
          <Search aria-hidden size={18} />
          {t('library.findInBrowse')}
        </Link>
      )}
      <button
        type="button"
        disabled={connect.isPending}
        onClick={() => connect.mutate()}
        className={empty ? SECONDARY : PRIMARY}
      >
        {empty && <FolderSearch aria-hidden size={18} />}
        {t('library.findDeviceMusic')}
      </button>
      <button type="button" onClick={() => setAdding(true)} className={SECONDARY}>
        <FolderPlus aria-hidden size={18} />
        {t('library.addFolder')}
      </button>
      {connect.isError && (
        // Rust says what's missing in plain words (a permission, a folder to add).
        <p role="alert" className="text-primary text-sm">
          {connect.error.message}
        </p>
      )}
      {adding && <AddFolderSheet onClose={() => setAdding(false)} />}
    </section>
  );
};
