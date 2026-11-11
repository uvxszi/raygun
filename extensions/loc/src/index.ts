// Library of Congress (loc.gov): early films and sound recordings that are in the public domain
// in the United States, through the loc.gov JSON API (no key). Films are in `films.ts`,
// recordings in `recordings.ts`; `api.ts` keeps every request within the API's limits.
import { registerProvider } from '@raygun/sdk';

import { films } from './films';
import { recordingSearch } from './recordings';

registerProvider(films);
registerProvider(recordingSearch);
