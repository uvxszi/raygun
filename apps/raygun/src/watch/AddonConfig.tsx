import type { StremioAddon, StremioConfigField } from '@raygun/bindings';
import { Check, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { useAddConfiguredAddon } from './useStremio';

/** A ticked box's value, as the Stremio addon SDK's form sends it. */
const ON = 'on';

/**
 * What a setting starts as: its value in the addon's address (set up before), else its default
 * (`checked` ticks a box; a select falls back on its first choice).
 */
const startOf = (field: StremioConfigField) => {
  if (field.value !== null) return field.value;
  switch (field.kind) {
    case 'checkbox':
      return field.default === 'checked' ? ON : '';
    case 'select':
      return field.default || (field.options[0] ?? '');
    default:
      return field.default ?? '';
  }
};

/** One setting: a text, number or password field, a box to tick, or a choice. */
const ConfigInput: FC<{
  field: StremioConfigField;
  value: string;
  onChange: (value: string) => void;
}> = ({ field, value, onChange }) => {
  const { t } = useTranslation();
  if (field.kind === 'checkbox') {
    return (
      <label className="flex items-center gap-2 px-1">
        <input
          type="checkbox"
          checked={value === ON}
          onChange={(event) => onChange(event.target.checked ? ON : '')}
          className="accent-primary size-4"
        />
        {field.title}
      </label>
    );
  }
  if (field.kind === 'select') {
    return (
      <label className="flex flex-col gap-1">
        <span className="text-muted px-1 text-xs">{field.title}</span>
        <select
          aria-label={field.title}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="bg-background rounded-xl px-4 py-3 outline-none"
        >
          {field.options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
    );
  }
  return (
    <label className="flex flex-col gap-1">
      <span className="text-muted px-1 text-xs">
        {field.required ? t('watch.config.requiredTitle', { title: field.title }) : field.title}
      </span>
      <TextField
        type={field.kind}
        inputMode={field.kind === 'number' ? 'decimal' : undefined}
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        autoComplete="off"
        value={value}
        onChange={onChange}
        aria-label={field.title}
        aria-required={field.required}
        className={SHEET_FIELD}
        inputClassName={SHEET_INPUT}
      />
    </label>
  );
};

/**
 * Raygun's own form for an addon's settings, as its manifest declares them (the Stremio addon
 * SDK's `config`): only the settings it marks required must be filled in, and one whose title
 * says "(optional)" never must. Rust makes the address the SDK's own page would and adds the
 * addon there, in place of the same addon added before.
 */
export const AddonConfigForm: FC<{
  addon: StremioAddon;
  onDone?: (added: StremioAddon) => void;
}> = ({ addon, onDone }) => {
  const { t } = useTranslation();
  const fields = addon.config ?? [];
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(fields.map((field) => [field.key, startOf(field)])),
  );
  const save = useAddConfiguredAddon();
  const missing = fields.some((field) => field.required && (values[field.key] ?? '').trim() === '');
  const set = (key: string, value: string) =>
    setValues((current) => ({ ...current, [key]: value }));

  return (
    <form
      aria-label={t('watch.config.form', { name: addon.name })}
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        if (missing || save.isPending) return;
        save.mutate(
          {
            url: addon.url,
            values: fields.map((field) => ({ key: field.key, value: values[field.key] ?? '' })),
          },
          { onSuccess: (added) => onDone?.(added) },
        );
      }}
      className="flex flex-col gap-3 text-sm"
    >
      {fields.map((field) => (
        <ConfigInput
          key={field.key}
          field={field}
          value={values[field.key] ?? ''}
          onChange={(value) => set(field.key, value)}
        />
      ))}
      {save.error && <SourceIssue compact>{save.error.message}</SourceIssue>}
      <button
        type="submit"
        disabled={missing || save.isPending}
        className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
      >
        {save.isPending ? (
          <LoaderCircle aria-hidden size={16} className="animate-spin" />
        ) : (
          <Check aria-hidden size={16} />
        )}
        {t(addon.id ? 'watch.config.save' : 'watch.config.add')}
      </button>
    </form>
  );
};

/** An added addon's settings, set up again in Raygun's own form. */
export const AddonConfigSheet: FC<{
  addon: StremioAddon;
  anchor?: HTMLElement | null;
  onClose: () => void;
}> = ({ addon, anchor, onClose }) => {
  const { t } = useTranslation();
  return (
    <Sheet title={t('watch.config.title', { name: addon.name })} anchor={anchor} onClose={onClose}>
      <AddonConfigForm addon={addon} onDone={onClose} />
    </Sheet>
  );
};
