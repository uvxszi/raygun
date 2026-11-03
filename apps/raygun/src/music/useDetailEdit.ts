import { commands } from '@raygun/bindings';
import type { DetailTarget } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

export const detailKey = (target: DetailTarget) => ['detailEdit', JSON.stringify(target)];

/** The user's own details for a song, an album or an artist (M46.30). */
export const useDetailEdit = (target: DetailTarget | null) =>
  useQuery({
    queryKey: target ? detailKey(target) : ['detailEdit', 'none'],
    queryFn: () => unwrapResult(commands.detailEdit(target as DetailTarget)),
    enabled: target !== null,
  });
