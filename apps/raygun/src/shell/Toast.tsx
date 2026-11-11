import { useEffect } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useToastStore } from './toastStore';

/** How long a toast stays, ms. */
const SHOWN_MS = 6_000;

/**
 * The toast host: one note at a time at the bottom, above the player bar and tabs and over open
 * menus and sheets (a menu's Copied), gone after a few seconds or with Undo.
 */
export const ToastHost: FC = () => {
  const { t } = useTranslation();
  const toast = useToastStore((state) => state.toast);
  const dismiss = useToastStore((state) => state.dismiss);

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => dismiss(toast.id), SHOWN_MS);
    return () => clearTimeout(timer);
  }, [toast, dismiss]);

  if (!toast) return null;
  return (
    <div
      // Clear of a cutout or a navigation bar at the sides, and of the gesture area with no bars
      // under it (the readers).
      className={`pointer-events-none fixed inset-x-0 z-[95] flex justify-center pr-[max(1rem,var(--safe-right))] pl-[max(1rem,var(--safe-left))] ${toast.bottom > 0 ? 'pb-3' : 'pb-[calc(0.75rem+var(--safe-bottom))]'}`}
      style={{ bottom: toast.bottom }}
    >
      <div
        role="status"
        data-testid="toast"
        className="bg-foreground text-background motion-rise pointer-events-auto flex max-w-md min-w-0 items-center gap-3 rounded-xl py-2 pr-2 pl-4 text-sm shadow-xl shadow-black/40"
      >
        <span className="min-w-0 flex-1 truncate">{toast.message}</span>
        {toast.undo && (
          <button
            type="button"
            onClick={() => {
              dismiss(toast.id);
              toast.undo?.();
            }}
            className="text-primary hover:bg-background/10 shrink-0 rounded-lg px-3 py-1.5 font-semibold"
          >
            {t('toast.undo')}
          </button>
        )}
      </div>
    </div>
  );
};
