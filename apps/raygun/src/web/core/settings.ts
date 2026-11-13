// Settings on the web: one synced `setting` row per key holding the value as JSON, as Rust keeps
// them (raygun-app settings.rs), over the defaults Rust writes beside the bindings.
import type { SearchFilterPrefs, SettingChange, Settings } from '@raygun/bindings';
import defaults from '@raygun/bindings/settings-defaults.json';

import { SyncedRows } from './context';
import type { CoreContext, SyncedRow } from './context';

export const SETTINGS_DEFAULTS = defaults as unknown as Settings;

/** Keys of removed settings, which rows synced from older versions may still hold. */
const RETIRED = new Set(['discord']);

export type SettingRow = SyncedRow & { key: string; value: string };

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/** `over` on top of `base`, field by field for objects, as serde's defaults fill missing fields. */
export const mergeOver = (base: unknown, over: unknown): unknown => {
  if (!isPlainObject(base) || !isPlainObject(over)) return over;
  const merged: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(over)) merged[key] = mergeOver(base[key], value);
  return merged;
};

const clone = <T>(value: T): T => structuredClone(value);

/** What a browser tab has instead of a window of its own: no frame to draw. */
const forTheWeb = (settings: Settings): Settings => ({ ...settings, windowFrame: 'system' });

export class SettingsService {
  private readonly rows: SyncedRows;

  constructor(context: CoreContext) {
    this.rows = new SyncedRows(context);
  }

  /** The settings as stored, defaults for anything never set (`settings::load`). */
  stored(): Settings {
    const settings = clone(SETTINGS_DEFAULTS) as unknown as Record<string, unknown>;
    for (const row of this.rows.live<SettingRow>('setting')) {
      if (RETIRED.has(row.key)) continue;
      try {
        settings[row.key] = mergeOver(settings[row.key], JSON.parse(row.value));
      } catch {
        // An unreadable row keeps the default, as Rust skips it.
      }
    }
    return settings as unknown as Settings;
  }

  /** The settings as the page shows them here. */
  load(): Settings {
    return forTheWeb(this.stored());
  }

  /** Changes several settings together (`settings::update_many`). */
  update(changes: SettingChange[]): Settings {
    const before = this.stored();
    const all = [...changes];
    const adultOn = all.some((change) => change.key === 'showNsfw' && change.value === true);
    const setsPill = all.some((change) => change.key === 'searchFilter');
    // Turning 18+ content on switches Search's "No 18+" pill on, unless the same changes set it.
    if (adultOn && !before.showNsfw && !setsPill) {
      all.push({
        key: 'searchFilter',
        value: { ...before.searchFilter, noAdult: true, adultOnly: false },
      });
    }
    const stamp = this.rows.stamp();
    for (const change of all) {
      let value: unknown = change.value;
      if (change.key === 'searchFilter') {
        // "No 18+" and "18+ only" turn each other off; asked for both, the pill wins.
        const prefs = value as SearchFilterPrefs;
        value = { ...prefs, adultOnly: prefs.adultOnly && !prefs.noAdult };
      }
      this.rows.write(
        'setting',
        { key: change.key, value: JSON.stringify(value ?? null), deleted: 0 },
        stamp,
      );
    }
    return this.load();
  }
}
