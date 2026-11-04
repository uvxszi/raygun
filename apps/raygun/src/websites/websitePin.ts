import type { Website } from '@raygun/bindings';

import type { PinTarget } from '../shell/useRail';

/** A site's pin in quick access: Library → Websites, which opens it (`site`), as Rust pins it. */
export const websitePin = (site: Pick<Website, 'id' | 'name'>): PinTarget => ({
  kind: 'website',
  to: '/library',
  search: { media: 'websites', site: site.id },
  name: site.name,
  artworkUrl: null,
});
