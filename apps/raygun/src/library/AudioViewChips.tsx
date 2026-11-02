import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useLayout } from '../layout/useLayout';
import { isLibraryView } from './views';
import type { LibraryView } from './views';

/** Library → Audio's chips: everything, podcasts, and the music views. */
export const AudioViewChips: FC<{ view: LibraryView }> = ({ view }) => {
  const { t } = useTranslation();
  // The user's chips in their order (M46.5); the open one shows even when hidden.
  const views = useLayout()
    .audioViews.filter((item) => item.visible || item.id === view)
    .map((item) => item.id)
    .filter(isLibraryView);
  return (
    <nav
      className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
      aria-label={t('library.views')}
    >
      {views.map((option) => (
        <Link
          key={option}
          to="/library"
          search={{ view: option }}
          replace
          data-testid={`library-view-${option}`}
          aria-current={option === view ? 'page' : undefined}
          className={
            option === view
              ? 'bg-primary/20 text-primary shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
              : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm'
          }
        >
          {t(`library.view.${option}`)}
        </Link>
      ))}
    </nav>
  );
};
