import { commands } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { forgetQueryCache } from '../shell/queryPersist';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { PrefAction, PrefGroup } from './prefs';
import { Problem } from './Problem';

/** What has to be typed, in any language: Rust checks the same word. */
export const ERASE_WORD = 'ERASE';

/** What the webview kept for this run goes too, so the next start is a first one. */
const forgetWebStorage = async () => {
  await forgetQueryCache();
  try {
    localStorage.clear();
    sessionStorage.clear();
  } catch {
    // Storage that can't be reached is cleared with the webview's profile at the next start.
  }
};

/**
 * Erase everything (M46.51): Raygun as after a new install, its backups kept. Asks with the
 * word ERASE typed out, then Raygun starts again into its first run.
 */
export const EraseCard: FC = () => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState('');
  const erase = useMutation({
    mutationFn: async () => {
      await forgetWebStorage();
      return unwrapResult(commands.dataEraseEverything(typed.trim()));
    },
  });
  const ready = typed.trim() === ERASE_WORD;
  const close = () => {
    if (erase.isPending || erase.isSuccess) return;
    setOpen(false);
    setTyped('');
  };

  return (
    <div data-testid="erase-card">
      <PrefGroup title={t('data.erase.group')}>
        <PrefAction
          icon={TriangleAlert}
          danger
          title={t('data.erase.title')}
          summary={t('data.erase.summary')}
          onClick={() => setOpen(true)}
        />
      </PrefGroup>
      {open && (
        <Sheet title={t('data.erase.ask')} onClose={close}>
          <form
            data-testid="erase-sheet"
            onSubmit={(event) => {
              event.preventDefault();
              if (ready && !erase.isPending) erase.mutate();
            }}
            className="flex flex-col gap-3 pb-1 text-sm"
          >
            <p>{t('data.erase.detail')}</p>
            <p className="text-muted">{t('data.erase.kept')}</p>
            <TextField
              value={typed}
              onChange={setTyped}
              placeholder={ERASE_WORD}
              aria-label={t('data.erase.type', { word: ERASE_WORD })}
              autoCapitalize="characters"
              autoCorrect="off"
              spellCheck={false}
              disabled={erase.isPending || erase.isSuccess}
              className={SHEET_FIELD}
              inputClassName={SHEET_INPUT}
            />
            <p className="text-muted text-xs">{t('data.erase.type', { word: ERASE_WORD })}</p>
            {erase.isSuccess && <p role="status">{t('data.erase.restarting')}</p>}
            {erase.error && <Problem title={t('data.erase.failed')} detail={erase.error.message} />}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={close}
                disabled={erase.isPending || erase.isSuccess}
                className="hover:bg-background rounded-full px-4 py-2 font-semibold"
              >
                {t('settings.cancel')}
              </button>
              <button
                type="submit"
                disabled={!ready || erase.isPending || erase.isSuccess}
                className="bg-primary text-background rounded-full px-4 py-2 font-semibold disabled:opacity-50"
              >
                {erase.isPending ? t('data.erase.erasing') : t('data.erase.confirm')}
              </button>
            </div>
          </form>
        </Sheet>
      )}
    </div>
  );
};
