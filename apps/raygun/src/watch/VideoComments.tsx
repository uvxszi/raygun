import { commands } from '@raygun/bindings';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Heart, LoaderCircle, MessageSquare } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';

/**
 * A video's comments from its source (M46.45: YouTube's), a page at a time. Hidden when the
 * source has none or doesn't offer them.
 */
export const VideoComments: FC<{ sourceId: string; videoId: string }> = ({ sourceId, videoId }) => {
  const { t } = useTranslation();
  const comments = useInfiniteQuery({
    queryKey: ['video', 'comments', sourceId, videoId],
    queryFn: ({ pageParam }) => unwrapResult(commands.videoComments(sourceId, videoId, pageParam)),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next ?? undefined,
    staleTime: 10 * 60_000,
    retry: false,
  });
  const list = comments.data?.pages.flatMap((page) => page.comments) ?? [];
  if (comments.isError || (comments.isSuccess && list.length === 0)) return null;
  return (
    <section
      aria-labelledby="video-comments"
      data-testid="video-comments"
      className="flex flex-col gap-3"
    >
      <h2 id="video-comments" className="flex items-center gap-2 font-semibold">
        <MessageSquare aria-hidden size={18} />
        {t('watch.comments')}
      </h2>
      {comments.isPending && (
        <LoaderCircle aria-label={t('watch.commentsLoading')} className="text-muted animate-spin" />
      )}
      <ul className="flex flex-col gap-4">
        {list.map((comment, index) => (
          <li key={`${comment.author}-${index}`} className="flex gap-3">
            <Artwork
              url={comment.avatarUrl ?? null}
              className="size-9 shrink-0 rounded-full"
              iconSize={16}
            />
            <div className="flex min-w-0 flex-col gap-1">
              <p className="text-muted text-xs">
                <span className="text-foreground font-medium">{comment.author}</span>
                {comment.published && ` · ${comment.published}`}
              </p>
              <p className="selectable text-sm leading-relaxed wrap-anywhere whitespace-pre-line">
                {comment.text}
              </p>
              <p className="text-muted flex items-center gap-3 text-xs">
                {comment.likes && (
                  <span className="flex items-center gap-1">
                    <Heart aria-hidden size={12} />
                    {comment.likes}
                  </span>
                )}
                {comment.replies ? t('watch.replies', { count: comment.replies }) : null}
              </p>
            </div>
          </li>
        ))}
      </ul>
      {comments.hasNextPage && (
        <button
          type="button"
          disabled={comments.isFetchingNextPage}
          onClick={() => void comments.fetchNextPage()}
          className="bg-surface self-start rounded-full px-4 py-2 text-sm disabled:opacity-60"
        >
          {comments.isFetchingNextPage ? t('watch.commentsLoading') : t('watch.moreComments')}
        </button>
      )}
    </section>
  );
};
