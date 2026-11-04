import { CheckSquare, Power, PowerOff, Square, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { BulkSelection } from './useBulkSelection';

/** The Select button that starts picking rows. */
export const SelectButton: FC<{ selection: BulkSelection }> = ({ selection }) => {
  const { t } = useTranslation();
  if (selection.selecting) return null;
  return (
    <button
      type="button"
      onClick={selection.start}
      className="bg-surface flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
    >
      <CheckSquare aria-hidden size={16} />
      {t('bulk.select')}
    </button>
  );
};

/** A row's checkbox while picking. */
export const SelectCheck: FC<{ selection: BulkSelection; id: string; name: string }> = ({
  selection,
  id,
  name,
}) => {
  const { t } = useTranslation();
  if (!selection.selecting) return null;
  const checked = selection.isSelected(id);
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={t('bulk.pick', { name })}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        selection.toggle(id);
      }}
      className={`grid size-11 shrink-0 place-items-center ${checked ? 'text-primary' : 'text-muted'}`}
    >
      {checked ? <CheckSquare aria-hidden size={22} /> : <Square aria-hidden size={22} />}
    </button>
  );
};

/** While picking: Select all, how many are picked, Turn on, Turn off and Done. */
export const BulkSwitchBar: FC<{
  selection: BulkSelection;
  busy?: boolean;
  onSwitch: (ids: string[], on: boolean) => void;
}> = ({ selection, busy, onSwitch }) => {
  const { t } = useTranslation();
  if (!selection.selecting) return null;
  const none = selection.selected.length === 0;
  const action =
    'bg-surface flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm disabled:opacity-50';
  return (
    <div
      role="toolbar"
      aria-label={t('bulk.bar')}
      className="bg-background/95 sticky top-0 z-10 flex flex-wrap items-center gap-2 py-2"
    >
      <button
        type="button"
        role="checkbox"
        aria-checked={selection.allSelected}
        onClick={selection.toggleAll}
        className={action}
      >
        {selection.allSelected ? (
          <CheckSquare aria-hidden size={16} className="text-primary" />
        ) : (
          <Square aria-hidden size={16} />
        )}
        {t('bulk.selectAll')}
      </button>
      <span className="text-muted text-sm">
        {t('bulk.count', { count: selection.selected.length })}
      </span>
      <span className="flex-1" />
      <button
        type="button"
        disabled={none || busy}
        onClick={() => onSwitch(selection.selected, true)}
        className={action}
      >
        <Power aria-hidden size={16} />
        {t('bulk.turnOn')}
      </button>
      <button
        type="button"
        disabled={none || busy}
        onClick={() => onSwitch(selection.selected, false)}
        className={action}
      >
        <PowerOff aria-hidden size={16} />
        {t('bulk.turnOff')}
      </button>
      <button
        type="button"
        aria-label={t('bulk.done')}
        onClick={selection.stop}
        className="grid size-9 place-items-center rounded-full"
      >
        <X aria-hidden size={18} />
      </button>
    </div>
  );
};
