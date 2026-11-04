import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

type AdultMarkProps = {
  className?: string;
};

/**
 * The small "18+" tag beside a source or title rated for adults: extensions and repository
 * entries rated `nsfw`, adult Stremio addons, and titles with adult genres.
 */
export const AdultMark: FC<AdultMarkProps> = ({ className = '' }) => {
  const { t } = useTranslation();
  return (
    <span
      role="img"
      aria-label={t('common.adultLabel')}
      title={t('common.adultLabel')}
      data-testid="adult-mark"
      className={`border-muted/60 text-muted inline-flex shrink-0 items-center rounded-sm border px-1 align-middle text-[0.625rem] leading-[0.875rem] font-bold tabular-nums ${className}`}
    >
      {t('common.adult')}
    </span>
  );
};
