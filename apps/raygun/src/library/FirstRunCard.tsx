import { FolderSearch } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AddFolderForm } from './AddFolderForm';
import { useConnectDeviceMusic } from './useLibrary';

/** Shown until the library has a folder: find the device's music, or add a folder by path. */
export const FirstRunCard: FC = () => {
  const { t } = useTranslation();
  const connect = useConnectDeviceMusic();
  return (
    <section
      data-testid="library-first-run"
      className="bg-surface flex flex-col items-center gap-4 rounded-xl px-6 py-8 text-center"
    >
      <FolderSearch aria-hidden size={40} className="text-muted" />
      <div className="flex flex-col gap-1">
        <p className="font-semibold">{t('library.emptyTitle')}</p>
        <p className="text-muted text-sm">{t('library.firstRunBody')}</p>
      </div>
      <button
        type="button"
        disabled={connect.isPending}
        onClick={() => connect.mutate()}
        className="bg-primary text-background rounded-lg px-5 py-2.5 font-semibold disabled:opacity-50"
      >
        {t('library.findDeviceMusic')}
      </button>
      {connect.isError && (
        <p role="alert" className="text-primary text-sm">
          {connect.error.message}
        </p>
      )}
      <AddFolderForm />
    </section>
  );
};
