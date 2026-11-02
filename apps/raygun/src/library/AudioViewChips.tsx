import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LIBRARY_VIEWS } from './views';
import type { LibraryView } from './views';

/** Library → Audio's chips: everything, podcasts, and the music views. */
export const AudioViewChips: FC<{ view: LibraryView }> = ({ view }) => {
  const { t } = useTranslation();
  return (
    <nav
      className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
      aria-label={t('library.views')}
    >
      {LIBRARY_VIEWS.map((option) => (
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
