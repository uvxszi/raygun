import { commands } from '@raygun/bindings';
import type { SourceSetup } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** Where the sources that need setup are kept; Rust's `SourcesSetupEvent` replaces it. */
export const SOURCES_SETUP_KEY = ['sourcesSetup'] as const;

const NONE: SourceSetup[] = [];

/**
 * The sources that need a key, an account or a sign-in before Raygun asks them anything, as Rust
 * lists them (turned off and hidden 18+ ones left out), each with the kinds it would answer and
 * what's missing. Kept current by `SourcesSetupEvent` (`shell/useLiveInvalidation`).
 */
export const useSourcesSetup = (): SourceSetup[] =>
  useQuery({
    queryKey: SOURCES_SETUP_KEY,
    queryFn: () => unwrapResult(commands.extensionsNeedingSetup()),
    staleTime: Infinity,
  }).data ?? NONE;

/** The extension an error says needs a key or sign-in (Rust's `CallError::NeedsSetup` line). */
export const needsSetupOf = (error: unknown): string | null => {
  if (typeof error !== 'string') return null;
  return /^needs setup \(([a-z0-9][a-z0-9.-]*)\)/m.exec(error)?.[1] ?? null;
};

/**
 * The source an error says needs a key or sign-in, named as the page knows it (`name`), else as
 * the error's first line does ("Podcast Index needs a key or sign-in"); null for other errors.
 */
export const setupSourceOf = (
  error: unknown,
  name?: string | null,
): { extensionId: string; name: string } | null => {
  const extensionId = needsSetupOf(error);
  if (!extensionId || typeof error !== 'string') return null;
  const said = /^(.+?) needs a key or sign-in/.exec(error.trim())?.[1];
  return { extensionId, name: name || said || extensionId };
};
