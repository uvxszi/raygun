import type { ExtensionSetting, SettingValue } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSetExtensionSetting } from './useExtensions';

type FieldProps = {
  setting: ExtensionSetting;
  onSave: (value: SettingValue) => void;
};

/** Text and numbers save when the field is left (or Enter is pressed), not on every keystroke. */
const TextField: FC<FieldProps> = ({ setting, onSave }) => {
  const current = setting.value === null ? '' : String(setting.value);
  const [draft, setDraft] = useState(current);
  const commit = () => {
    if (draft === current) return;
    if (setting.kind === 'number') {
      const number = Number(draft);
      if (draft.trim() !== '' && Number.isFinite(number)) onSave(number);
      else setDraft(current);
    } else {
      onSave(draft);
    }
  };
  return (
    <input
      id={`setting-${setting.key}`}
      type={setting.kind === 'number' ? 'number' : 'text'}
      inputMode={setting.kind === 'number' ? 'decimal' : 'text'}
      autoCapitalize="off"
      autoCorrect="off"
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur();
      }}
      className="bg-background w-full rounded-lg px-3 py-2 outline-none"
    />
  );
};

const Field: FC<FieldProps> = ({ setting, onSave }) => {
  switch (setting.kind) {
    case 'boolean': {
      const on = setting.value === true;
      return (
        <button
          id={`setting-${setting.key}`}
          type="button"
          role="switch"
          aria-checked={on}
          onClick={() => onSave(!on)}
          className={`relative h-7 w-12 shrink-0 rounded-full transition-colors ${on ? 'bg-primary' : 'bg-background'}`}
        >
          <span
            className={`bg-foreground absolute top-1 size-5 rounded-full transition-all ${on ? 'left-6' : 'left-1'}`}
          />
        </button>
      );
    }
    case 'select':
      return (
        <select
          id={`setting-${setting.key}`}
          value={typeof setting.value === 'string' ? setting.value : ''}
          onChange={(event) => onSave(event.target.value)}
          className="bg-background w-full rounded-lg px-3 py-2 outline-none"
        >
          {setting.options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      );
    default:
      // Re-mount when the stored value changes so the draft follows it.
      return <TextField key={String(setting.value)} setting={setting} onSave={onSave} />;
  }
};

export const SettingsForm: FC<{ extensionId: string; settings: ExtensionSetting[] }> = ({
  extensionId,
  settings,
}) => {
  const { t } = useTranslation();
  const save = useSetExtensionSetting();

  return (
    <section data-testid="extension-settings" className="flex flex-col gap-4 text-sm">
      <h2 className="text-primary px-1 pt-2 font-semibold">{t('extensions.settings')}</h2>
      {settings.map((setting) => (
        <div
          key={setting.key}
          className={
            setting.kind === 'boolean'
              ? 'flex items-center justify-between gap-4'
              : 'flex flex-col gap-1.5'
          }
        >
          <label htmlFor={`setting-${setting.key}`} className="font-medium">
            {setting.label}
          </label>
          <Field
            setting={setting}
            onSave={(value) => save.mutate({ extensionId, key: setting.key, value })}
          />
        </div>
      ))}
      {save.error && (
        <p role="alert" className="text-primary wrap-anywhere">
          {save.error.message}
        </p>
      )}
    </section>
  );
};
