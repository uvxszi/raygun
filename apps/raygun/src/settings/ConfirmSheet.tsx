import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';

/** Asks before something that can't be taken back (Reset to defaults, Unlink, Clear history). */
export const ConfirmSheet: FC<{
  /** The question: "Reset Library settings?" */
  title: string;
  /** What it does, only where the question doesn't say. */
  detail?: string;
  /** The button that does it: "Reset". */
  confirm: string;
  onConfirm: () => void;
  onClose: () => void;
}> = ({ title, detail, confirm, onConfirm, onClose }) => {
  const { t } = useTranslation();
  return (
    <Sheet title={title} onClose={onClose}>
      <div data-testid="confirm-sheet" className="flex flex-col gap-4 pb-1">
        {detail && <p className="text-muted text-sm">{detail}</p>}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="hover:bg-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('settings.cancel')}
          </button>
          <button
            type="button"
            onClick={() => {
              onConfirm();
              onClose();
            }}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {confirm}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
