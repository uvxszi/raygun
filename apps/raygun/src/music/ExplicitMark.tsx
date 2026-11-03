import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** The "E" beside an explicit song's title (M46.36), as music apps show it. */
export const ExplicitMark: FC = () => {
  const { t } = useTranslation();
  return (
    <span
      role="img"
      aria-label={t('songs.explicit')}
      title={t('songs.explicit')}
      className="bg-muted/30 text-muted shrink-0 rounded-sm px-1 text-[0.625rem] leading-4 font-bold"
    >
      E
    </span>
  );
};
