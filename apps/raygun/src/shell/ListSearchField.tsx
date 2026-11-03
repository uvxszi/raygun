import { Search, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** The field over a long list that narrows it (M46.35). */
export const ListSearchField: FC<{
  value: string;
  onChange: (value: string) => void;
  label: string;
  className?: string;
}> = ({ value, onChange, label, className = '' }) => {
  const { t } = useTranslation();
  return (
    <label
      className={`bg-surface flex min-w-0 items-center gap-2 rounded-full px-3 py-1.5 ${className}`}
    >
      <Search aria-hidden size={15} className="text-muted shrink-0" />
      <input
        type="search"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={label}
        aria-label={label}
        className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
      />
      {value && (
        <button
          type="button"
          aria-label={t('search.clear')}
          onClick={() => onChange('')}
          className="text-muted hover:text-foreground grid size-5 place-items-center rounded-full"
        >
          <X aria-hidden size={13} />
        </button>
      )}
    </label>
  );
};
