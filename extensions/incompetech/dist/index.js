// dev.raygun.incompetech 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var SITE = "https://incompetech.com/music/royalty-free";
  var ARTIST = "Kevin MacLeod";
  var KEEP_MS = 12 * 60 * 60 * 1e3;
  var kept = null;
  var reading = null;
  var read = async () => {
    const [pieces, genres, collections] = await Promise.all([
      fetchJson(`${SITE}/pieces.json`),
      fetchJson(`${SITE}/genre.json`),
      fetchJson(`${SITE}/collections.json`)
    ]);
    return {
      pieces: (Array.isArray(pieces) ? pieces : []).filter(
        (piece) => typeof piece.title === "string" && piece.title.trim() && piece.filename
      ),
      genres: new Map((genres ?? []).map((item) => [String(item.id), item.genre])),
      collections: new Map((collections ?? []).map((item) => [String(item.id), item])),
      readAt: Date.now()
    };
  };
  var catalogue = async () => {
    if (kept && Date.now() - kept.readAt < KEEP_MS) return kept;
    reading ??= read().finally(() => {
      reading = null;
    });
    try {
      kept = await reading;
    } catch (error) {
      if (kept) return kept;
      throw error;
    }
    return kept;
  };
  var simple = (text) => (text ?? "").toLowerCase().replace(/['’]/g, "").replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var wordsOf = (text) => simple(text).split(" ").filter(Boolean);
  var parseLength = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var fileAddress = (filename) => {
    let name = filename;
    try {
      name = decodeURIComponent(filename);
    } catch {
    }
    return `${SITE}/mp3-royaltyfree/${encodeURIComponent(name)}`;
  };
  var albumOf = (piece, books) => {
    const collection = books.collections.get(String(piece.collection ?? ""));
    return collection?.collection_name?.trim() || null;
  };
  var toTrack = (piece, books) => ({
    id: piece.filename ?? piece.title ?? "",
    title: (piece.title ?? "").trim(),
    artist: ARTIST,
    album: albumOf(piece, books),
    durationMs: parseLength(piece.length),
    source: { kind: "url", url: fileAddress(piece.filename ?? ""), headers: {} }
  });
  var genreOf = (piece, books) => books.genres.get(String(piece.genre ?? "")) ?? "";
  var feelsOf = (piece) => (piece.feel ?? "").split(",").map((feel) => simple(feel)).filter(Boolean);
  var newestFirst = (a, b) => (b.uploaded ?? "").localeCompare(a.uploaded ?? "");
  var WELL_KNOWN = [
    "Monkeys Spinning Monkeys",
    "Sneaky Snitch",
    "Fluffing a Duck",
    "Carefree",
    "Investigations",
    "Local Forecast - Elevator",
    "Wallpaper",
    "Pixel Peeker Polka - faster",
    "Merry Go",
    "Cipher",
    "Volatile Reaction",
    "Aurea Carmina",
    "Amazing Plan",
    "Spazzmatica Polka",
    "Itty Bitty 8 Bit",
    "Run Amok",
    "Life of Riley",
    "The Builder",
    "Bushwick Tarantella",
    "Thatched Villagers",
    "Sneaky Adventure",
    "Quirky Dog",
    "Fig Leaf Rag",
    "Mining by Moonlight",
    "Batty McFaddin",
    "Hyperfun",
    "Sugar Plum Breakdown",
    "Kool Kats",
    "Wholesome",
    "Cheery Monday",
    "Funkorama",
    "Easy Lemon",
    "Hidden Agenda",
    "Hitman",
    "Night on the Docks - Sax",
    "Lobby Time",
    "Electrodoodle",
    "Pamgaea",
    "Vivacity",
    "Airport Lounge",
    "Bossa Antigua",
    "Getting it Done",
    "Clean Soul",
    "Jaunty Gumption"
  ].map(simple);
  var knownRank = (piece) => {
    const rank = WELL_KNOWN.indexOf(simple(piece.title));
    return rank < 0 ? Number.MAX_SAFE_INTEGER : rank;
  };
  var byStanding = (a, b) => knownRank(a) - knownRank(b) || newestFirst(a, b);
  var GENRE_TAGS = {
    Soundtrack: "Soundtrack",
    "Silent Film Score": "Soundtrack",
    Electronica: "Electronic",
    Jazz: "Jazz",
    Rock: "Rock",
    Classical: "Classical",
    Funk: "Funk",
    Latin: "Latin",
    Pop: "Pop",
    Blues: "Blues",
    Reggae: "Reggae",
    Disco: "Disco",
    Ska: "Ska",
    Urban: "Hip-Hop"
  };
  var NO_CHART = /* @__PURE__ */ new Set(["Stings", "Unclassifiable"]);
  var CHART_MIN = 15;
  var MOOD_FEELS = {
    chill: ["relaxed", "calming"],
    energetic: ["driving", "intense", "action", "aggressive"],
    happy: ["bright", "uplifting", "bouncy", "humorous"],
    sad: ["somber"],
    dark: ["dark", "eerie", "unnerving", "suspenseful"],
    party: ["grooving", "bouncy"],
    focus: ["calming", "mystical"],
    workout: ["driving", "action", "intense"],
    sleep: ["calming"]
  };
  var CHART_MOODS = ["Chill", "Energetic", "Happy", "Sad", "Dark"];
  var inGenre = (wanted, books) => (piece) => {
    const genre = genreOf(piece, books);
    const tag = GENRE_TAGS[genre];
    return wanted.includes(simple(genre)) || tag !== void 0 && wanted.includes(simple(tag));
  };
  var inMood = (wanted) => {
    const feels = new Set(wanted.flatMap((name) => [name, ...MOOD_FEELS[name] ?? []]));
    return (piece) => feelsOf(piece).some((feel) => feels.has(feel));
  };
  var atMost = (limit, fallback = 50) => Math.max(1, Math.min(limit || fallback, 200));
  var searchPieces = (text, books) => {
    const wanted = wordsOf(text);
    if (wanted.length === 0) return [];
    const scored = [];
    for (const piece of books.pieces) {
      const title = wordsOf(piece.title ?? "");
      const about = /* @__PURE__ */ new Set([
        ...title,
        ...wordsOf(genreOf(piece, books)),
        ...wordsOf(piece.feel ?? ""),
        ...wordsOf(piece.instruments ?? ""),
        ...wordsOf(piece.description ?? ""),
        ...wordsOf(albumOf(piece, books) ?? "")
      ]);
      if (!wanted.every((word) => about.has(word) || title.some((part) => part.startsWith(word)))) {
        continue;
      }
      const inTitle = wanted.filter((word) => title.some((part) => part.startsWith(word))).length;
      scored.push({ piece, score: inTitle * 10 + (simple(piece.title) === simple(text) ? 100 : 0) });
    }
    return scored.sort((a, b) => b.score - a.score || byStanding(a.piece, b.piece)).map((entry) => entry.piece);
  };
  var toAlbum = (collection, books) => ({
    id: String(collection.id),
    title: collection.collection_name.trim(),
    artist: ARTIST,
    trackCount: books.pieces.filter(
      (piece) => String(piece.collection ?? "") === String(collection.id)
    ).length,
    kind: "album"
  });
  var metadata = defineProvider({
    kind: "music.metadata",
    id: "search",
    name: "Incompetech",
    async search({ query, limit }) {
      const books = await catalogue();
      const found = searchPieces(query, books);
      return { tracks: found.slice(0, atMost(limit, 20)).map((piece) => toTrack(piece, books)) };
    },
    async searchAlbums({ query, limit }) {
      const books = await catalogue();
      const wanted = wordsOf(query);
      const artistAsked = wanted.length > 0 && wanted.every((word) => simple(ARTIST).includes(word));
      const albums = [...books.collections.values()].filter((collection) => {
        const about = wordsOf(
          `${collection.collection_name} ${collection.collection_category ?? ""}`
        );
        return artistAsked || wanted.every((word) => about.some((part) => part.startsWith(word)));
      }).map((collection) => toAlbum(collection, books)).filter((album) => (album.trackCount ?? 0) > 0);
      return { albums: albums.slice(0, atMost(limit, 20)) };
    },
    async album({ id }) {
      const books = await catalogue();
      const collection = books.collections.get(id);
      if (!collection) throw new Error("Incompetech no longer has this collection");
      const tracks = books.pieces.filter((piece) => String(piece.collection ?? "") === id).sort(
        (a, b) => (a.uploaded ?? "").localeCompare(b.uploaded ?? "") || (a.title ?? "").localeCompare(b.title ?? "")
      ).map((piece) => toTrack(piece, books));
      return { album: toAlbum(collection, books), tracks };
    },
    async artistReleases({ query, limit }) {
      if (simple(query) !== simple(ARTIST)) return { releases: [] };
      const books = await catalogue();
      const albums = [...books.collections.values()].map((collection) => toAlbum(collection, books)).filter((album) => (album.trackCount ?? 0) > 0);
      return { releases: albums.slice(0, atMost(limit, 50)) };
    }
  });
  var charts = defineProvider({
    kind: "music.charts",
    id: "charts",
    name: "Incompetech",
    async charts() {
      const books = await catalogue();
      const counts = /* @__PURE__ */ new Map();
      for (const piece of books.pieces) {
        const genre = genreOf(piece, books);
        if (genre) counts.set(genre, (counts.get(genre) ?? 0) + 1);
      }
      const genres = [...books.genres.entries()].filter(([, name]) => !NO_CHART.has(name) && (counts.get(name) ?? 0) >= CHART_MIN).sort((a, b) => (counts.get(b[1]) ?? 0) - (counts.get(a[1]) ?? 0)).map(([id, name]) => ({
        id: `genre:${id}`,
        name: `${name} by Kevin MacLeod`,
        description: "Kevin MacLeod, CC BY 4.0",
        tag: GENRE_TAGS[name] ?? null,
        group: "genre"
      }));
      const moods = CHART_MOODS.map((mood) => ({
        id: `mood:${mood}`,
        name: `${mood} pieces by Kevin MacLeod`,
        description: "Kevin MacLeod, CC BY 4.0",
        tag: mood,
        group: "mood"
      }));
      return {
        charts: [
          {
            id: "well-known",
            name: "Kevin MacLeod: well-known pieces",
            description: "Some of his most used pieces, CC BY 4.0",
            group: "top"
          },
          {
            id: "newest",
            name: "Kevin MacLeod: newest pieces",
            description: "Added to incompetech.com lately, CC BY 4.0",
            group: "new"
          },
          ...genres,
          ...moods
        ]
      };
    },
    async chart({ id, limit }) {
      const books = await catalogue();
      const count = atMost(limit);
      let pieces;
      if (id === "newest") {
        pieces = [...books.pieces].sort(newestFirst);
      } else if (id.startsWith("genre:")) {
        const genre = id.slice("genre:".length);
        pieces = books.pieces.filter((piece) => String(piece.genre ?? "") === genre).sort(byStanding);
      } else if (id.startsWith("mood:")) {
        pieces = books.pieces.filter(inMood([simple(id.slice("mood:".length))])).sort(byStanding);
      } else {
        pieces = books.pieces.filter((piece) => knownRank(piece) < Number.MAX_SAFE_INTEGER).sort(byStanding);
      }
      return { tracks: pieces.slice(0, count).map((piece) => toTrack(piece, books)) };
    },
    async tagSongs({ tag, aliases, kind, limit }) {
      const books = await catalogue();
      const wanted = [tag, ...aliases].map(simple).filter(Boolean);
      let pieces;
      if (kind === "mood") {
        pieces = books.pieces.filter(inMood(wanted));
      } else if (wanted.includes("instrumental")) {
        pieces = [...books.pieces];
      } else {
        pieces = books.pieces.filter(inGenre(wanted, books));
      }
      pieces.sort(byStanding);
      return {
        tracks: pieces.slice(0, atMost(limit, 30)).map((piece) => toTrack(piece, books)),
        name: `${tag} by Kevin MacLeod`
      };
    }
  });
  var streams = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "Incompetech",
    async candidates({ title, artist, limit }) {
      if (!artist || !simple(artist).includes(simple(ARTIST))) return { candidates: [] };
      const wanted = simple(title.replace(/\([^)]*\)|\[[^\]]*\]/g, " "));
      if (!wanted) return { candidates: [] };
      const books = await catalogue();
      const exact = books.pieces.filter((piece) => simple(piece.title) === wanted);
      const matching = exact.length > 0 ? exact : books.pieces.filter((piece) => simple(piece.title).startsWith(`${wanted} `));
      return {
        candidates: matching.slice(0, Math.max(1, limit)).map((piece) => {
          const track = toTrack(piece, books);
          return {
            id: String(track.id),
            title: track.title,
            artist: ARTIST,
            durationMs: track.durationMs ?? null,
            quality: "MP3",
            source: track.source ?? null
          };
        })
      };
    }
  });
  registerProvider(metadata);
  registerProvider(charts);
  registerProvider(streams);
})();
