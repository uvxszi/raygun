// Wikimedia Commons: public-domain and Creative Commons recordings and videos, through the
// MediaWiki API. Every file listed carries a public-domain mark or a Creative Commons licence;
// files filed under nudity or sex stay out unless the app shows 18+ content. Songs are in
// `audio.ts`, videos in `videos.ts`.
import { registerProvider } from '@raygun/sdk';

import { search, streams } from './audio';
import { videos } from './videos';

registerProvider(search);
registerProvider(streams);
registerProvider(videos);
