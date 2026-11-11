// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/youtube-auth.ts.

// YouTube's own request signatures, derived from its cookies the way its pages do it. Computed
// afresh for each request; neither the cookies nor the signatures leave the page's origin.
export async function getYouTubeAuthorization(origin: string, cookie: string, now = Date.now()) {
  const cookies = new Map(
    cookie.split(';').map((part) => {
      const separator = part.indexOf('=');
      return [part.slice(0, separator).trim(), part.slice(separator + 1)] as const;
    }),
  );
  const thirdParty = cookies.get('__Secure-3PAPISID');
  const sources: [string, string | undefined][] = [
    ['SAPISIDHASH', cookies.get('SAPISID') || thirdParty],
    ['SAPISID1PHASH', cookies.get('__Secure-1PAPISID')],
    ['SAPISID3PHASH', thirdParty],
  ];
  const timestamp = Math.floor(now / 1000);
  const signatures = await Promise.all(
    sources
      .filter((source): source is [string, string] => Boolean(source[1]))
      .map(async ([scheme, sid]) => {
        const digest = await crypto.subtle.digest(
          'SHA-1',
          new TextEncoder().encode(`${timestamp} ${sid} ${origin}`),
        );
        const hash = Array.from(new Uint8Array(digest), (byte) =>
          byte.toString(16).padStart(2, '0'),
        ).join('');
        return `${scheme} ${timestamp}_${hash}`;
      }),
  );
  return signatures.length ? signatures.join(' ') : undefined;
}
