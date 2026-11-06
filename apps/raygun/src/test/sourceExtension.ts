import type { ExtensionInfo } from '@raygun/bindings';

/**
 * A loaded extension with one provider of each kind given (`music.charts`, `podcast.directory`…):
 * what a screen needs installed before it offers that kind of thing.
 */
export const sourceExtension = (
  kinds: string[],
  id = 'dev.example.sources',
  name = 'Example sources',
): ExtensionInfo => ({
  id,
  name,
  contentRating: 'safe',
  adult: false,
  version: '1.0.0',
  description: null,
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: kinds,
  net: [],
  providers: kinds.map((kind) => ({
    id: `${id}/${kind}`,
    extensionId: id,
    kind,
    name,
    adult: false,
  })),
  settings: [],
  compatibility: null,
  icon: null,
  website: null,
  repository: null,
});
