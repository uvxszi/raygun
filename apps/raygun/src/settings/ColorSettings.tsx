import type { CustomColors } from '@raygun/bindings';
import { RotateCcw } from 'lucide-react';
import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefGroup } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const COLOR_KEYS = ['background', 'surface', 'foreground', 'muted'] as const;
type ColorKey = (typeof COLOR_KEYS)[number];

const NO_COLORS: CustomColors = { background: '', surface: '', foreground: '', muted: '' };

/** The theme's own colour behind a row's swatch while no colour is picked. */
const themeColor = (key: ColorKey) =>
  typeof getComputedStyle === 'function'
    ? getComputedStyle(document.documentElement).getPropertyValue(`--color-${key}`).trim()
    : '';

/**
 * Settings → Appearance → Colours (M46.8): the page, panels, text and secondary text in any
 * colour over the theme's, each with a way back to the theme's.
 */
export const ColorSettings: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const timer = useRef<number | undefined>(undefined);
  const colors: CustomColors = { ...NO_COLORS, ...settings.data?.colors };
  const save = (patch: Partial<CustomColors>) =>
    update.mutate({ key: 'colors', value: { ...colors, ...patch } });
  // The picker reports every move; the colour is saved once it rests.
  const pick = (key: ColorKey, value: string) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => save({ [key]: value.toLowerCase() }), 250);
  };

  return (
    <PrefGroup title={t('settings.colors.title')}>
      {COLOR_KEYS.map((key) => (
        <li
          key={key}
          data-pref={t(`settings.colors.${key}`)}
          className="flex items-center gap-3 px-1 py-2.5"
        >
          <label className="relative flex min-w-0 flex-1 cursor-pointer items-center gap-3">
            <span
              aria-hidden
              style={{ background: colors[key] || themeColor(key) || undefined }}
              className="size-8 shrink-0 rounded-full border border-white/15"
            />
            <span className="flex min-w-0 flex-col">
              <span>{t(`settings.colors.${key}`)}</span>
              <span className="text-muted text-xs">
                {colors[key] || t('settings.colors.fromTheme')}
              </span>
            </span>
            <input
              type="color"
              aria-label={t(`settings.colors.${key}`)}
              value={colors[key] || '#000000'}
              onChange={(event) => pick(key, event.target.value)}
              className="absolute inset-0 size-full cursor-pointer opacity-0"
            />
          </label>
          {colors[key] && (
            <button
              type="button"
              aria-label={t('settings.colors.reset', { name: t(`settings.colors.${key}`) })}
              onClick={() => save({ [key]: '' })}
              className="text-muted hover:text-foreground grid size-9 place-items-center rounded-full"
            >
              <RotateCcw aria-hidden size={16} />
            </button>
          )}
        </li>
      ))}
    </PrefGroup>
  );
};
