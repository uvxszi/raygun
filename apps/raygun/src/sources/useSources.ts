import { commands } from '@raygun/bindings';
import type { DetectedSource } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** What an address is, read before it's added (Sources → Add). */
export const useDetectSource = () =>
  useMutation({
    mutationFn: (address: string) => unwrapResult(commands.sourcesDetect(address)),
  });

/**
 * Adds what an address is: an addon, a repository, an extension. Everything it can bring then
 * shows: addons' catalogs, repositories' extensions, installed extensions.
 */
export const useAddSource = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (address: string) => unwrapResult(commands.sourcesAdd(address)),
    onSuccess: (added: DetectedSource) => {
      // Installed extensions name the repository they came from, so they follow a new one.
      const keys: string[][] =
        added.kind === 'addon'
          ? [['stremio']]
          : added.kind === 'repository'
            ? [
                ['extensionRepos'],
                ['extensionCatalog'],
                ['extensionSuggestedRepos'],
                ['extensions'],
              ]
            : [['extensions'], ['extensionCatalog']];
      for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey });
    },
  });
};
