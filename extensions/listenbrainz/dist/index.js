// dev.raygun.listenbrainz 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var fetchJson = async (url, init) => {
    const response = await raygun.http.fetch(url, init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} from ${url}`);
    }
    return response.json();
  };

  // src/index.ts
  var API = "https://api.listenbrainz.org/1";
  var HEADERS = { "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )" };
  var RANGES = {
    week: { name: "Most listened this week", description: "Everyone on ListenBrainz, last week" },
    month: { name: "Most listened this month", description: "Everyone on ListenBrainz, last month" },
    year: { name: "Most listened this year", description: "Everyone on ListenBrainz, this year" }
  };
  var FRESH = "fresh";
  var LOOKUP_BATCH = 25;
  var cover = (release, id) => release && id ? `https://coverartarchive.org/release/${release}/${id}-500.jpg` : null;
  var lookup = async (mbids) => {
    const tracks = [];
    for (let start = 0; start < mbids.length; start += LOOKUP_BATCH) {
      const batch = mbids.slice(start, start + LOOKUP_BATCH);
      const found = await fetchJson(
        `${API}/metadata/recording/?recording_mbids=${batch.join(",")}&inc=artist%20release`,
        { headers: HEADERS }
      );
      for (const mbid of batch) {
        const entry = found[mbid];
        const title = entry?.recording?.name;
        if (!entry || !title) continue;
        tracks.push({
          id: mbid,
          title,
          artist: entry.artist?.name ?? null,
          album: entry.release?.name ?? null,
          durationMs: entry.recording?.length ?? null,
          artworkUrl: cover(entry.release?.caa_release_mbid, entry.release?.caa_id)
        });
      }
    }
    return tracks;
  };
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "ListenBrainz",
      async charts() {
        const charts = Object.keys(RANGES).map((range) => ({
          id: range,
          ...RANGES[range],
          group: "top"
        }));
        charts.push({
          id: FRESH,
          name: "Fresh singles",
          description: "Singles from the last two weeks that ListenBrainz listeners play most",
          group: "new"
        });
        return { charts };
      },
      async chart({ id, limit }) {
        const count = Math.min(Math.max(limit, 1), 100);
        if (id === FRESH) {
          const fresh = await fetchJson(
            `${API}/explore/fresh-releases/?days=14&sort=release_date&past=true&future=false`,
            { headers: HEADERS }
          );
          const singles = (fresh.payload?.releases ?? []).filter((release) => release.release_group_primary_type === "Single").sort((a, b) => (b.listen_count ?? 0) - (a.listen_count ?? 0)).slice(0, count);
          const tracks2 = singles.map((release) => ({
            id: release.release_mbid,
            title: release.release_name,
            artist: release.artist_credit_name,
            album: release.release_name,
            artworkUrl: cover(release.caa_release_mbid, release.caa_id)
          }));
          return { tracks: tracks2 };
        }
        if (!(id in RANGES)) throw new Error(`Unknown chart ${id}`);
        const stats = await fetchJson(
          `${API}/stats/sitewide/recordings?count=${count}&range=${id}`,
          { headers: HEADERS }
        );
        const tracks = (stats.payload?.recordings ?? []).map((recording) => ({
          id: recording.recording_mbid ?? void 0,
          title: recording.track_name,
          artist: recording.artist_name ?? null,
          album: recording.release_name ?? null,
          artworkUrl: cover(recording.caa_release_mbid, recording.caa_id)
        }));
        return { tracks };
      },
      // Tag radio: recordings listeners tagged with the genre or mood (or one of its other names),
      // the better known ones first.
      async tagSongs({ tag, aliases, limit }) {
        const names = [tag, ...aliases].map((name) => name.toLowerCase()).filter((name, index, all) => all.indexOf(name) === index).slice(0, 5);
        const params = [
          ...names.map((name) => `tag=${encodeURIComponent(name)}`),
          "operator=or",
          "pop_begin=30",
          "pop_end=100",
          `count=${Math.min(Math.max(limit, 1), 100)}`
        ];
        const hits = await fetchJson(`${API}/lb-radio/tags?${params.join("&")}`, {
          headers: HEADERS
        });
        const mbids = [...new Set(hits.map((hit) => hit.recording_mbid))].slice(0, limit);
        return { tracks: await lookup(mbids), name: `${tag} on ListenBrainz` };
      }
    })
  );
})();
