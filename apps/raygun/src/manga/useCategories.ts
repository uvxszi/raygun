import { commands } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from './medium';

const key = (medium: Medium) => [medium, 'categories'];

export const useCategories = (medium: Medium) =>
  useQuery({
    queryKey: key(medium),
    queryFn: () => unwrapResult(commands.libraryCategories(medium)),
  });

/** A category change: refreshes the categories and the library (memberships live there). */
const useCategoryMutation = <Args>(medium: Medium, run: (args: Args) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: key(medium) }),
        queryClient.invalidateQueries({ queryKey: [medium, 'library'] }),
      ]),
  });
};

export const useCreateCategory = (medium: Medium) =>
  useCategoryMutation(medium, (name: string) =>
    unwrapResult(commands.libraryCategoryCreate(medium, name)),
  );

export const useRenameCategory = (medium: Medium) =>
  useCategoryMutation(medium, ({ id, name }: { id: string; name: string }) =>
    unwrapResult(commands.libraryCategoryRename(medium, id, name)),
  );

export const useDeleteCategory = (medium: Medium) =>
  useCategoryMutation(medium, (id: string) =>
    unwrapResult(commands.libraryCategoryDelete(medium, id)),
  );

export const useReorderCategories = (medium: Medium) =>
  useCategoryMutation(medium, (ids: string[]) =>
    unwrapResult(commands.libraryCategoryReorder(medium, ids)),
  );

export const useSetTitleCategories = (medium: Medium) =>
  useCategoryMutation(medium, ({ titleId, ids }: { titleId: string; ids: string[] }) =>
    unwrapResult(commands.librarySetTitleCategories(medium, titleId, ids)),
  );
