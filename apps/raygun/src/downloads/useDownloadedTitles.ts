import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';

/** Titles of `medium` with downloaded chapters or episodes, in the library or not. */
export const useDownloadedTitles = (medium: Medium) =>
  useQuery({
    // Under the library's key: whatever changes the library reads these again.
    queryKey: [medium, 'library', 'downloaded'],
    queryFn: () => unwrapResult(commands.libraryDownloaded(medium)),
  });
