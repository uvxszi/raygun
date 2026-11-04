import type { BrowseSection } from './BrowsePage';

export const BROWSE_TAB_LABEL: Record<BrowseSection, string> = {
  search: 'search.title',
  discover: 'discover.title',
  sources: 'browse.sources',
  extensions: 'extensions.title',
};

/** The tab Browse opens on: the chosen one while it's shown, else the first shown. */
export const browseStartTab = (
  chosen: string | undefined,
  shownTabs: BrowseSection[],
): BrowseSection =>
  shownTabs.find((id) => id === (chosen ?? 'discover')) ?? shownTabs[0] ?? 'search';
