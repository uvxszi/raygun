import { ArrowDown, ArrowUp } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { RowMove } from './tableView';
import { useHoldPress } from './useHoldPress';

const MoveButton: FC<{
  label: string;
  hint: string;
  Icon: LucideIcon;
  disabled: boolean;
  onTap: () => void;
  onHold: () => void;
}> = ({ label, hint, Icon, disabled, onTap, onHold }) => {
  const { holding, props } = useHoldPress({ onTap, onHold, disabled });
  return (
    <button
      type="button"
      aria-label={label}
      title={hint}
      disabled={disabled}
      {...props}
      onKeyDown={(event) => {
        props.onKeyDown(event);
        // Home and End do from the keyboard what a hold does.
        if (event.key !== 'Home' && event.key !== 'End') return;
        event.preventDefault();
        event.stopPropagation();
        if (!disabled) onHold();
      }}
      data-holding={holding || undefined}
      className="text-muted hover:text-foreground relative grid size-9 shrink-0 place-items-center overflow-hidden rounded-full select-none disabled:opacity-30"
    >
      {/* Fills while the press counts towards a hold. */}
      <span
        aria-hidden
        className={`bg-primary/25 absolute inset-0 origin-bottom rounded-full transition-transform ease-linear ${
          holding ? 'scale-y-100 duration-500' : 'scale-y-0 duration-0'
        }`}
      />
      <Icon aria-hidden size={18} className="relative" />
    </button>
  );
};

/**
 * A row's up and down buttons in edit mode: a tap moves the song one place, holding moves it
 * all the way to the top or the bottom (Home and End from the keyboard).
 */
export const RowMoveButtons: FC<{
  title: string;
  first: boolean;
  last: boolean;
  onMove: (move: RowMove) => void;
}> = ({ title, first, last, onMove }) => {
  const { t } = useTranslation();
  return (
    <span className="flex shrink-0">
      <MoveButton
        label={t('collections.moveUp', { title })}
        hint={t('tracks.holdToTop')}
        Icon={ArrowUp}
        disabled={first}
        onTap={() => onMove('up')}
        onHold={() => onMove('top')}
      />
      <MoveButton
        label={t('collections.moveDown', { title })}
        hint={t('tracks.holdToBottom')}
        Icon={ArrowDown}
        disabled={last}
        onTap={() => onMove('down')}
        onHold={() => onMove('bottom')}
      />
    </span>
  );
};
