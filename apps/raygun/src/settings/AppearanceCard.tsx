import type {
  Accent,
  MotionLevel,
  NavLabels,
  ScrollbarStyle,
  StartScreen,
  Theme,
  WindowFrame,
} from '@raygun/bindings';
import { Check, Palette } from 'lucide-react';
import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefFont } from '../fonts/FontPicker';
import type { FontValue } from '../fonts/fontValue';
import { FONT_DEFAULTS, INTERFACE_STACK } from '../fonts/systemFonts';
import { LANGUAGES, SYSTEM_LANGUAGE, nativeName, resolveLanguage } from '../i18n/languages';
import { useLayout } from '../layout/useLayout';
import { NAV_TABS, isNavTabId } from '../shell/navTabs';
import { useIsDesktop } from '../shell/useIsDesktop';
import { useSideRail } from '../shell/useWideScreen';
import { ColorSettings } from './ColorSettings';
import { PrefBlock, PrefChoice, PrefGroup, PrefSize } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const THEMES: { value: Theme; labelKey: string; preview: [string, string, string] }[] = [
  { value: 'system', labelKey: 'settings.themeSystem', preview: ['#f6f7fb', '#16161f', '#e5e7ee'] },
  { value: 'light', labelKey: 'settings.themeLight', preview: ['#f6f7fb', '#f6f7fb', '#e5e7ee'] },
  { value: 'dark', labelKey: 'settings.themeDark', preview: ['#16161f', '#16161f', '#24242f'] },
  { value: 'amoled', labelKey: 'settings.themeAmoled', preview: ['#000000', '#000000', '#1a1a1f'] },
];

/** Swatches in the accent's dark-theme shade (the same as in styles.css). */
const ACCENTS: { value: Accent; color: string }[] = [
  { value: 'red', color: 'oklch(0.72 0.19 25)' },
  { value: 'orange', color: 'oklch(0.8 0.16 60)' },
  { value: 'green', color: 'oklch(0.8 0.17 150)' },
  { value: 'teal', color: 'oklch(0.8 0.12 190)' },
  { value: 'blue', color: 'oklch(0.74 0.15 255)' },
  { value: 'purple', color: 'oklch(0.74 0.16 300)' },
  { value: 'pink', color: 'oklch(0.77 0.17 355)' },
];

/**
 * Interface size (M46.48): percent of the size the screen gives (`ZOOM_LEVELS` in raygun-model),
 * shown as what it adds. Phones keep to sizes their screens fit.
 */
const INTERFACE_SIZE = { desktop: { min: 50, max: 200 }, phone: { min: 70, max: 150 } };

/** Text size: percent more or less than the screen's own text (`TEXT_SIZES` in raygun-model). */
const TEXT_SIZE = { min: -30, max: 100 };

const DEFAULT_FONT: FontValue = { kind: 'own', id: 'default' };

/** Settings → Appearance: theme and colours, language, sizes, how the app opens and moves. */
export const AppearanceCard: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const data = settings.data;
  const layout = useLayout();
  const desktop = useIsDesktop();
  // Navigation labels are the bottom bar's, which phones show.
  const sideRail = useSideRail();
  const colorTimer = useRef<number | undefined>(undefined);
  if (!data) return null;
  // The picker reports every move; the colour is saved once it rests.
  const pickColor = (color: string) => {
    window.clearTimeout(colorTimer.current);
    colorTimer.current = window.setTimeout(() => {
      updateSetting.mutate({ key: 'accentColor', value: color.toLowerCase() });
      if (data.accent !== 'custom') updateSetting.mutate({ key: 'accent', value: 'custom' });
    }, 250);
  };
  const customSelected = data.accent === 'custom';
  const fonts = data.fonts ?? FONT_DEFAULTS;

  return (
    <div data-testid="appearance-card" className="flex flex-col">
      <PrefGroup>
        <PrefBlock>
          <div className="flex flex-col gap-2">
            <span id="theme-label" className="font-medium">
              {t('settings.theme')}
            </span>
            <div role="radiogroup" aria-labelledby="theme-label" className="grid grid-cols-4 gap-3">
              {THEMES.map((option) => {
                const selected = data.theme === option.value;
                const [top, bottom, bar] = option.preview;
                return (
                  <button
                    key={option.value}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    aria-label={t(option.labelKey)}
                    onClick={() => updateSetting.mutate({ key: 'theme', value: option.value })}
                    className="flex flex-col items-center gap-2 text-sm"
                  >
                    <span
                      aria-hidden
                      className={`relative flex aspect-[3/5] w-full max-w-24 flex-col overflow-hidden rounded-xl border-2 ${selected ? 'border-primary' : 'border-surface'}`}
                      style={{ background: `linear-gradient(135deg, ${top} 50%, ${bottom} 50%)` }}
                    >
                      <span className="bg-primary mx-2 mt-3 h-2 w-1/2 rounded-full" />
                      <span className="mx-2 mt-2 h-8 rounded-md" style={{ background: bar }} />
                      <span className="mt-auto h-4" style={{ background: bar }} />
                      {selected && (
                        <span className="bg-primary text-background absolute top-1.5 right-1.5 grid size-5 place-items-center rounded-full">
                          <Check size={12} strokeWidth={3} />
                        </span>
                      )}
                    </span>
                    <span className={selected ? 'text-primary font-medium' : 'text-muted'}>
                      {t(option.labelKey)}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        </PrefBlock>
        <PrefBlock>
          <div className="flex flex-col gap-2">
            <span id="accent-label" className="font-medium">
              {t('settings.accent')}
            </span>
            <div
              role="radiogroup"
              aria-labelledby="accent-label"
              className="flex flex-wrap gap-2.5"
            >
              {ACCENTS.map((option) => {
                const selected = data.accent === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    aria-label={t(`settings.accents.${option.value}`)}
                    onClick={() => updateSetting.mutate({ key: 'accent', value: option.value })}
                    className={`grid size-10 place-items-center rounded-full ring-offset-2 ring-offset-[var(--color-background)] ${selected ? 'ring-foreground ring-2' : ''}`}
                    style={{ background: option.color }}
                  >
                    {selected && <Check aria-hidden size={20} strokeWidth={3} color="#111" />}
                  </button>
                );
              })}
              <label
                title={t('settings.accentCustom')}
                className={`relative grid size-10 cursor-pointer place-items-center rounded-full ring-offset-2 ring-offset-[var(--color-background)] ${customSelected ? 'ring-foreground ring-2' : ''}`}
                style={{
                  background: customSelected
                    ? data.accentColor
                    : 'conic-gradient(#f55, #fb4, #5d5, #4cd, #58f, #a6f, #f5a, #f55)',
                }}
              >
                {customSelected ? (
                  <Check aria-hidden size={20} strokeWidth={3} color="#111" />
                ) : (
                  <Palette aria-hidden size={18} color="#111" />
                )}
                <input
                  type="color"
                  aria-label={t('settings.accentCustom')}
                  value={data.accentColor ?? '#e5484d'}
                  onChange={(event) => pickColor(event.target.value)}
                  className="absolute inset-0 size-full cursor-pointer opacity-0"
                />
              </label>
            </div>
          </div>
        </PrefBlock>
        <PrefChoice<string>
          title={t('settings.language')}
          value={LANGUAGES.includes(data.language) ? data.language : SYSTEM_LANGUAGE}
          options={[
            {
              value: SYSTEM_LANGUAGE,
              label: t('settings.languageSystem', {
                name: nativeName(resolveLanguage(SYSTEM_LANGUAGE)),
              }),
            },
            ...LANGUAGES.map((tag) => ({ value: tag, label: nativeName(tag) })),
          ]}
          onChange={(value) => updateSetting.mutate({ key: 'language', value })}
        />
        <PrefSize
          title={t('settings.window.interfaceSize')}
          unit="%"
          zero={100}
          step={5}
          {...(desktop ? INTERFACE_SIZE.desktop : INTERFACE_SIZE.phone)}
          value={data.zoom ?? 100}
          onChange={(value) => updateSetting.mutate({ key: 'zoom', value })}
        />
        <PrefSize
          title={t('settings.window.textSize')}
          unit="%"
          step={5}
          {...TEXT_SIZE}
          value={data.textSize ?? 0}
          onChange={(value) => updateSetting.mutate({ key: 'textSize', value })}
        />
        <PrefFont
          title={t('settings.fonts.interface')}
          own={[{ id: 'default', label: t('settings.fonts.default'), stack: INTERFACE_STACK }]}
          value={fonts.interface ? { kind: 'family', family: fonts.interface } : DEFAULT_FONT}
          fallback={INTERFACE_STACK}
          onPick={(pick) =>
            updateSetting.mutate({
              key: 'fonts',
              value: { ...fonts, interface: pick.kind === 'family' ? pick.family : '' },
            })
          }
        />
        <PrefChoice<StartScreen>
          title={t('settings.startScreen')}
          value={data.startScreen}
          // Any tab the user shows (M46.5), and the current one even if it's hidden since.
          options={layout.tabs
            .filter(
              (item) =>
                (item.visible || item.id === data.startScreen) &&
                item.id !== 'more' &&
                item.id !== 'settings',
            )
            .map((item) => item.id)
            .filter(isNavTabId)
            .map((id) => ({ value: id as StartScreen, label: t(NAV_TABS[id].labelKey) }))}
          onChange={(value) => updateSetting.mutate({ key: 'startScreen', value })}
        />
        {!desktop && !sideRail && (
          <PrefChoice<NavLabels>
            title={t('settings.navLabels')}
            value={data.navLabels}
            options={[
              { value: 'always', label: t('settings.navLabelsAlways') },
              { value: 'selected', label: t('settings.navLabelsSelected') },
            ]}
            onChange={(value) => updateSetting.mutate({ key: 'navLabels', value })}
          />
        )}
        <PrefChoice<MotionLevel>
          title={t('settings.motion')}
          value={data.motion ?? 'lively'}
          options={(['lively', 'calm', 'off'] as const).map((value) => ({
            value,
            label: t(`settings.motionLevels.${value}`),
          }))}
          onChange={(value) => updateSetting.mutate({ key: 'motion', value })}
        />
        <PrefChoice<ScrollbarStyle>
          title={t('settings.scrollbars')}
          value={data.scrollbars ?? 'thin'}
          options={(['thin', 'wide', 'hidden', 'whileScrolling'] as const).map((value) => ({
            value,
            label: t(`settings.scrollbarStyles.${value}`),
          }))}
          onChange={(value) => updateSetting.mutate({ key: 'scrollbars', value })}
        />
        {desktop && (
          <PrefChoice<WindowFrame>
            title={t('settings.window.frame')}
            value={data.windowFrame ?? 'app'}
            options={(['app', 'system'] as const).map((value) => ({
              value,
              label: t(`settings.window.frames.${value}`),
            }))}
            onChange={(value) => updateSetting.mutate({ key: 'windowFrame', value })}
          />
        )}
      </PrefGroup>
      <ColorSettings />
    </div>
  );
};
