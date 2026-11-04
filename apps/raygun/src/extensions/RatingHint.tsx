import type { ContentOverride } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** The small note beside something the user marked: "Set by you", or "Not 18+ · set by you". */
export const RatingHint: FC<{ mark: ContentOverride; className?: string }> = ({
  mark,
  className = '',
}) => {
  const { t } = useTranslation();
  if (mark === 'default') return null;
  return (
    <span data-testid="rating-hint" className={`text-muted text-xs ${className}`}>
      {t(mark === 'adult' ? 'contentRating.setByYou' : 'contentRating.notAdultByYou')}
    </span>
  );
};
