// dev.raygun.podcast-directory 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SEARCH = "https://itunes.apple.com/search";
  var LOOKUP = "https://itunes.apple.com/lookup";
  var TOP = "https://rss.marketingtools.apple.com/api/v2";
  var MAX_LIMIT = 50;
  var query = (params) => Object.entries(params).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
  var getJson = async (url) => {
    const response = await raygun.http.fetch(url);
    if (!response.ok) throw new Error(`The podcast directory answered ${response.status}`);
    return response.json();
  };
  var limitOf = (limit) => Math.min(Math.max(Math.floor(limit) || 20, 1), MAX_LIMIT);
  var toResult = (podcast) => podcast.collectionName && podcast.feedUrl ? {
    title: podcast.collectionName,
    author: podcast.artistName ?? null,
    feedUrl: podcast.feedUrl,
    artworkUrl: podcast.artworkUrl600 ?? podcast.artworkUrl100 ?? null,
    description: podcast.primaryGenreName ?? null
  } : null;
  var country = () => (raygun.settings.get("country") ?? "us").trim().toLowerCase().slice(0, 2) || "us";
  registerProvider(
    defineProvider({
      kind: "podcast.directory",
      id: "itunes",
      name: "Podcast directory",
      async search({ query: term, limit }) {
        const result = await getJson(
          `${SEARCH}?${query({ media: "podcast", entity: "podcast", limit: limitOf(limit), term })}`
        );
        return result.results.map(toResult).filter((item) => item !== null);
      },
      async top({ limit }) {
        const chart = await getJson(
          `${TOP}/${country()}/podcasts/top/${limitOf(limit)}/podcasts.json`
        );
        const ids = chart.feed.results.map((show) => show.id);
        if (ids.length === 0) return [];
        const found = await getJson(
          `${LOOKUP}?${query({ id: ids.join(","), entity: "podcast" })}`
        );
        const byId = new Map(found.results.map((podcast) => [String(podcast.collectionId), podcast]));
        return ids.map((id) => byId.get(id)).filter((podcast) => podcast !== void 0).map(toResult).filter((item) => item !== null);
      }
    })
  );
})();
