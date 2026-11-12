import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** Best matches or By source: how several sources' answers are arranged (`useGrouped`). */
export const ArrangeToggle: FC<{
  grouped: boolean;
  onChange: (grouped: boolean) => void;
  className?: string;
}> = ({ grouped, onChange, className = 'ml-auto' }) => {
  const { t } = useTranslation();
  return (
    <div
      role="group"
      aria-label={t('search.arrange')}
      className={`bg-surface flex rounded-full p-0.5 text-sm ${className}`}
    >
      {([false, true] as const).map((byGroup) => (
        <button
          key={String(byGroup)}
          type="button"
          aria-pressed={grouped === byGroup}
          onClick={() => onChange(byGroup)}
          className={`rounded-full px-3 py-1 ${grouped === byGroup ? 'bg-primary/20 text-primary font-semibold' : 'text-muted'}`}
        >
          {t(byGroup ? 'search.bySource' : 'search.byResult')}
        </button>
      ))}
    </div>
  );
};
