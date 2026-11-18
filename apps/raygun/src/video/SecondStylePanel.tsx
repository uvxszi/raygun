import type { SubtitleFont, SubtitleStyle } from '@raygun/bindings';
import { X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SUBTITLE_FONTS } from '../settings/playerOptions';
import { PrefChoice, PrefGroup } from '../settings/prefs';
import { SubtitleStyleRows } from '../settings/SubtitleStyleRows';
import { SheetAboveContext } from '../shell/sheetAbove';

type Props = {
  /** How the second subtitles look now (their own style, else the main one's). */
  style: Required<SubtitleStyle>;
  family: string | null;
  /** A style kept for the title; null goes back to the main subtitles' look. */
  onChange: (style: SubtitleStyle | null) => void;
  onClose: () => void;
};

/**
 * The second subtitles' style over the desktop player, as Settings → Video player → Subtitles has
 * the main ones': size, font, colours, edge, background, height (from the picture's top) and line
 * width. Kept for the title; Like the main subtitles forgets it.
 */
export const SecondStylePanel: FC<Props> = ({ style, family, onChange, onClose }) => {
  const { t } = useTranslation();
  const title = t('videoPlayer.secondStyle');
  return (
    <section
      aria-label={title}
      data-testid="second-style-panel"
      onClick={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
      className="bg-background text-foreground motion-pop absolute inset-y-4 right-4 z-20 flex w-[min(32rem,calc(100%-2rem))] flex-col overflow-hidden rounded-2xl shadow-2xl ring-1 ring-white/10 select-text"
    >
      <header className="flex shrink-0 items-center gap-2 px-3 pt-3 pb-2">
        <h2 className="min-w-0 flex-1 truncate px-1 text-base font-semibold">{title}</h2>
        <button
          type="button"
          aria-label={t('desktopPlayer.closePanel')}
          onClick={onClose}
          className="hover:bg-surface grid size-9 place-items-center rounded-full"
        >
          <X aria-hidden size={18} />
        </button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-3">
        <SheetAboveContext.Provider value>
          <PrefGroup>
            <SubtitleStyleRows
              style={style}
              setStyle={(change) => onChange({ ...style, ...change })}
              onReset={() => onChange(null)}
              resetLabel={t('videoPlayer.secondStyleReset')}
              family={family}
              positionTitle={t('videoPlayer.height')}
              font={
                <PrefChoice<SubtitleFont>
                  title={t('settings.player.subtitleFont')}
                  value={style.font}
                  options={SUBTITLE_FONTS.map((value) => ({
                    value,
                    label: t(`settings.player.fonts.${value}`),
                  }))}
                  onChange={(font) => onChange({ ...style, font })}
                />
              }
            />
          </PrefGroup>
        </SheetAboveContext.Provider>
      </div>
    </section>
  );
};
