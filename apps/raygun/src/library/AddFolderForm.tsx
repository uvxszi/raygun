import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { useAddRoot } from './useLibrary';

/** Adds a music folder by its path, in a sheet (from the Library's menu or the empty library). */
export const AddFolderSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const [path, setPath] = useState('');
  const addRoot = useAddRoot();

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (path.trim()) {
      addRoot.mutate(path.trim(), { onSuccess: onClose });
    }
  };

  return (
    <Sheet title={t('library.addFolder')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3 text-sm">
        <TextField
          autoFocus
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          value={path}
          onChange={setPath}
          placeholder={t('library.folderPlaceholder')}
          aria-label={t('library.folderPath')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        <p className="text-muted text-xs">{t('library.folderHint')}</p>
        {addRoot.isError && (
          <p role="alert" className="text-primary">
            {addRoot.error.message}
          </p>
        )}
        <button
          type="submit"
          disabled={!path.trim() || addRoot.isPending}
          className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          {t('library.addFolder')}
        </button>
      </form>
    </Sheet>
  );
};
