import { useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import { findInCache } from '../shell/queryMemory';
import type { Medium } from './medium';

/** What a list already showed of a title: enough for its page's header while it loads. */
export type KnownCard = { title: string; coverUrl: string | null };

/**
 * A title's name and cover as a search or a list already showed them (M46.38), so its page shows
 * them at once, and still when the source then fails.
 */
export const useKnownCard = (medium: Medium, sourceId: string, id: string): KnownCard | null => {
  const client = useQueryClient();
  return useMemo(() => {
    if (!sourceId || !id) return null;
    return findInCache(client, [medium], (record) =>
      record.sourceId === sourceId &&
      (record.id === id || record.remoteId === id) &&
      typeof record.title === 'string'
        ? {
            title: record.title,
            coverUrl: typeof record.coverUrl === 'string' ? record.coverUrl : null,
          }
        : null,
    );
  }, [client, medium, sourceId, id]);
};
