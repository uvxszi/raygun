// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/watch-nav.ts.
//
// "Back skips watched videos": going from one watch page to another replaces the history entry
// instead of adding one, so Back returns to the page before the videos (a search, a channel)
// rather than through every video watched.

function isWatch(address: string): boolean {
  try {
    return new URL(address, location.href).pathname.startsWith('/watch');
  } catch {
    return false;
  }
}

function resolveHistoryUrl(url: unknown): string {
  if ((typeof url === 'string' && url) || url instanceof URL) {
    try {
      return new URL(url, location.href).href;
    } catch {
      return location.href;
    }
  }
  return location.href;
}

export function installWatchNavigation() {
  const pushState = history.pushState;
  history.pushState = function (
    this: History,
    data: unknown,
    unused: string,
    url?: string | URL | null,
  ) {
    if (isWatch(location.href) && isWatch(resolveHistoryUrl(url))) {
      return history.replaceState.call(this, data, unused, url);
    }
    return pushState.call(this, data, unused, url);
  };
}
