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
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");

  // src/index.ts
  var API = "https://api.listenbrainz.org/1";
  var LABS = "https://labs.api.listenbrainz.org";
  var MUSICBRAINZ = "https://musicbrainz.org/ws/2";
  var HEADERS = {
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )",
    Accept: "application/json"
  };
  var MOST = 100;
  var SIMILAR_ARTISTS = "session_based_days_7500_session_300_contribution_5_threshold_10_limit_100_filter_True_skip_30";
  var SIMILAR_RECORDINGS = "session_based_days_9000_session_300_contribution_5_threshold_15_limit_50_skip_30";
  var HOUR = 60 * 60 * 1e3;
  var MONTH = 30 * 24 * HOUR;
  var RANGES = [
    { id: "week", name: "Top songs this week", description: "Most listened to on ListenBrainz" },
    { id: "month", name: "Top songs this month", description: "Most listened to on ListenBrainz" },
    { id: "year", name: "Top songs this year", description: "Most listened to on ListenBrainz" },
    {
      id: "all_time",
      name: "Top songs of all time",
      description: "Most listened to on ListenBrainz"
    }
  ];
  var coverArt = (caaId, release) => caaId && release ? `https://coverartarchive.org/release/${release}/${caaId}-250.jpg` : null;
  var toTrack = (recording, index) => ({
    id: recording.recording_mbid ?? `${index}`,
    title: recording.track_name,
    artist: recording.artist_name ?? null,
    album: recording.release_name ?? null,
    artworkUrl: coverArt(recording.caa_id, recording.caa_release_mbid)
  });
  var get = (url) => fetchJson(url, { headers: HEADERS });
  var normalized = (text) => text.normalize("NFKD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/&/g, " and ").replace(/[^\p{L}\p{N}]+/gu, " ").replace(/^the /, "").trim();
  var songKey = (title) => normalized(title.replace(/\s*[([].*?[)\]]\s*/g, " ").replace(/\s+-\s+.*$/, ""));
  var clamp = (value, low, high) => Math.max(low, Math.min(Number.isFinite(value) ? Math.floor(value) : low, high));
  var musicBrainzTurn = Promise.resolve();
  var lastMusicBrainz = 0;
  var musicBrainz = (url) => {
    const ask = musicBrainzTurn.then(async () => {
      const wait = lastMusicBrainz + 1100 - Date.now();
      if (wait > 0) await raygun.time.sleep(wait);
      lastMusicBrainz = Date.now();
      const response = await raygun.http.fetch(url, { headers: HEADERS });
      lastMusicBrainz = Date.now();
      if (response.status === 503) {
        await raygun.time.sleep(1500);
        lastMusicBrainz = Date.now();
        return fetchJson(url, { headers: HEADERS });
      }
      if (!response.ok) throw new Error(`HTTP ${response.status} from ${url}`);
      return response.json();
    });
    musicBrainzTurn = ask.catch(() => void 0);
    return ask;
  };
  var artistId = async (name) => {
    const wanted = normalized(name);
    if (!wanted) return null;
    const key = `artist:${wanted}`;
    const kept = raygun.storage.get(key);
    if (kept && Date.now() - kept.at < MONTH) return kept.id;
    const found = await musicBrainz(
      `${MUSICBRAINZ}/artist/?${query({ query: `artist:"${name.replace(/["\\]/g, " ")}"`, fmt: "json", limit: 5 })}`
    );
    const artists = found.artists ?? [];
    const exact = artists.find(
      (artist) => normalized(artist.name) === wanted || (artist.aliases ?? []).some((alias) => normalized(alias.name) === wanted)
    );
    const id = exact?.id ?? (artists[0] && (artists[0].score ?? 0) >= 95 ? artists[0].id : null);
    remember(key, { id, at: Date.now() });
    return id;
  };
  var remember = (key, value) => {
    try {
      raygun.storage.set(key, value);
    } catch {
      const old = raygun.storage.keys().filter((name) => name.startsWith("artist:")).map((name) => ({ name, at: raygun.storage.get(name)?.at ?? 0 })).sort((a, b) => a.at - b.at);
      for (const entry of old.slice(0, Math.ceil(old.length / 2))) raygun.storage.remove(entry.name);
      raygun.storage.set(key, value);
    }
  };
  var needArtist = async (name) => {
    const id = await artistId(name);
    if (!id) throw new Error(`MusicBrainz doesn't know an artist named ${name}`);
    return id;
  };
  var songsOf = async (ids) => {
    const tracks = [];
    for (let start = 0; start < ids.length; start += 50) {
      const batch = ids.slice(start, start + 50);
      const found = await get(
        `${API}/metadata/recording/?${query({ recording_mbids: batch.join(","), inc: "artist release" })}`
      );
      for (const id of batch) {
        const meta = found[id];
        const title = meta?.recording?.name?.trim();
        if (!title) continue;
        tracks.push({
          id,
          title,
          artist: meta?.artist?.name ?? null,
          album: meta?.release?.name ?? null,
          durationMs: meta?.recording?.length ?? null,
          artworkUrl: coverArt(meta?.release?.caa_id, meta?.release?.caa_release_mbid)
        });
      }
    }
    return tracks;
  };
  var distinct = (tracks) => {
    const seen = /* @__PURE__ */ new Set();
    return tracks.filter((track) => {
      const key = `${normalized(track.artist ?? "")}|${songKey(track.title)}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  };
  var artistRadio = (id, similar, perArtist, popularity) => get(
    `${API}/lb-radio/artist/${id}?${query({
      mode: "easy",
      max_similar_artists: similar,
      max_recordings_per_artist: perArtist,
      pop_begin: popularity,
      pop_end: 100
    })}`
  );
  var interleave = (groups) => {
    const order = [];
    const longest = Math.max(0, ...groups.map((group) => group.length));
    for (let index = 0; index < longest; index += 1) {
      for (const group of groups) {
        const recording = group[index];
        if (recording) order.push(recording.recording_mbid);
      }
    }
    return order;
  };
  var freshKept = /* @__PURE__ */ new Map();
  var freshReleases = async (days) => {
    const kept = freshKept.get(days);
    if (kept && Date.now() - kept.at < HOUR) return kept.releases;
    const answer = await get(
      `${API}/explore/fresh-releases/?${query({ days, sort: "release_date", past: true, future: false })}`
    );
    const releases = (answer.payload?.releases ?? []).map((release) => ({
      artist_mbids: release.artist_mbids,
      release_mbid: release.release_mbid,
      release_name: release.release_name,
      release_date: release.release_date,
      caa_id: release.caa_id,
      caa_release_mbid: release.caa_release_mbid
    }));
    freshKept.clear();
    freshKept.set(days, { at: Date.now(), releases });
    return releases;
  };
  var credit = (names) => (names ?? []).map((part) => `${part.name ?? ""}${part.joinphrase ?? ""}`).join("").trim() || null;
  var releaseSongs = async (release) => {
    const found = await musicBrainz(
      `${MUSICBRAINZ}/release/${release.release_mbid}?${query({ inc: "recordings artist-credits", fmt: "json" })}`
    );
    const album = found.title ?? release.release_name ?? null;
    return (found.media ?? []).flatMap(
      (medium) => (medium.tracks ?? []).flatMap((track) => {
        const title = (track.title ?? track.recording?.title ?? "").trim();
        if (!title) return [];
        return [
          {
            id: track.recording?.id ?? `${release.release_mbid}:${title}`,
            title,
            artist: credit(track["artist-credit"]) ?? credit(found["artist-credit"]),
            album,
            durationMs: track.length ?? track.recording?.length ?? null,
            artworkUrl: coverArt(release.caa_id, release.caa_release_mbid)
          }
        ];
      })
    );
  };
  var recordingOf = async (artist, title) => {
    const found = await get(
      `${LABS}/recording-search/json?${query({ query: `${artist} ${title}` })}`
    );
    const wantedArtist = normalized(artist);
    const wantedTitle = songKey(title);
    const match = found.find(
      (recording) => normalized(recording.artist_credit_name ?? "") === wantedArtist && songKey(recording.recording_name ?? "") === wantedTitle
    );
    return match?.recording_mbid ?? null;
  };
  var songRadio = async (recording) => {
    const similar = await get(
      `${LABS}/similar-recordings/json?${query({ recording_mbids: recording, algorithm: SIMILAR_RECORDINGS })}`
    );
    return similar.flatMap(
      (found) => found.recording_name?.trim() ? [
        {
          id: found.recording_mbid,
          title: found.recording_name.trim(),
          artist: found.artist_credit_name ?? null,
          album: found.release_name ?? null,
          artworkUrl: coverArt(found.caa_id, found.caa_release_mbid)
        }
      ] : []
    );
  };
  var tagKept = /* @__PURE__ */ new Map();
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "ListenBrainz",
      async charts() {
        const charts = RANGES.map((range) => ({ ...range, group: "top" }));
        return { charts };
      },
      async chart({ id, limit }) {
        const range = RANGES.find((known) => known.id === id)?.id ?? "week";
        const count = Math.max(1, Math.min(limit || 50, MOST));
        const answer = await get(
          `${API}/stats/sitewide/recordings?range=${range}&count=${count}`
        );
        const tracks = (answer.payload?.recordings ?? []).filter((recording) => recording.track_name).map(toTrack);
        return { tracks };
      },
      // LB Radio's tag mode: songs MusicBrainz tags with the genre (or any of its spellings).
      async tagSongs({ tag, aliases, limit }) {
        const tags = [...new Set([tag, ...aliases].map((name) => name.trim().toLowerCase()))].filter(Boolean).slice(0, 4);
        if (tags.length === 0) return { tracks: [] };
        const count = clamp(limit || 25, 1, 50);
        const key = `${tags.join("|")}:${count}`;
        const kept = tagKept.get(key);
        if (kept && Date.now() - kept.at < 6 * HOUR) return { tracks: kept.tracks };
        const found = await get(
          `${API}/lb-radio/tags?${query({ tag: tags, operator: "or", pop_begin: 0, pop_end: 100, count: count * 2 })}`
        );
        const tracks = distinct(await songsOf(found.map((recording) => recording.recording_mbid)));
        const shown = tracks.slice(0, count);
        if (tagKept.size > 40) tagKept.clear();
        tagKept.set(key, { at: Date.now(), tracks: shown });
        return { tracks: shown };
      }
    })
  );
  registerProvider(
    defineProvider({
      kind: "music.recommend",
      id: "recommend",
      name: "ListenBrainz",
      async similarArtists({ artist, limit }) {
        const id = await artistId(artist);
        if (!id) return { artists: [] };
        const similar = await get(
          `${LABS}/similar-artists/json?${query({ artist_mbids: id, algorithm: SIMILAR_ARTISTS })}`
        );
        const artists = similar.filter((found) => found.artist_mbid !== id && found.name?.trim()).slice(0, clamp(limit || 10, 1, 100)).map((found) => ({ id: found.artist_mbid, name: found.name.trim() }));
        return { artists };
      },
      // The artist's most listened-to songs: a large sample of LB Radio's most popular tenth (it
      // picks at random within it), by listens.
      async topTracks({ artist, limit }) {
        const id = await needArtist(artist);
        const count = clamp(limit || 10, 1, 50);
        const radio = await artistRadio(id, 0, 200, 90);
        const own = (radio[id] ?? []).slice().sort((a, b) => (b.total_listen_count ?? 0) - (a.total_listen_count ?? 0)).slice(0, count * 3);
        const tracks = distinct(await songsOf(own.map((recording) => recording.recording_mbid)));
        return { tracks: tracks.slice(0, count) };
      },
      // Fresh releases of the last `days` days (at most 30) whose artist is this one, newest first.
      async newReleases({ artist, days, limit }) {
        const id = await artistId(artist);
        if (!id) return { tracks: [] };
        const releases = (await freshReleases(clamp(days || 28, 1, 30))).filter((release) => release.release_mbid && (release.artist_mbids ?? []).includes(id)).sort((a, b) => (b.release_date ?? "").localeCompare(a.release_date ?? ""));
        const count = clamp(limit || 10, 1, 50);
        const tracks = [];
        for (const release of releases.slice(0, 3)) {
          if (tracks.length >= count) break;
          tracks.push(...await releaseSongs(release));
        }
        return { tracks: distinct(tracks).slice(0, count) };
      },
      // Songs like one song (Labs' similar recordings), else songs by the artist and similar ones.
      async radio({ artist, title, limit }) {
        const count = clamp(limit || 25, 1, 100);
        if (title?.trim()) {
          const recording = await recordingOf(artist, title);
          if (recording) {
            const tracks2 = distinct(await songRadio(recording)).slice(0, count);
            if (tracks2.length > 0) return { tracks: tracks2 };
          }
        }
        const id = await needArtist(artist);
        const radio = await artistRadio(id, 10, Math.max(2, Math.ceil(count / 8)), 50);
        const groups = Object.entries(radio).sort(([a], [b]) => Number(a === id) - Number(b === id)).map(([, recordings]) => recordings);
        const tracks = distinct(await songsOf(interleave(groups).slice(0, count * 2)));
        return { tracks: tracks.slice(0, count) };
      }
    })
  );
})();
