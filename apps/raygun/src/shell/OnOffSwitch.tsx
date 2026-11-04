import { Check } from 'lucide-react';
import type { FC } from 'react';

/**
 * A small on/off switch for a list row (a source, an extension, an addon). It stops the click so
 * the row's own link or menu doesn't open.
 */
export const OnOffSwitch: FC<{
  checked: boolean;
  label: string;
  disabled?: boolean;
  onChange: (on: boolean) => void;
}> = ({ checked, label, disabled, onChange }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    disabled={disabled}
    onClick={(event) => {
      event.preventDefault();
      event.stopPropagation();
      onChange(!checked);
    }}
    className="grid size-11 shrink-0 place-items-center rounded-full disabled:opacity-60"
  >
    <span
      aria-hidden
      className={`relative h-6 w-10 rounded-full border-2 transition-colors ${checked ? 'bg-primary border-primary' : 'bg-surface border-muted/50'}`}
    >
      <span
        className={`absolute top-1/2 grid -translate-y-1/2 place-items-center rounded-full transition-[left,width,height] ${checked ? 'bg-background left-[1.05rem] size-4.5' : 'bg-muted left-1 size-3'}`}
      >
        {checked && <Check size={11} className="text-primary" strokeWidth={3} />}
      </span>
    </span>
  </button>
);
