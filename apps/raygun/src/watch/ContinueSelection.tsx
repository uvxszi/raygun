import { Check, CheckCheck, Trash2, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { showToast } from '../shell/toastStore';
import type { ContinueSelection } from './useContinueSelection';
import { useRemoveContinueMany } from './useWatch';

/**
 * An entry's pick circle, in its top left corner: it shows on hover or focus (to start picking
 * with a mouse or keys) and always while picking. Shift-click picks a range.
 */
export const PickCircle: FC<{
  checked: boolean;
  /** Picking is on: the circle always shows. */
  shown: boolean;
  name: string;
  onPick: (range: boolean) => void;
  className?: string;
}> = ({ checked, shown, name, onPick, className = 'absolute top-1.5 left-1.5' }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={t('watch.continueSelect.pick', { title: name })}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onPick(event.shiftKey);
      }}
      className={`z-10 grid size-6 shrink-0 place-items-center rounded-full border-2 shadow transition-opacity ${
        checked
          ? 'border-primary bg-primary text-background'
          : 'border-white/90 bg-black/40 text-transparent'
      } ${
        shown || checked
          ? 'opacity-100'
          : 'pointer-events-none opacity-0 group-hover/pick:pointer-events-auto group-hover/pick:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100'
      } ${className}`}
    >
      <Check aria-hidden size={14} strokeWidth={3} />
    </button>
  );
};

/**
 * What can be done to the picked entries: pick all (or none), or remove them, asked first with
 * how many. `history` words it for the watch history, which removing empties too.
 */
export const ContinueSelectionBar: FC<{
  selection: ContinueSelection;
  history?: boolean;
  className?: string;
}> = ({ selection, history = false, className = '' }) => {
  const { t } = useTranslation();
  const remove = useRemoveContinueMany();
  const [asking, setAsking] = useState(false);
  const count = selection.selected.length;
  const question = t(
    history ? 'watch.continueSelect.removeHistoryAsk' : 'watch.continueSelect.removeAsk',
    { count },
  );
  const confirm = () =>
    remove.mutate(selection.selected, {
      onSuccess: (removed) => {
        showToast(
          t(history ? 'watch.continueSelect.removedHistory' : 'watch.continueSelect.removed', {
            count: removed,
          }),
        );
        setAsking(false);
        selection.clear();
      },
    });

  const action = (label: string, Icon: LucideIcon, onClick: () => void, wide = false) => (
    <button
      type="button"
      aria-label={wide ? undefined : label}
      title={label}
      onClick={onClick}
      className={`hover:bg-background flex shrink-0 items-center gap-2 rounded-full text-sm font-medium ${
        wide ? 'px-3 py-2' : 'size-10 justify-center'
      }`}
    >
      <Icon aria-hidden size={wide ? 16 : 20} />
      {wide && label}
    </button>
  );

  return (
    <div
      data-testid="continue-selection-bar"
      className={`bg-surface flex flex-col gap-1 rounded-2xl p-1 shadow-xl ${className}`}
    >
      {asking ? (
        <div
          role="alertdialog"
          aria-label={question}
          className="flex flex-wrap items-center justify-end gap-2 px-2 py-1"
        >
          <p className="min-w-0 flex-1 text-sm">{question}</p>
          <button
            type="button"
            onClick={() => setAsking(false)}
            className="hover:bg-background rounded-full px-3 py-1.5 text-sm"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            autoFocus
            disabled={remove.isPending}
            onClick={confirm}
            className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
          >
            {t('watch.continueSelect.remove')}
          </button>
        </div>
      ) : (
        <div
          role="toolbar"
          aria-label={t('watch.continueSelect.actions')}
          className="flex items-center gap-1"
        >
          {action(t('watch.endSelection'), X, selection.clear)}
          <span aria-live="polite" className="min-w-0 flex-1 truncate px-1 text-sm font-semibold">
            {t('watch.selectedCount', { count })}
          </span>
          {action(
            t(selection.allSelected ? 'watch.selectNone' : 'watch.selectAllShown'),
            CheckCheck,
            selection.toggleAll,
            true,
          )}
          {action(t('watch.continueSelect.remove'), Trash2, () => setAsking(true), true)}
        </div>
      )}
      {remove.error && <SourceIssue compact>{remove.error.message}</SourceIssue>}
    </div>
  );
};
