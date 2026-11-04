// dev.raygun.blender-movies 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var COMMONS = "https://commons.wikimedia.org";
  var API = `${COMMONS}/w/api.php`;
  var AGENT = "Raygun/1.0 (https://github.com/uvxszi/raygun)";
  var MEDIA_HEADERS = { "User-Agent": AGENT };
  var API_HEADERS = { "User-Agent": AGENT, "Api-User-Agent": AGENT };
  var FILMS = [
    { id: "big-buck-bunny", title: "Big Buck Bunny", year: 2008, file: "Big Buck Bunny 4K.webm" },
    { id: "sintel", title: "Sintel", year: 2010, file: "Sintel movie 4K.webm" },
    {
      id: "tears-of-steel",
      title: "Tears of Steel",
      year: 2012,
      file: "Tears of Steel in 4k - Official Blender Foundation release.webm"
    },
    {
      id: "elephants-dream",
      title: "Elephants Dream",
      year: 2006,
      file: "Elephants Dream (2006) 1080p24.webm"
    },
    {
      id: "cosmos-laundromat",
      title: "Cosmos Laundromat: First Cycle",
      year: 2015,
      file: "Cosmos Laundromat - First Cycle - Official Blender Foundation release.webm"
    },
    { id: "spring", title: "Spring", year: 2019, file: "Spring - Blender Open Movie.webm" },
    {
      id: "sprite-fright",
      title: "Sprite Fright",
      year: 2021,
      file: "Sprite Fright - Blender Open Movie-full movie.webm"
    },
    {
      id: "coffee-run",
      title: "Coffee Run",
      year: 2020,
      file: "Coffee Run - Blender Open Movie-full movie.webm"
    },
    {
      id: "charge",
      title: "Charge",
      year: 2022,
      file: "Charge - Blender Open Movie-full movie.webm"
    },
    {
      id: "wing-it",
      title: "Wing It!",
      year: 2023,
      file: "WING IT! - Blender Open Movie-full movie.webm"
    },
    {
      id: "singularity",
      title: "Singularity",
      year: 2026,
      file: "SINGULARITY - Blender Open Movie-full movie.webm"
    },
    { id: "hero", title: "Hero", year: 2018, file: "HERO - Blender Open Movie-full movie.webm" },
    {
      id: "caminandes-llama-drama",
      title: "Caminandes: Llama Drama",
      year: 2013,
      file: "Caminandes- Llama Drama - Short Movie.ogv"
    },
    {
      id: "caminandes-gran-dillama",
      title: "Caminandes: Gran Dillama",
      year: 2013,
      file: "Caminandes - Gran Dillama - Blender Foundation's new Open Movie.webm"
    },
    {
      id: "caminandes-llamigos",
      title: "Caminandes 3: Llamigos",
      year: 2016,
      file: "Caminandes 3 - Llamigos - Blender Animated Short.webm"
    },
    {
      id: "glass-half",
      title: "Glass Half",
      year: 2015,
      file: "Glass Half - Blender Open Movie-full movie.webm"
    },
    {
      id: "the-daily-dweebs",
      title: "The Daily Dweebs",
      year: 2017,
      file: "The Daily Dweebs - Blender Open Movie-full movie.webm"
    }
  ];
  var filmById = (id) => {
    const film = FILMS.find((known) => known.id === id);
    if (!film) throw new Error("Not one of the Blender open movies");
    return film;
  };
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<[^>]+>/g, "").replace(/&nbsp;|&#160;/g, " ").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/[^\S\n]+/g, " ").trim();
  var field = (info, key) => {
    const value = info.extmetadata?.[key]?.value;
    return value === void 0 ? "" : plainText(String(value));
  };
  var creditOf = (info) => (field(info, "Artist").split("\n")[0] ?? "").split(/;|\.\s/)[0].replace(/^(credit|author|by|produced by|directed by)\s*:?\s*/i, "").trim();
  var clean = (url) => url ? url.replace(/[?&]utm_[^#]*$/, "") : void 0;
  var files = null;
  var filesOf = () => {
    if (!files) {
      const titles = FILMS.map((film) => `File:${film.file}`).join("|");
      const params = {
        action: "query",
        format: "json",
        formatversion: 2,
        titles,
        prop: "videoinfo",
        viprop: "url|mime|size|extmetadata|derivatives|timedtext",
        viurlwidth: 640,
        viextmetadatafilter: "ImageDescription|Artist|LicenseShortName",
        viextmetadatamultilang: 0
      };
      const query = Object.entries(params).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
      files = raygun.http.fetch(`${API}?${query}`, { headers: API_HEADERS }).then(async (response) => {
        if (!response.ok) throw new Error(`Wikimedia Commons answered HTTP ${response.status}`);
        const answer = await response.json();
        const asked = new Map((answer.query?.normalized ?? []).map((name) => [name.to, name.from]));
        const byFile = /* @__PURE__ */ new Map();
        for (const page of answer.query?.pages ?? []) {
          const info = page.videoinfo?.[0];
          if (page.missing || !info) continue;
          byFile.set((asked.get(page.title) ?? page.title).replace(/^File:/, ""), info);
        }
        return byFile;
      });
      files.catch(() => {
        files = null;
      });
    }
    return files;
  };
  var infoOf = async (film) => {
    const info = (await filesOf()).get(film.file);
    if (!info) throw new Error(`${film.title} is no longer on Wikimedia Commons`);
    return info;
  };
  var listing = async (films) => {
    const known = await filesOf().catch(() => /* @__PURE__ */ new Map());
    const items = films.map((film) => ({
      id: film.id,
      title: film.title,
      coverUrl: clean(known.get(film.file)?.thumburl) ?? null
    }));
    return { items, hasNextPage: false };
  };
  var nominalHeight = (key) => Number.parseInt(key ?? "", 10) || void 0;
  var videosOf = (info) => {
    const subtitles = (info.timedtext ?? []).filter((track) => track.src && track.kind !== "metadata").map((track) => ({ url: track.src, lang: track.srclang, label: track.label }));
    const videos = (info.derivatives ?? []).filter((copy) => /^video\/webm/.test(copy.type ?? "") && /vp9|vp8/i.test(copy.type ?? "")).map((copy) => {
      const height = nominalHeight(copy.transcodekey) ?? copy.height;
      return {
        url: copy.src,
        label: height ? `${height}p · VP9` : "VP9",
        quality: height,
        headers: MEDIA_HEADERS,
        subtitles
      };
    });
    const original = clean(info.url);
    if (original && info.mime === "video/webm") {
      videos.push({
        url: original,
        label: info.height ? `Original · ${info.width}×${info.height}` : "Original",
        quality: info.height || void 0,
        headers: MEDIA_HEADERS,
        subtitles
      });
    }
    return videos.sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
  };
  var server = async (id) => {
    const film = filmById(id);
    const videos = videosOf(await infoOf(film));
    if (videos.length === 0)
      throw new Error(`Commons has no copy of ${film.title} that Raygun can play`);
    return { id, name: "Wikimedia Commons", videos };
  };
  var ACCENTS = new RegExp(`[${String.fromCharCode(768)}-${String.fromCharCode(879)}]`, "g");
  var words = (text) => text.toLowerCase().normalize("NFD").replace(ACCENTS, "").split(/[^a-z0-9]+/).filter(Boolean);
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "movies",
      name: "Blender open movies",
      popular: async ({ page }) => page > 1 ? { items: [], hasNextPage: false } : listing(FILMS),
      latest: async ({ page }) => page > 1 ? { items: [], hasNextPage: false } : listing([...FILMS].sort((a, b) => b.year - a.year)),
      async search({ query, page }) {
        if (page > 1) return { items: [], hasNextPage: false };
        const wanted = words(query);
        return listing(
          FILMS.filter((film) => {
            const offered = [...words(film.title), "blender", String(film.year)];
            return wanted.every((word) => offered.some((have) => have.startsWith(word)));
          })
        );
      },
      async details({ id }) {
        const film = filmById(id);
        const info = await infoOf(film);
        const about = field(info, "ImageDescription");
        const licence = field(info, "LicenseShortName");
        const credit = creditOf(info);
        return {
          id,
          title: film.title,
          coverUrl: clean(info.thumburl) ?? null,
          description: [
            about,
            [licence && `Licence: ${licence}`, credit && `By ${credit}`].filter(Boolean).join(" · ")
          ].filter(Boolean).join("\n\n") || null,
          // Tears of Steel is the one live-action film among them.
          genres: id === "tears-of-steel" ? ["Science fiction", "Short film"] : ["Animation", "Short film"],
          directors: [],
          type: "movie",
          year: film.year,
          status: "completed",
          url: `${COMMONS}/wiki/File:${encodeURIComponent(film.file.replace(/ /g, "_"))}`
        };
      },
      episodes: async ({ id }) => ({ episodes: [{ id: filmById(id).id, name: "Film" }] }),
      servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),
      videos: async ({ serverId }) => ({ videos: (await server(serverId)).videos })
    })
  );
})();
