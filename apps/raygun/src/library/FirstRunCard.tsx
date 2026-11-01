import { FolderPlus, FolderSearch } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddFolderSheet } from './AddFolderForm';
import { useConnectDeviceMusic } from './useLibrary';

/** Shown until the library has a folder: find the device's music, or add a folder by path. */
export const FirstRunCard: FC = () => {
  const { t } = useTranslation();
  const connect = useConnectDeviceMusic();
  const [adding, setAdding] = useState(false);
  return (
    <section
      data-testid="library-first-run"
      className="flex flex-col items-center gap-4 px-6 py-12 text-center"
    >
      <FolderSearch aria-hidden size={56} className="text-primary" />
      <div className="flex flex-col gap-1">
        <p className="font-semibold">{t('library.emptyTitle')}</p>
        <p className="text-muted text-sm">{t('library.firstRunBody')}</p>
      </div>
      <button
        type="button"
        disabled={connect.isPending}
        onClick={() => connect.mutate()}
        className="bg-primary text-background rounded-full px-6 py-3 font-semibold disabled:opacity-50"
      >
        {t('library.findDeviceMusic')}
      </button>
      <button
        type="button"
        onClick={() => setAdding(true)}
        className="text-primary flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium"
      >
        <FolderPlus aria-hidden size={18} />
        {t('library.addFolder')}
      </button>
      {connect.isError && (
        <p role="alert" className="text-primary text-sm">
          {connect.error.message}
        </p>
      )}
      {adding && <AddFolderSheet onClose={() => setAdding(false)} />}
    </section>
  );
};
