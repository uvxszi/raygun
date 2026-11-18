import { useMutation } from '@tanstack/react-query';
import { CheckCheck, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from '../shell/toastStore';
import { useDragScroll } from '../shell/useDragScroll';

/** Something the picked items of a Home row can have done to them. */
export type PickAction = {
  label: string;
  icon: LucideIcon;
  /** Asked first, in the bar (taking things out of the library). */
  confirm?: string;
  /** Does it; what it did, said in a toast. */
  run: () => Promise<string | null>;
};

/**
 * The bar of a Home row's picked items (Continue reading, Continue listening, Recently played):
 * how many, Select all, and what they share, each a labelled button; picking ends once one is
 * done. An action that asks first asks in the bar.
 */
export const ContinuePickBar: FC<{
  /** The toolbar's name: "Continue reading actions". */
  label: string;
  count: number;
  allPicked: boolean;
  onSelectAll: () => void;
  onClose: () => void;
  actions: PickAction[];
}> = ({ label, count, allPicked, onSelectAll, onClose, actions }) => {
  const { t } = useTranslation();
  const scroller = useDragScroll<HTMLDivElement>();
  const [asking, setAsking] = useState<PickAction | null>(null);
  const run = useMutation({
    mutationFn: (action: PickAction) => action.run(),
    onSuccess: (message) => {
      if (message) showToast(message);
      setAsking(null);
      onClose();
    },
    onError: (error) => showToast(error.message),
  });

  const button = (text: string, Icon: LucideIcon, onClick: () => void) => (
    <button
      key={text}
      type="button"
      disabled={run.isPending}
      onClick={onClick}
      className="hover:bg-background flex shrink-0 items-center gap-2 rounded-full px-3 py-2 text-sm font-medium disabled:opacity-50"
    >
      <Icon aria-hidden size={16} />
      {text}
    </button>
  );

  return (
    <div
      data-testid="continue-pick-bar"
      className="bg-surface motion-rise flex flex-col gap-1 rounded-2xl p-1 shadow-xl"
    >
      {asking ? (
        <div
          role="alertdialog"
          aria-label={asking.confirm}
          className="flex flex-wrap items-center justify-end gap-2 px-2 py-1"
        >
          <p className="min-w-0 flex-1 text-sm">{asking.confirm}</p>
          <button
            type="button"
            onClick={() => setAsking(null)}
            className="hover:bg-background rounded-full px-3 py-1.5 text-sm"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            autoFocus
            disabled={run.isPending}
            onClick={() => run.mutate(asking)}
            className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
          >
            {asking.label}
          </button>
        </div>
      ) : (
        <div role="toolbar" aria-label={label} className="flex items-center gap-1">
          <button
            type="button"
            aria-label={t('watch.endSelection')}
            title={t('watch.endSelection')}
            onClick={onClose}
            className="hover:bg-background grid size-10 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={20} />
          </button>
          <span aria-live="polite" className="shrink-0 px-1 text-sm font-semibold">
            {t('watch.selectedCount', { count })}
          </span>
          <div ref={scroller} className="flex min-w-0 flex-1 no-scrollbar overflow-x-auto">
            {!allPicked && button(t('watch.selectAllShown'), CheckCheck, onSelectAll)}
            {actions.map((action) =>
              button(action.label, action.icon, () =>
                action.confirm ? setAsking(action) : run.mutate(action),
              ),
            )}
          </div>
        </div>
      )}
    </div>
  );
};
