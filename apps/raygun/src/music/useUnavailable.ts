import { commands, events } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';

/**
 * Which of these songs no source had when imported (M46.31), or when played. Background
 * matching after an import announces each song it finished, and playback each song it found no
 * source for or found one again, which refreshes saved lists and these marks.
 */
export const useUnavailable = (keys: string[]) => {
  const client = useQueryClient();
  useEffect(() => {
    const refresh = () => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void client.invalidateQueries({ queryKey: ['unavailable'] });
    };
    const unlisten = [events.importEvent.listen(refresh), events.unavailableEvent.listen(refresh)];
    return () => {
      for (const stopping of unlisten) void stopping.then((stop) => stop());
    };
  }, [client]);
  const asked = useQuery({
    queryKey: ['unavailable', keys.join('|')],
    queryFn: () => unwrapResult(commands.tracksUnavailable(keys)),
    enabled: keys.length > 0,
  });
  return new Set(asked.data ?? []);
};
