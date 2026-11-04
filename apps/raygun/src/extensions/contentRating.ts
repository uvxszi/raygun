import type { ContentOverride, ContentOverrides } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { RotateCcw, ShieldAlert, ShieldCheck } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** A menu item, fit for both the ⋮ menus and the long-press and right-click menus. */
export type RatingItem = { label: string; icon: LucideIcon; onSelect: () => void };

/** What a mark is set on: an extension (and so its sources), a repository, a Stremio addon. */
export type RatingTarget = keyof ContentOverrides;

const NONE: Required<ContentOverrides> = { extensions: {}, repositories: {}, addons: {} };

/** The user's mark on `id`, `default` when there's none. */
export const markOf = (
  overrides: ContentOverrides | undefined,
  target: RatingTarget,
  id: string,
): ContentOverride => overrides?.[target]?.[id] ?? 'default';

/** `overrides` with `id` marked `mark`; `default` drops the mark. */
export const withMark = (
  overrides: ContentOverrides | undefined,
  target: RatingTarget,
  id: string,
  mark: ContentOverride,
): ContentOverrides => {
  const current = { ...NONE, ...overrides };
  const marks = { ...current[target] };
  if (mark === 'default') delete marks[id];
  else marks[id] = mark;
  return { ...current, [target]: marks };
};

/**
 * Marking sources 18+ or not 18+ over their own rating (`Settings::content_overrides`). Rust
 * applies the marks at once; every list is read again since any may depend on them.
 */
export const useContentRating = () => {
  const { t } = useTranslation();
  const overrides = useSettings().data?.contentOverrides;
  const update = useUpdateSetting();
  const queryClient = useQueryClient();

  const setMark = (target: RatingTarget, id: string, mark: ContentOverride) =>
    update.mutate(
      { key: 'contentOverrides', value: withMark(overrides, target, id, mark) },
      { onSuccess: () => void queryClient.invalidateQueries() },
    );

  /**
   * Menu items for `id`: the opposite of how it counts now (`adult`; a repository has no rating
   * of its own, so both until it's marked), and going back to the source's rating once marked.
   */
  const menuItems = (target: RatingTarget, id: string, adult?: boolean): RatingItem[] => {
    const mark = markOf(overrides, target, id);
    const counts = mark === 'default' ? adult : mark === 'adult';
    const markAdult: RatingItem = {
      label: t('contentRating.markAdult'),
      icon: ShieldAlert,
      onSelect: () => setMark(target, id, 'adult'),
    };
    const markNotAdult: RatingItem = {
      label: t('contentRating.markNotAdult'),
      icon: ShieldCheck,
      onSelect: () => setMark(target, id, 'notAdult'),
    };
    return [
      ...(counts !== true ? [markAdult] : []),
      ...(counts !== false ? [markNotAdult] : []),
      ...(mark !== 'default'
        ? [
            {
              label: t('contentRating.useOwn'),
              icon: RotateCcw,
              onSelect: () => setMark(target, id, 'default'),
            },
          ]
        : []),
    ];
  };

  return {
    overrides,
    markOf: (target: RatingTarget, id: string) => markOf(overrides, target, id),
    setMark,
    menuItems,
  };
};
