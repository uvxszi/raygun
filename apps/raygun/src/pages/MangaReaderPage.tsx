import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { useCallback } from 'react';
import type { FC } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { PageReader } from '../manga/reader/PageReader';
import { MANGA, saveProgress, useMangaChapter } from '../manga/useManga';

/** The manga reader: webtoon strip or paged, left or right to left (M27). */
export const MangaReaderPage: FC = () => {
  const { title, chapter } = useSearch({ from: '/manga/read' });
  const navigate = useNavigate({ from: '/manga/read' });
  const queryClient = useQueryClient();
  const loaded = useMangaChapter(title, chapter);

  const pagesOf = useCallback(
    (next: string) =>
      queryClient
        .fetchQuery({
          queryKey: [MANGA, 'chapter', title, next],
          queryFn: () => unwrapResult(commands.mangaChapter(title, next)),
          staleTime: 60_000,
        })
        .then((upcoming) => upcoming.pages),
    [queryClient, title],
  );
  // The title page, Continue reading and the updates feed show read state.
  const onLeave = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] });
    void queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] });
    void queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] });
  }, [queryClient]);

  return (
    <PageReader
      data={loaded.data}
      error={loaded.error}
      onRetry={() => void loaded.refetch()}
      medium="manga"
      // Chapters replace each other: Back leaves the reader.
      onGo={(next) => void navigate({ search: { title, chapter: next }, replace: true })}
      pagesOf={pagesOf}
      saveProgress={saveProgress}
      onLeave={onLeave}
    />
  );
};
