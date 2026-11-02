import { commands } from '@raygun/bindings';
import type { CategoryDisplay, LibraryList } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from './medium';

const marksKey = (medium: Medium, titleId: string) => [medium, 'marks', titleId];

/** A title's list, favorite and pin, for its page (M46). */
export const useTitleMarks = (medium: Medium, titleId: string | null | undefined) =>
  useQuery({
    queryKey: marksKey(medium, titleId ?? ''),
    queryFn: () => unwrapResult(commands.libraryTitleMarks(medium, titleId ?? '')),
    enabled: Boolean(titleId),
  });

/**
 * A change to library titles: the library, title pages, marks, categories and downloads may all
 * show it, so they're read again.
 */
const useLibraryMutation = <Args, Result>(medium: Medium, run: (args: Args) => Promise<Result>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () =>
      Promise.all(
        ['library', 'title', 'marks', 'categories', 'downloads', 'stremio'].map((part) =>
          queryClient.invalidateQueries({ queryKey: [medium, part] }),
        ),
      ).then(() => queryClient.invalidateQueries({ queryKey: ['stremio'] })),
  });
};

export const useSetList = (medium: Medium) =>
  useLibraryMutation(medium, ({ ids, list }: { ids: string[]; list: LibraryList | null }) =>
    unwrapResult(commands.librarySetList(medium, ids, list)),
  );

export const useSetFavorite = (medium: Medium) =>
  useLibraryMutation(medium, ({ ids, favorite }: { ids: string[]; favorite: boolean }) =>
    unwrapResult(commands.librarySetFavorite(medium, ids, favorite)),
  );

export const useSetPinned = (medium: Medium) =>
  useLibraryMutation(medium, ({ ids, pinned }: { ids: string[]; pinned: boolean }) =>
    unwrapResult(commands.librarySetPinned(medium, ids, pinned)),
  );

export const useMarkRead = (medium: Medium) =>
  useLibraryMutation(medium, ({ ids, read }: { ids: string[]; read: boolean }) =>
    unwrapResult(commands.libraryMarkRead(medium, ids, read)),
  );

export const useRemoveTitles = (medium: Medium) =>
  useLibraryMutation(
    medium,
    ({
      ids,
      fromLibrary,
      deleteDownloads,
    }: {
      ids: string[];
      fromLibrary: boolean;
      deleteDownloads: boolean;
    }) => unwrapResult(commands.libraryRemove(medium, ids, fromLibrary, deleteDownloads)),
  );

export const useChangeCategories = (medium: Medium) =>
  useLibraryMutation(
    medium,
    ({ ids, add, remove }: { ids: string[]; add: string[]; remove: string[] }) =>
      unwrapResult(commands.libraryChangeCategories(medium, ids, add, remove)),
  );

export const useDownloadNext = (medium: Medium) =>
  useLibraryMutation(medium, ({ ids, amount }: { ids: string[]; amount: number | null }) =>
    unwrapResult(commands.libraryDownloadNext(medium, ids, amount)),
  );

export const useSetCategoryDisplay = (medium: Medium) =>
  useLibraryMutation(
    medium,
    ({ categoryId, display }: { categoryId: string; display: CategoryDisplay | null }) =>
      unwrapResult(commands.librarySetCategoryDisplay(medium, categoryId, display)),
  );

export const useUpdateCategory = (medium: Medium) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (categoryId: string) =>
      unwrapResult(commands.libraryUpdateCategory(medium, categoryId)),
    onSuccess: (status) => queryClient.setQueryData(['manga', 'updateStatus'], status),
  });
};

/** Where a title continues (Mihon's continue button). */
export const fetchContinue = (medium: Medium, titleId: string) =>
  unwrapResult(commands.libraryContinue(medium, titleId));

/** Library titles that may be the same as one about to be added. */
export const fetchDuplicates = (medium: Medium, titleId: string, title: string) =>
  unwrapResult(commands.libraryDuplicates(medium, titleId, title));
