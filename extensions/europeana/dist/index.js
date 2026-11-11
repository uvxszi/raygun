// dev.raygun.europeana 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var needsSetup = (reason) => {
    const host = raygun;
    if (typeof host.needsSetup === "function") host.needsSetup(reason);
    throw new Error(reason);
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");
  var selectValue = (values, filter) => {
    const chosen = values?.[filter.key];
    const known = (value) => typeof value === "string" && filter.options.some((option) => option.value === value);
    if (known(chosen)) return chosen;
    if (known(filter.default)) return filter.default;
    return filter.options[0]?.value ?? "";
  };

  // src/api.ts
  var API = "https://api.europeana.eu/record/v2";
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var apiKey = () => {
    const key = raygun.settings.get("apiKey")?.trim();
    if (!key) {
      return needsSetup(
        "Europeana needs your own free API key: create one in your europeana.eu account and enter it in this source's settings"
      );
    }
    return key;
  };
  var answers = /* @__PURE__ */ new Map();
  var remember = (address, answer) => {
    answers.delete(address);
    answers.set(address, answer);
    if (answers.size > 60) answers.delete(answers.keys().next().value);
  };
  var europeana = async (path, params) => {
    const key = apiKey();
    const address = `${API}/${path}?${query(params)}`;
    const kept = answers.get(address);
    if (kept !== void 0) return kept;
    const response = await raygun.http.fetch(address, {
      headers: { Accept: "application/json", "X-Api-Key": key, "User-Agent": AGENT }
    });
    if (response.status === 401 || response.status === 403) {
      needsSetup("Europeana didn't accept the API key: check it in this source's settings");
    }
    if (response.status === 404) throw new Error("This item is no longer on Europeana");
    if (response.status === 429) {
      throw new Error("Europeana is limiting the requests of this API key; try again later");
    }
    if (!response.ok) throw new Error(`Europeana answered HTTP ${response.status}`);
    const answer = await response.json();
    if (answer.success === false) {
      throw new Error(`Europeana couldn't answer${answer.error ? `: ${answer.error}` : ""}`);
    }
    remember(address, answer);
    return answer;
  };
  var searchWords = (typed) => typed.replace(/[+\-&|!(){}[\]^"~*?:\\/]/g, " ").replace(/\s+/g, " ").trim();
  var OPEN = /^https?:\/\/creativecommons\.org\/(publicdomain\/(mark|zero)\/1\.0|licenses\/by(-sa)?\/\d\.\d(\/[a-z]{2,3})?)\/?$/i;
  var isOpen = (rights) => Boolean(rights && OPEN.test(rights.trim()));
  var licenceName = (rights) => {
    const marked = /publicdomain\/(mark|zero)\/(\d\.\d)/i.exec(rights);
    if (marked) return marked[1].toLowerCase() === "mark" ? "Public Domain Mark 1.0" : "CC0 1.0";
    const licence = /licenses\/(by(?:-sa)?)\/(\d\.\d)(?:\/([a-z]{2,3}))?/i.exec(rights);
    if (!licence) return rights;
    const port = licence[3] ? ` ${licence[3].toUpperCase()}` : "";
    return `CC ${licence[1].toUpperCase()} ${licence[2]}${port}`;
  };
  var pick = (values, languages) => {
    if (!values) return null;
    if (Array.isArray(values)) return values.find((value) => value.trim())?.trim() ?? null;
    for (const language of [...languages, "def", ...Object.keys(values)]) {
      const found = values[language]?.find((value) => value.trim());
      if (found) return found.trim();
    }
    return null;
  };
  var people = (values, languages) => {
    if (!values) return [];
    const lists = Array.isArray(values) ? [values] : [...languages, "def", ...Object.keys(values)].map((language) => values[language] ?? []);
    const first = lists.find((list) => list.some((value) => !/^https?:\/\//i.test(value))) ?? [];
    return [
      ...new Set(
        first.filter((value) => !/^https?:\/\//i.test(value)).map((value) => value.replace(/\s+/g, " ").trim()).filter(Boolean)
      )
    ];
  };
  var firstYear = (texts) => {
    for (const text of texts) {
      const year = /\b(1[5-9]\d\d|20\d\d)\b/.exec(text ?? "");
      if (year) return Number(year[1]);
    }
    return null;
  };
  var PAGES = /^https?:\/\/([a-z0-9-]+\.)*(vimeo\.com|youtube\.com|youtu\.be|dailymotion\.com)\//i;
  var extensionOf = (address) => (/\.([a-z0-9]{2,5})$/i.exec(address.split(/[?#]/)[0] ?? "")?.[1] ?? "").toLowerCase();
  var isPage = (address) => PAGES.test(address) || /\.(html?|php|aspx?)$/i.test(address.split(/[?#]/)[0] ?? "");
  var isWeb = (address) => typeof address === "string" && /^https?:\/\//i.test(address.trim());
  var plainId = (id) => id.replace(/^\/+/, "");

  // src/music.ts
  var AUDIO = /\.(mp3|m4a|ogg|oga|opus|flac|wav)$/i;
  var trackOf = (item) => {
    const id = item.id ? plainId(item.id) : "";
    const file = item.edmIsShownBy?.find(isWeb);
    if (!id || !file || !AUDIO.test(file.split(/[?#]/)[0] ?? "")) return [];
    if (!isOpen(item.rights?.[0])) return [];
    const languages = [...item.language ?? [], "def"];
    const title = pick(item.dcTitleLangAware, languages) ?? pick(item.title, []);
    if (!title) return [];
    const creators = people(item.dcCreatorLangAware ?? item.dcCreator, languages);
    return [
      {
        id,
        title,
        artist: creators.slice(0, 3).join(", ") || item.dataProvider?.[0]?.trim() || null,
        artworkUrl: item.edmPreview?.find(isWeb) ?? null,
        source: { kind: "url", url: file, headers: {} }
      }
    ];
  };
  var recordings = defineProvider({
    kind: "music.metadata",
    id: "music",
    name: "Europeana",
    async search({ query: query2, limit }) {
      const words = searchWords(query2);
      if (!words) return { tracks: [] };
      const answer = await europeana("search.json", {
        query: words,
        theme: "music",
        qf: ["TYPE:SOUND", "MIME_TYPE:audio/mpeg"],
        reusability: "open",
        media: true,
        rows: Math.min(100, Math.max(10, limit * 2)),
        profile: "minimal"
      });
      const seen = /* @__PURE__ */ new Set();
      const tracks = (answer.items ?? []).flatMap(trackOf).filter((track) => {
        const key = String(track.id);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      return { tracks: tracks.slice(0, Math.max(1, limit)) };
    }
  });

  // src/videos.ts
  var PAGE_SIZE = 40;
  var DEEPEST = 1e3;
  var LANGUAGES = ["en"];
  var makers = (names) => names.filter((name) => !/\((curator|beheerder)\)$/i.test(name));
  var COUNTRY = {
    kind: "select",
    key: "country",
    label: "Country",
    options: [
      { value: "any", label: "Any country" },
      ...[
        "Netherlands",
        "Germany",
        "Romania",
        "Sweden",
        "Hungary",
        "Spain",
        "Finland",
        "Poland",
        "Greece",
        "Ireland",
        "France"
      ].map((country) => ({ value: country.toLowerCase(), label: country }))
    ],
    default: "any"
  };
  var PROVIDER = {
    kind: "select",
    key: "provider",
    label: "Institution",
    options: [
      { value: "any", label: "Any institution" },
      // The institutions with the most open films that have an MP4.
      ...[
        "Netherlands Institute for Sound & Vision",
        "Naturalis Biodiversity Center",
        "Romanian Television",
        "Institut des Hautes Études Scientifiques",
        "Ruhr University Bochum",
        "Federal Archives of Germany",
        "Museum of Ethnography",
        "Csorba Győző Library - Pécs",
        "Federal Waterways Engineering and Research Institute",
        "German Informatics Society"
      ].map((name) => ({ value: name, label: name }))
    ],
    default: "any"
  };
  var SORT = {
    kind: "select",
    key: "sort",
    label: "Sort",
    options: [
      { value: "relevance", label: "Relevance" },
      { value: "added", label: "Recently added" }
    ],
    default: "relevance"
  };
  var VIDEOS = ["TYPE:VIDEO", "MIME_TYPE:video/mp4"];
  var summaryOf = (item) => {
    const id = item.id ? plainId(item.id) : "";
    const title = pick(item.dcTitleLangAware, [...LANGUAGES, ...item.language ?? []]) ?? pick(item.title, []);
    const media = item.edmIsShownBy?.find(isWeb);
    if (!id || !title || !isOpen(item.rights?.[0]) || !media || isPage(media)) return [];
    const languages = [...LANGUAGES, ...item.language ?? []];
    return [
      {
        id,
        title,
        coverUrl: item.edmPreview?.find(isWeb) ?? null,
        authors: makers(people(item.dcCreatorLangAware ?? item.dcCreator, languages)).slice(0, 3)
      }
    ];
  };
  var listPage = async (page, { words = "", refinements = [], sort = "" } = {}) => {
    const asked = Math.max(1, page);
    const start = (asked - 1) * PAGE_SIZE + 1;
    if (start + PAGE_SIZE - 1 > DEEPEST) return { items: [], hasNextPage: false };
    const answer = await europeana("search.json", {
      query: words || "*",
      qf: [...VIDEOS, ...refinements],
      reusability: "open",
      media: true,
      rows: PAGE_SIZE,
      start,
      profile: "minimal",
      ...sort ? { sort } : {}
    });
    const items = answer.items ?? [];
    const seen = /* @__PURE__ */ new Set();
    return {
      items: items.flatMap(summaryOf).filter((item) => {
        if (seen.has(item.id)) return false;
        seen.add(item.id);
        return true;
      }),
      hasNextPage: items.length > 0 && asked * PAGE_SIZE < Math.min(answer.totalResults ?? 0, DEEPEST)
    };
  };
  var record = async (id) => {
    const answer = await europeana(
      `${plainId(id).split("/").map(encodeURIComponent).join("/")}.json`,
      {}
    );
    const object = answer.object;
    const aggregation = object?.aggregations?.[0];
    if (!object || !aggregation) throw new Error("This item is no longer on Europeana");
    const rights = pick(aggregation.edmRights, []);
    if (!isOpen(rights)) {
      throw new Error("This item isn't under an open licence, so it isn't offered");
    }
    return { object, aggregation, rights };
  };
  var organisation = (value, organizations) => {
    const named = pick(value, []);
    if (!named) return null;
    const known = organizations?.find((found) => found.about === named);
    return known ? pick(known.prefLabel, LANGUAGES) : /^https?:\/\//.test(named) ? null : named;
  };
  var PLAYABLE = /^video\/(mp4|x-m4v|webm)$/i;
  var filesOf = (aggregation) => {
    const resources = new Map(
      (aggregation.webResources ?? []).map((resource) => [resource.about ?? "", resource])
    );
    const links = [aggregation.edmIsShownBy, ...aggregation.hasView ?? []].filter(isWeb);
    return [...new Set(links)].flatMap((link) => {
      const resource = resources.get(link);
      const mime = resource?.ebucoreHasMimeType;
      const playable = mime ? PLAYABLE.test(mime) : ["mp4", "m4v", "webm"].includes(extensionOf(link));
      const ownRights = pick(resource?.webResourceEdmRights, []);
      if (!playable || isPage(link) || ownRights && !isOpen(ownRights)) return [];
      return [{ url: link, height: resource?.ebucoreHeight ?? null }];
    });
  };
  var providerProxies = (proxies) => (proxies ?? []).filter((proxy) => !proxy.europeanaProxy);
  var fromProxies = (proxies, field, languages) => {
    for (const proxy of proxies) {
      const found = pick(proxy[field], languages);
      if (found) return found;
    }
    return null;
  };
  var allOf = (proxies, field, languages) => {
    for (const proxy of proxies) {
      const map = proxy[field];
      if (!map) continue;
      for (const language of [...languages, "def", ...Object.keys(map)]) {
        const values = (map[language] ?? []).map((value) => value.trim()).filter(Boolean);
        if (values.length > 0) return [...new Set(values)].join("\n\n");
      }
    }
    return "";
  };
  var episodeIdOf = (id, index) => `${id}#${index}`;
  var videosOf = async (episodeId) => {
    const cut = episodeId.lastIndexOf("#");
    const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
    const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
    const file = filesOf((await record(id)).aggregation)[index];
    if (!file) throw new Error("This video is no longer on Europeana");
    return [
      {
        url: file.url,
        label: file.height ? `${file.height}p` : "Video",
        ...file.height ? { quality: file.height } : {}
      }
    ];
  };
  var videos = defineProvider({
    kind: "video.source",
    id: "videos",
    name: "Europeana",
    // Europeana's own order for everything open with an MP4 (it has no counts of views).
    popular: ({ page }) => listPage(page),
    latest: ({ page }) => listPage(page, { sort: "timestamp_created desc" }),
    search({ query: typed, page, filters }) {
      const country = selectValue(filters, COUNTRY);
      const provider = selectValue(filters, PROVIDER);
      const sort = selectValue(filters, SORT);
      const refinements = [];
      if (country !== "any") refinements.push(`COUNTRY:${country}`);
      if (provider !== "any") refinements.push(`DATA_PROVIDER:"${provider}"`);
      return listPage(page, {
        words: searchWords(typed),
        refinements,
        sort: sort === "added" ? "timestamp_created desc" : ""
      });
    },
    filters: async () => [COUNTRY, PROVIDER, SORT],
    async details({ id }) {
      const { object, aggregation, rights } = await record(id);
      const proxies = providerProxies(object.proxies);
      const languages = [
        ...LANGUAGES,
        ...proxies.flatMap((proxy) => proxy.dcLanguage?.def ?? []).map((tag) => tag.toLowerCase())
      ];
      const holder = organisation(aggregation.edmDataProvider, object.organizations);
      const through = organisation(aggregation.edmProvider, object.organizations);
      const creators = makers(proxies.flatMap((proxy) => people(proxy.dcCreator, languages)));
      const credit = holder ? `Provided by ${holder}${through && through !== holder ? ` through ${through}` : ""} and Europeana.` : "Provided through Europeana.";
      return {
        id,
        title: fromProxies(proxies, "dcTitle", languages) ?? id,
        coverUrl: object.europeanaAggregation?.edmPreview ?? null,
        description: [
          allOf(proxies, "dcDescription", languages),
          `Licence: ${licenceName(rights)} (${rights}).`,
          credit
        ].filter(Boolean).join("\n\n"),
        genres: [
          ...new Set(
            (object.concepts ?? []).map((concept) => pick(concept.prefLabel, LANGUAGES)).filter((label) => Boolean(label))
          )
        ].slice(0, 6),
        directors: [...new Set(creators)].slice(0, 4),
        type: "other",
        year: firstYear([
          ...proxies.flatMap((proxy) => [
            fromProxies([proxy], "dctermsCreated", []),
            fromProxies([proxy], "dcDate", []),
            fromProxies([proxy], "dctermsIssued", []),
            fromProxies([proxy], "year", [])
          ]),
          ...object.year ?? []
        ]),
        status: "completed",
        url: object.europeanaAggregation?.edmLandingPage ?? `https://www.europeana.eu/item/${id}`
      };
    },
    async episodes({ id }) {
      const files = filesOf((await record(id)).aggregation);
      if (files.length === 0) throw new Error("Europeana links no video file of this item");
      if (files.length === 1) return { episodes: [{ id: episodeIdOf(id, 0), name: "Film" }] };
      return {
        episodes: files.map((_, index) => ({
          id: episodeIdOf(id, index),
          name: `Part ${index + 1}`,
          number: index + 1
        }))
      };
    },
    // One server, resolved at once: the institution's file, as the record links it.
    servers: async ({ episodeId }) => ({
      servers: [{ id: episodeId, name: "Europeana", videos: await videosOf(episodeId) }]
    }),
    videos: async ({ serverId }) => ({ videos: await videosOf(serverId) })
  });

  // src/index.ts
  registerProvider(videos);
  registerProvider(recordings);
})();
