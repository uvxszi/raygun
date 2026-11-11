import { useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import { findInCache } from '../shell/queryMemory';
import { authorsIn } from './bookWords';
import type { Medium } from './medium';

/** What a list already showed of a title: enough for its page's header while it loads. */
export type KnownCard = { title: string; coverUrl: string | null; authors?: string[] };

/**
 * A title's name, cover and authors as a search or a list already showed them (M46.38), so its
 * page shows them at once, and still when the source then fails.
 */
export const useKnownCard = (medium: Medium, sourceId: string, id: string): KnownCard | null => {
  const client = useQueryClient();
  return useMemo(() => {
    if (!sourceId || !id) return null;
    return findInCache(client, [medium], (record): KnownCard | null => {
      if (
        record.sourceId !== sourceId ||
        (record.id !== id && record.remoteId !== id) ||
        typeof record.title !== 'string'
      ) {
        return null;
      }
      const authors = authorsIn(record);
      return {
        title: record.title,
        coverUrl: typeof record.coverUrl === 'string' ? record.coverUrl : null,
        ...(authors ? { authors } : {}),
      };
    });
  }, [client, medium, sourceId, id]);
};
