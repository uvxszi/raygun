import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/**
 * Beside a source's name: what it shows came from its listings (popular, latest, its sorts, a
 * music source's charts), matched against the query, because its own search failed or found
 * nothing (`Settings::searchListings`).
 */
export const FromListingsNote: FC = () => {
  const { t } = useTranslation();
  return (
    <span
      data-testid="from-listings"
      title={t('search.fromListingsHint')}
      className="text-muted shrink-0 text-xs font-normal"
    >
      · {t('search.fromListing')}
    </span>
  );
};
