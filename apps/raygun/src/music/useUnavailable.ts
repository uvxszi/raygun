import { commands, events } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';

/**
 * Which of these songs no source had when imported (M46.31). Background matching after an
 * import announces each song it finished, which refreshes saved lists and these marks.
 */
export const useUnavailable = (keys: string[]) => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.importEvent.listen(() => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void client.invalidateQueries({ queryKey: ['unavailable'] });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  const asked = useQuery({
    queryKey: ['unavailable', keys.join('|')],
    queryFn: () => unwrapResult(commands.tracksUnavailable(keys)),
    enabled: keys.length > 0,
  });
  return new Set(asked.data ?? []);
};
