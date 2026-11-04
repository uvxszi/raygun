import type { SettingChange, Settings } from '@raygun/bindings';

/** What a screen's Reset to defaults sends: its settings as a fresh install has them. */
export type SettingsReset = (current: Settings, defaults: Settings) => SettingChange[];

/** The changes that put `keys` back as `defaults` has them. */
export const resetKeys =
  (...keys: (keyof Settings)[]): SettingsReset =>
  (_current, defaults) =>
    keys.map((key) => ({ key, value: defaults[key] }) as SettingChange);

/** Several resets as one. */
export const resetAll =
  (...resets: SettingsReset[]): SettingsReset =>
  (current, defaults) =>
    resets.flatMap((reset) => reset(current, defaults));
