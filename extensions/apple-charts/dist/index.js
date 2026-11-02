// dev.raygun.apple-charts 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var FEED = "https://rss.marketingtools.apple.com/api/v2";
  var DEFAULT_REGION = "us";
  var MAX_SONGS = 100;
  var HEADERS = { "User-Agent": "Raygun/1.0 (https://github.com/uvxszi/raygun)" };
  var fetchFeed = async (url) => {
    let response = await raygun.http.fetch(url, { headers: HEADERS });
    if (response.status >= 500) {
      await raygun.time.sleep(1e3);
      response = await raygun.http.fetch(url, { headers: HEADERS });
    }
    if (!response.ok) throw new Error(`Apple's charts answered HTTP ${response.status}`);
    return response.json();
  };
  var region = (asked) => asked && /^[a-z]{2}$/i.test(asked) ? asked.toLowerCase() : DEFAULT_REGION;
  var bigArtwork = (url) => url ? url.replace(/\/\d+x\d+bb\./, "/600x600bb.") : null;
  var countryName = (code) => {
    try {
      return new Intl.DisplayNames(["en"], { type: "region" }).of(code.toUpperCase()) ?? code;
    } catch {
      return code.toUpperCase();
    }
  };
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Apple Music",
      async charts(args) {
        const country = region(args.region);
        return {
          charts: [
            {
              id: "most-played",
              name: `Top songs: ${countryName(country)}`,
              description: "The most played songs on Apple Music"
            }
          ]
        };
      },
      async chart({ id, region: asked, limit }) {
        if (id !== "most-played") throw new Error(`Unknown chart ${id}`);
        const count = Math.min(Math.max(limit, 1), MAX_SONGS);
        const page = await fetchFeed(
          `${FEED}/${region(asked)}/music/most-played/${count}/songs.json`
        );
        const tracks = (page.feed.results ?? []).map((song) => ({
          id: song.id,
          title: song.name,
          artist: song.artistName ?? null,
          artworkUrl: bigArtwork(song.artworkUrl100)
        }));
        return { tracks };
      }
    })
  );
})();
