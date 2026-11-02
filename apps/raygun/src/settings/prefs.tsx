import { Link } from '@tanstack/react-router';
import { Check, ChevronRight } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useId, useState } from 'react';
import type { FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';

/**
 * Preference rows (Mihon's): a title, a line saying what it does or what's chosen, and the
 * control. Rows sit in titled groups on a settings screen.
 */
export const PrefGroup: FC<{ title?: string; children: ReactNode }> = ({ title, children }) => (
  <section aria-label={title} className="flex flex-col">
    {title && <h2 className="text-primary px-1 pt-4 pb-1 text-sm font-semibold">{title}</h2>}
    <ul className="flex flex-col">{children}</ul>
  </section>
);

const ROW = 'flex w-full items-center gap-4 rounded-xl px-1 py-3 text-left';

const RowText: FC<{ title: string; summary?: ReactNode; titleId?: string }> = ({
  title,
  summary,
  titleId,
}) => (
  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
    <span id={titleId} className="font-medium">
      {title}
    </span>
    {summary && <span className="text-muted text-sm">{summary}</span>}
  </span>
);

/** An on/off preference; the whole row toggles it. */
export const PrefSwitch: FC<{
  title: string;
  summary?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}> = ({ title, summary, checked, onChange, disabled }) => {
  const titleId = useId();
  return (
    <li>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={titleId}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`${ROW} hover:bg-surface disabled:opacity-60`}
      >
        <RowText title={title} summary={summary} titleId={titleId} />
        <span
          aria-hidden
          className={`relative h-8 w-13 shrink-0 rounded-full border-2 transition-colors ${checked ? 'bg-primary border-primary' : 'bg-surface border-muted/50'}`}
        >
          <span
            className={`absolute top-1/2 grid -translate-y-1/2 place-items-center rounded-full transition-all ${checked ? 'bg-background left-[1.375rem] size-6' : 'bg-muted left-1.5 size-4'}`}
          >
            {checked && <Check size={14} className="text-primary" strokeWidth={3} />}
          </span>
        </span>
      </button>
    </li>
  );
};

export type PrefOption<T> = { value: T; label: string; hint?: string };

/** One choice among a few; the row shows the current one and opens a sheet to pick. */
export const PrefChoice = <T,>({
  title,
  summary,
  value,
  options,
  onChange,
  disabled,
}: {
  title: string;
  /** Shown under the current choice, e.g. what the setting changes. */
  summary?: string;
  value: T;
  options: PrefOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) => {
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const open = anchor !== null;
  const setOpen = (shown: boolean) => {
    if (!shown) setAnchor(null);
  };
  const current = options.find((option) => Object.is(option.value, value));
  return (
    <li>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={disabled}
        onClick={(event) => setAnchor(open ? null : event.currentTarget)}
        className={`${ROW} hover:bg-surface disabled:opacity-60`}
      >
        <RowText
          title={title}
          summary={
            <>
              <span className="text-foreground/80">{current?.label ?? ''}</span>
              {summary && <span className="block">{summary}</span>}
            </>
          }
        />
      </button>
      {open && (
        <Sheet title={title} anchor={anchor} onClose={() => setOpen(false)}>
          <div role="radiogroup" aria-label={title} className="-mx-2 flex flex-col">
            {options.map((option) => {
              const selected = Object.is(option.value, value);
              return (
                <button
                  key={String(option.value)}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  onClick={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                  className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left"
                >
                  <span
                    aria-hidden
                    className={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${selected ? 'border-primary' : 'border-muted'}`}
                  >
                    {selected && <span className="bg-primary size-2.5 rounded-full" />}
                  </span>
                  <span className="flex min-w-0 flex-col">
                    <span>{option.label}</span>
                    {option.hint && <span className="text-muted text-xs">{option.hint}</span>}
                  </span>
                </button>
              );
            })}
          </div>
        </Sheet>
      )}
    </li>
  );
};

/** A number on a slider, its value shown beside the title. */
export const PrefSlider: FC<{
  title: string;
  summary?: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (value: number) => string;
  onChange: (value: number) => void;
  disabled?: boolean;
}> = ({ title, summary, value, min, max, step, format, onChange, disabled }) => {
  const titleId = useId();
  // Moves with the thumb; saved when let go, so dragging doesn't write every step.
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft !== null && draft !== value) onChange(draft);
    setDraft(null);
  };
  return (
    <li className="flex flex-col gap-2 px-1 py-3">
      <div className="flex items-baseline gap-3">
        <RowText title={title} summary={summary} titleId={titleId} />
        <span className="text-primary shrink-0 font-semibold tabular-nums">{format(shown)}</span>
      </div>
      <input
        type="range"
        aria-labelledby={titleId}
        aria-valuetext={format(shown)}
        min={min}
        max={max}
        step={step}
        value={shown}
        disabled={disabled}
        onChange={(event) => setDraft(Number(event.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className="accent-primary w-full"
      />
    </li>
  );
};

/** A row that opens another screen. */
export const PrefLink: FC<{
  title: string;
  summary?: ReactNode;
  to: string;
  search?: Record<string, unknown>;
  icon?: LucideIcon;
}> = ({ title, summary, to, search, icon: Icon }) => (
  <li>
    <Link to={to} search={search} className={`${ROW} hover:bg-surface`}>
      {Icon && <Icon aria-hidden size={22} className="text-primary shrink-0" />}
      <RowText title={title} summary={summary} />
      <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
    </Link>
  </li>
);

/** Free content inside a group (a card that has more than one control). */
export const PrefBlock: FC<{ children: ReactNode }> = ({ children }) => (
  <li className="px-1 py-2">{children}</li>
);

/** Free text, edited in a sheet. `check` returns why a text can't be saved, or null. */
export const PrefText: FC<{
  title: string;
  summary?: string;
  value: string;
  placeholder?: string;
  check?: (text: string) => string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
}> = ({ title, summary, value, placeholder, check, onChange, disabled }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const problem = draft === null ? null : (check?.(draft.trim()) ?? null);
  const save = (event: FormEvent) => {
    event.preventDefault();
    if (draft === null || problem) return;
    onChange(draft.trim());
    setDraft(null);
  };
  return (
    <li>
      <button
        type="button"
        aria-haspopup="dialog"
        disabled={disabled}
        onClick={() => setDraft(value)}
        className={`${ROW} hover:bg-surface disabled:opacity-60`}
      >
        <RowText
          title={title}
          summary={
            <>
              <span className={value ? 'text-foreground/80' : ''}>{value || placeholder}</span>
              {summary && <span className="block">{summary}</span>}
            </>
          }
        />
      </button>
      {draft !== null && (
        <Sheet title={title} onClose={() => setDraft(null)}>
          <form onSubmit={save} className="flex flex-col gap-3 text-sm">
            {summary && <p className="text-muted">{summary}</p>}
            <input
              autoFocus
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder={placeholder}
              aria-label={title}
              aria-invalid={problem !== null}
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
            />
            {problem && (
              <p role="alert" className="text-primary">
                {problem}
              </p>
            )}
            <button
              type="submit"
              disabled={problem !== null}
              className="bg-primary text-background self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
            >
              {t('settings.save')}
            </button>
          </form>
        </Sheet>
      )}
    </li>
  );
};
