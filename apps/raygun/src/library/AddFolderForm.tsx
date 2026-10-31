import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useAddRoot } from './useLibrary';

export const AddFolderForm: FC = () => {
  const { t } = useTranslation();
  const [path, setPath] = useState('');
  const addRoot = useAddRoot();

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (path.trim()) {
      addRoot.mutate(path.trim(), { onSuccess: () => setPath('') });
    }
  };

  return (
    <form onSubmit={onSubmit} className="flex w-full max-w-md flex-col gap-2 text-sm">
      <div className="flex gap-2">
        <input
          value={path}
          onChange={(event) => setPath(event.target.value)}
          placeholder={t('library.folderPlaceholder')}
          aria-label={t('library.folderPath')}
          className="bg-background placeholder:text-muted min-w-0 flex-1 rounded-lg px-3 py-2 outline-none"
        />
        <button
          type="submit"
          disabled={!path.trim() || addRoot.isPending}
          className="bg-background rounded-lg px-4 py-2 disabled:opacity-40"
        >
          {t('library.addFolder')}
        </button>
      </div>
      {addRoot.isError && (
        <p role="alert" className="text-primary">
          {addRoot.error.message}
        </p>
      )}
    </form>
  );
};
