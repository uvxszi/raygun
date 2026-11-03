import { Link } from '@tanstack/react-router';
import { Check, ChevronRight, RotateCcw } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useEffectEvent, useId, useRef, useState } from 'react';
import type { ChangeEvent, FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { parseSize, shownSize, signedSize } from './sizes';
import type { SizeLimits } from './sizes';

/**
 * Preference rows (Mihon's): a short title, the chosen value or a second line only where it helps
 * to decide, and the control. Rows sit in groups with a small label. Each row carries its title in
 * `data-pref`, which settings search scrolls to.
 */
export const PrefGroup: FC<{ title?: string; children: ReactNode }> = ({ title, children }) => (
  <section aria-label={title} className="flex flex-col pt-3 first:pt-0">
    {title && <h2 className="text-muted px-1 pt-1 pb-0.5 text-xs font-semibold">{title}</h2>}
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
    <li data-pref={title}>
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
  /** Shown under the current choice, only where the choice needs it. */
  summary?: string;
  value: T;
  options: PrefOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) => {
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const open = anchor !== null;
  const close = () => setAnchor(null);
  const current = options.find((option) => Object.is(option.value, value));
  return (
    <li data-pref={title}>
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
        <Sheet title={title} anchor={anchor} onClose={close}>
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
                    close();
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

/** How long a let-go slider shows its own value while the saved one hasn't come back. */
const DRAFT_MS = 3000;

/**
 * A range input that saves once the thumb is let go (pointer, touch or keys: the browser's own
 * `change`), so dragging doesn't write every step. What it shows meanwhile stays until the saved
 * value comes back (or a while, when saving failed), so the thumb doesn't jump back.
 */
const useSlider = (value: number, onChange: (value: number) => void) => {
  const input = useRef<HTMLInputElement>(null);
  // The value the thumb is at, and the saved value it was moved from.
  const [draft, setDraft] = useState<{ at: number; from: number } | null>(null);
  const shown = draft && draft.from === value ? draft.at : value;
  const commit = useEffectEvent((element: HTMLInputElement) => {
    const next = Number(element.value);
    if (next !== value) onChange(next);
    window.setTimeout(() => setDraft(null), DRAFT_MS);
  });
  useEffect(() => {
    const element = input.current;
    if (!element) return;
    const onCommit = () => commit(element);
    element.addEventListener('change', onCommit);
    return () => element.removeEventListener('change', onCommit);
  }, []);

  return {
    shown,
    props: {
      ref: input,
      type: 'range' as const,
      value: shown,
      onChange: (event: ChangeEvent<HTMLInputElement>) =>
        setDraft({ at: Number(event.target.value), from: value }),
      className: 'accent-primary w-full',
    },
  };
};

/** A number on a slider, its value always shown beside the title. */
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
  const slider = useSlider(value, onChange);
  return (
    <li data-pref={title} className="flex flex-col gap-2 px-1 py-3">
      <div className="flex items-baseline gap-3">
        <RowText title={title} summary={summary} titleId={titleId} />
        <span className="text-primary shrink-0 font-semibold tabular-nums">
          {format(slider.shown)}
        </span>
      </div>
      <input
        {...slider.props}
        aria-labelledby={titleId}
        aria-valuetext={format(slider.shown)}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
      />
    </li>
  );
};

export type SizeFieldProps = SizeLimits & {
  title: string;
  /** A second line, only where it helps to decide. */
  summary?: string;
  /** The stored value; limits and step are in stored units too. */
  value: number;
  /** Shown after the number: "%", "px". */
  unit: string;
  onChange: (value: number) => void;
  disabled?: boolean;
};

/**
 * A size (M46.48): the screen gives every size, and this adds to it or takes from it. A slider
 * and a number to type (kept on the step and within the limits), its unit, and Reset back to the
 * screen's own size, shown as 0. `PrefSize` is the settings row; this is the control itself, for
 * panels outside settings lists.
 */
export const SizeField: FC<SizeFieldProps> = ({
  title,
  summary,
  value,
  unit,
  onChange,
  disabled,
  ...limits
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  const slider = useSlider(value, onChange);
  const { min, max, step, zero = 0, factor = 1 } = limits;
  const shown = (stored: number) => shownSize(stored, limits);
  // The field is the browser's while it's being typed in (its key holds still); Enter or leaving
  // it saves.
  const [typingKey, setTypingKey] = useState<number | null>(null);
  const save = (field: HTMLInputElement) => {
    setTypingKey(null);
    const next = parseSize(field.value, limits) ?? value;
    // What was typed shows as it's kept: on the step, within the limits.
    field.value = String(shown(next));
    if (next !== value) onChange(next);
  };

  return (
    <div data-testid="size-field" className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span id={titleId} className="font-medium">
            {title}
          </span>
          {summary && <span className="text-muted text-sm">{summary}</span>}
        </span>
        <label className="bg-surface focus-within:ring-primary flex shrink-0 items-center gap-1 rounded-lg px-2 py-1 focus-within:ring-2">
          <input
            // Remade with each new value, unless it's being typed in.
            key={typingKey ?? slider.shown}
            type="number"
            aria-label={t('settings.sizeValue', { name: title })}
            defaultValue={shown(slider.shown)}
            min={shown(min)}
            max={shown(max)}
            step={Number((step * factor).toPrecision(6))}
            disabled={disabled}
            onFocus={(event) => {
              setTypingKey(slider.shown);
              event.currentTarget.select();
            }}
            onBlur={(event) => save(event.currentTarget)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') event.currentTarget.blur();
              if (event.key === 'Escape') {
                event.currentTarget.value = String(shown(value));
                event.currentTarget.blur();
              }
            }}
            className="w-14 [appearance:textfield] bg-transparent text-right font-semibold tabular-nums outline-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          />
          {unit && <span className="text-muted text-sm">{unit}</span>}
        </label>
        <button
          type="button"
          aria-label={t('settings.sizeReset', { name: title })}
          title={t('settings.sizeReset', { name: title })}
          disabled={disabled || value === zero}
          onClick={() => onChange(zero)}
          className="text-muted hover:text-foreground hover:bg-surface grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-30"
        >
          <RotateCcw aria-hidden size={16} />
        </button>
      </div>
      <input
        {...slider.props}
        aria-labelledby={titleId}
        aria-valuetext={`${signedSize(slider.shown, limits)} ${unit}`.trim()}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
      />
    </div>
  );
};

/** A size as a settings row (see {@link SizeField}). */
export const PrefSize: FC<SizeFieldProps> = (props) => (
  <li data-pref={props.title} className="px-1 py-3">
    <SizeField {...props} />
  </li>
);

/** A row that opens another screen. */
export const PrefLink: FC<{
  title: string;
  summary?: ReactNode;
  to: string;
  search?: Record<string, unknown>;
  hash?: string;
  icon?: LucideIcon;
}> = ({ title, summary, to, search, hash, icon: Icon }) => (
  <li data-pref={title}>
    <Link to={to} search={search} hash={hash} className={`${ROW} hover:bg-surface`}>
      {Icon && <Icon aria-hidden size={22} className="text-primary shrink-0" />}
      <RowText title={title} summary={summary} />
      <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
    </Link>
  </li>
);

/** A row that does something (Create backup, Clear history); `danger` for what removes data. */
export const PrefAction: FC<{
  title: string;
  summary?: ReactNode;
  icon?: LucideIcon;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}> = ({ title, summary, icon: Icon, onClick, disabled, danger }) => (
  <li data-pref={title}>
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`${ROW} hover:bg-surface disabled:opacity-60`}
    >
      {Icon && (
        <Icon
          aria-hidden
          size={22}
          className={`shrink-0 ${danger ? 'text-primary' : 'text-muted'}`}
        />
      )}
      <RowText title={title} summary={summary} />
    </button>
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
    <li data-pref={title}>
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
            <span className={`block truncate ${value ? 'text-foreground/80' : ''}`}>
              {value || placeholder}
            </span>
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
            <div className="flex justify-end gap-2">
              {draft !== '' && placeholder && (
                <button
                  type="button"
                  onClick={() => setDraft('')}
                  className="hover:bg-background rounded-full px-4 py-2.5"
                >
                  {t('settings.useDefault')}
                </button>
              )}
              <button
                type="submit"
                disabled={problem !== null}
                className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
              >
                {t('settings.save')}
              </button>
            </div>
          </form>
        </Sheet>
      )}
    </li>
  );
};
