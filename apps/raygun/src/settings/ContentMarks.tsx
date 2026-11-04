import type { ContentOverride } from '@raygun/bindings';
import { X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useContentRating } from '../extensions/contentRating';
import type { RatingTarget } from '../extensions/contentRating';
import { useExtensions, useRepos } from '../extensions/useExtensions';
import { useStremioAddons } from '../watch/useStremio';

const KIND_LABEL: Record<RatingTarget, string> = {
  extensions: 'contentRating.kindExtension',
  repositories: 'contentRating.kindRepository',
  addons: 'contentRating.kindAddon',
};

const TARGETS: RatingTarget[] = ['extensions', 'repositories', 'addons'];

/**
 * Settings → Browse → Content: every source, extension and repository marked 18+ or not 18+,
 * each with a button back to its own rating. Nothing shows while nothing is marked.
 */
export const ContentMarks: FC = () => {
  const { t } = useTranslation();
  const rating = useContentRating();
  const extensions = useExtensions().data;
  const repos = useRepos().data;
  const addons = useStremioAddons().data;
  const nameOf = (target: RatingTarget, id: string) =>
    (target === 'extensions'
      ? extensions?.find((extension) => extension.id === id)?.name
      : target === 'repositories'
        ? repos?.find((repo) => repo.id === id)?.name
        : addons?.find((addon) => addon.id === id)?.name) ?? id;
  const rows = TARGETS.flatMap((target) =>
    Object.entries(rating.overrides?.[target] ?? {})
      .filter((entry): entry is [string, ContentOverride] => !!entry[1] && entry[1] !== 'default')
      .map(([id, mark]) => ({ target, id, mark, name: nameOf(target, id) })),
  ).sort((first, second) => first.name.localeCompare(second.name));
  if (rows.length === 0) return null;

  return (
    <li aria-label={t('contentRating.listTitle')} className="flex flex-col gap-1 px-1 py-3">
      <span className="font-medium">{t('contentRating.listTitle')}</span>
      <ul data-testid="content-marks" className="flex flex-col">
        {rows.map((row) => (
          <li key={`${row.target}-${row.id}`} className="flex items-center gap-3 py-1.5">
            <span className="flex min-w-0 flex-1 flex-col">
              <span className="truncate text-sm">{row.name}</span>
              <span className="text-muted text-xs">
                {[
                  t(KIND_LABEL[row.target]),
                  t(row.mark === 'adult' ? 'contentRating.adult' : 'contentRating.notAdult'),
                ].join(' · ')}
              </span>
            </span>
            <button
              type="button"
              aria-label={t('contentRating.clear', { name: row.name })}
              title={t('contentRating.useOwn')}
              onClick={() => rating.setMark(row.target, row.id, 'default')}
              className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full"
            >
              <X aria-hidden size={16} />
            </button>
          </li>
        ))}
      </ul>
    </li>
  );
};
