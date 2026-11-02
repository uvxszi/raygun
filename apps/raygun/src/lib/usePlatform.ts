import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

/** Which platform Raygun runs on (`android`, `windows`…), once known; it never changes. */
export const usePlatform = () =>
  useQuery({
    queryKey: ['appInfo'],
    queryFn: () => commands.appInfo(),
    staleTime: Infinity,
    select: (info) => info.platform,
  }).data;
