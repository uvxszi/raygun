// Europeana (europeana.eu): films and recordings under open licences from Europe's archives,
// libraries and museums, through the Europeana Search and Record APIs. Only `reusability=open`
// items (Public Domain Mark, CC0, CC BY, CC BY-SA), checked again item by item.
//
// Keys are personal and confidential: each person creates a free one in their europeana.eu
// account and enters it in this source's settings. The setting is required, so Raygun asks
// nothing while it's empty, and a key Europeana refuses says so with `needsSetup`. Films are in
// `videos.ts`, recordings in `music.ts`.
import { registerProvider } from '@raygun/sdk';

import { recordings } from './music';
import { videos } from './videos';

registerProvider(videos);
registerProvider(recordings);
