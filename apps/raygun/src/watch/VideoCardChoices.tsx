import type { VideoCardPrefs, VideoCardStyle, VideoPreviews } from '@raygun/bindings';
import { LayoutGrid } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';

const VIDEO_CARD_STYLES: readonly VideoCardStyle[] = ['landscape', 'poster', 'list'];
const VIDEO_PREVIEWS: readonly VideoPreviews[] = ['auto', 'always', 'off'];

const DEFAULTS: Required<VideoCardPrefs> = { style: 'landscape', previews: 'auto' };

/** `Settings::videoCards` with every field, and a way to change part of it. */
const useVideoCardPrefs = () => {
  const stored = useSettings().data?.videoCards;
  const update = useUpdateSetting();
  const prefs: Required<VideoCardPrefs> = { ...DEFAULTS, ...stored };
  const set = (change: Partial<VideoCardPrefs>) =>
    update.mutate({ key: 'videoCards', value: { ...prefs, ...change } });
  return { prefs, set };
};

/** Settings → Appearance: how video titles show, and their previews. */
export const VideoCardsGroup: FC = () => {
  const { t } = useTranslation();
  const { prefs, set } = useVideoCardPrefs();
  return (
    <PrefGroup title={t('videoCards.title')}>
      <PrefChoice<VideoCardStyle>
        title={t('videoCards.style')}
        value={prefs.style}
        options={VIDEO_CARD_STYLES.map((value) => ({
          value,
          label: t(`videoCards.styles.${value}`),
        }))}
        onChange={(style) => set({ style })}
      />
      <PrefChoice<VideoPreviews>
        title={t('videoCards.previews')}
        summary={t(`videoCards.previewHints.${prefs.previews}`)}
        value={prefs.previews}
        options={VIDEO_PREVIEWS.map((value) => ({
          value,
          label: t(`videoCards.previewModes.${value}`),
        }))}
        onChange={(previews) => set({ previews })}
      />
    </PrefGroup>
  );
};

const pill = (on: boolean) =>
  `rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;

/**
 * The card style and previews as pills, for a list's own view options (a library's Display, a
 * search's View). `styles` leaves out choices the list has its own way to (the library's List).
 */
export const VideoCardPills: FC<{ styles?: readonly VideoCardStyle[] }> = ({
  styles = VIDEO_CARD_STYLES,
}) => {
  const { t } = useTranslation();
  const { prefs, set } = useVideoCardPrefs();
  return (
    <div className="flex flex-col gap-3">
      <div role="group" aria-label={t('videoCards.style')} className="flex flex-col gap-2">
        <h3 className="text-muted text-xs font-semibold">{t('videoCards.style')}</h3>
        <div className="flex flex-wrap gap-2">
          {styles.map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={prefs.style === value}
              onClick={() => set({ style: value })}
              className={pill(prefs.style === value)}
            >
              {t(`videoCards.styles.${value}`)}
            </button>
          ))}
        </div>
      </div>
      <div role="group" aria-label={t('videoCards.previews')} className="flex flex-col gap-2">
        <h3 className="text-muted text-xs font-semibold">{t('videoCards.previews')}</h3>
        <div className="flex flex-wrap gap-2">
          {VIDEO_PREVIEWS.map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={prefs.previews === value}
              onClick={() => set({ previews: value })}
              className={pill(prefs.previews === value)}
            >
              {t(`videoCards.previewModes.${value}`)}
            </button>
          ))}
        </div>
        <p className="text-muted text-xs">{t(`videoCards.previewHints.${prefs.previews}`)}</p>
      </div>
    </div>
  );
};

/** A video list's View button: the card style and previews in a sheet (a popover when wide). */
export const VideoViewButton: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
        className={`border-border text-muted flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs ${className}`}
      >
        <LayoutGrid aria-hidden size={14} />
        {t('videoCards.view')}
      </button>
      {open && (
        <Sheet title={t('videoCards.title')} anchor={anchor} onClose={() => setOpen(false)}>
          <div className="px-4 pb-4">
            <VideoCardPills />
          </div>
        </Sheet>
      )}
    </>
  );
};
