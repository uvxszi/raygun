import type { CatalogEntry } from '@raygun/bindings';
import { useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { repoContents } from './catalogFilters';

/**
 * What a repository holds, as chips: "Comics sources · 12", "Book sources · 40", and how many
 * are Nuclear plugins or Shosetsu scripts.
 */
export const RepoContents: FC<{ entries: CatalogEntry[]; label: string }> = ({
  entries,
  label,
}) => {
  const { t } = useTranslation();
  const { kinds, formats } = useMemo(() => repoContents(entries), [entries]);
  if (kinds.length === 0) return null;
  const chip = 'bg-background text-muted rounded-full px-2.5 py-0.5 text-xs whitespace-nowrap';
  return (
    <ul aria-label={label} data-testid="repo-contents" className="flex flex-wrap gap-1.5">
      {kinds.map(({ kind, count }) => (
        <li key={kind} className={chip}>
          {t(`extensions.contents.${kind}`)} · {count}
        </li>
      ))}
      {formats.map(({ format, count }) => (
        <li key={format} className={chip}>
          {t(`extensions.format.${format}`)} · {count}
        </li>
      ))}
    </ul>
  );
};
