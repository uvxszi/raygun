import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/**
 * The "E" beside an explicit song's title (M46.36): a small gray tag, as music apps show it
 * (the theme's gray with the page's color for the letter, so it reads in light and dark).
 */
export const ExplicitMark: FC = () => {
  const { t } = useTranslation();
  return (
    <span
      role="img"
      aria-label={t('songs.explicit')}
      title={t('songs.explicit')}
      data-testid="explicit-mark"
      className="bg-muted text-background inline-flex size-3.5 shrink-0 items-center justify-center rounded-[3px] align-middle text-[0.5625rem] leading-none font-bold"
    >
      E
    </span>
  );
};
