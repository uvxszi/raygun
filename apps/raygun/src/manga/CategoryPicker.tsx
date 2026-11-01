import { Link } from '@tanstack/react-router';
import { FolderOpen } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from './medium';
import { useCategories, useSetTitleCategories } from './useCategories';
import { useMangaLibrary } from './useManga';

/** On a library title's page: choose which categories it's in. */
export const CategoryPicker: FC<{ medium: Medium; titleId: string }> = ({ medium, titleId }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const categories = useCategories(medium);
  const library = useMangaLibrary(medium);
  const setCategories = useSetTitleCategories(medium);
  const current = library.data?.find((entry) => entry.titleId === titleId)?.categoryIds ?? [];
  const names = categories.data
    ?.filter((category) => current.includes(category.id))
    .map((category) => category.name);

  const toggle = (id: string) =>
    setCategories.mutate({
      titleId,
      ids: current.includes(id) ? current.filter((known) => known !== id) : [...current, id],
    });

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="bg-surface flex w-fit items-center gap-2 rounded-full px-4 py-2 text-sm"
      >
        <FolderOpen aria-hidden size={16} className="text-primary" />
        {names && names.length > 0 ? names.join(', ') : t('library.categories.choose')}
      </button>
      {open && (
        <div
          role="group"
          aria-label={t('library.categories.title')}
          className="bg-surface flex flex-col gap-2 rounded-xl px-4 py-3 text-sm"
        >
          {categories.data?.length === 0 && (
            <p className="text-muted">{t('library.categories.noneYet')}</p>
          )}
          {categories.data?.map((category) => (
            <label key={category.id} className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={current.includes(category.id)}
                disabled={setCategories.isPending}
                onChange={() => toggle(category.id)}
                className="accent-primary size-4"
              />
              {category.name}
            </label>
          ))}
          <Link to="/library/categories" search={{ medium }} className="text-primary w-fit">
            {t('library.categories.edit')}
          </Link>
        </div>
      )}
    </div>
  );
};
