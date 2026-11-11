// dev.raygun.loc 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
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
  var SITE = "https://www.loc.gov";
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var GAP_MS = 3100;
  var BLOCK_MS = 60 * 60 * 1e3;
  var BLOCKED_KEY = "blockedUntil";
  var Lru = class {
    constructor(size) {
      this.size = size;
    }
    size;
    entries = /* @__PURE__ */ new Map();
    get(key) {
      const value = this.entries.get(key);
      if (value !== void 0) {
        this.entries.delete(key);
        this.entries.set(key, value);
      }
      return value;
    }
    set(key, value) {
      this.entries.delete(key);
      this.entries.set(key, value);
      while (this.entries.size > this.size) {
        const oldest = this.entries.keys().next().value;
        if (oldest === void 0) break;
        this.entries.delete(oldest);
      }
    }
  };
  var pause = async (ms) => {
    let left = ms;
    while (left > 0) {
      const step = Math.min(left, 1e4);
      await raygun.time.sleep(step);
      left -= step;
    }
  };
  var lastStart = 0;
  var line = Promise.resolve();
  var inTurn = (request) => {
    const turn = line.then(async () => {
      const wait = lastStart + GAP_MS - Date.now();
      if (wait > 0) await pause(wait);
      lastStart = Date.now();
      return request();
    });
    line = turn.catch(() => void 0);
    return turn;
  };
  var blockedUntil = () => {
    const until = raygun.storage.get(BLOCKED_KEY);
    return typeof until === "number" && until > Date.now() ? until : 0;
  };
  var blockedError = (until) => {
    const minutes = Math.max(1, Math.ceil((until - Date.now()) / 6e4));
    return new Error(
      `The Library of Congress asked Raygun to slow down; it can be asked again in ${minutes} minute${minutes === 1 ? "" : "s"}`
    );
  };
  var once = async (url) => {
    let response;
    try {
      response = await raygun.http.fetch(url, {
        headers: { Accept: "application/json", "User-Agent": AGENT }
      });
    } catch (error) {
      return { retry: String(error) };
    }
    const type = response.headers["content-type"] ?? "";
    if (response.status === 429) return { blocked: true };
    if (response.status === 404) return { missing: true };
    if (response.status >= 500) return { retry: `HTTP ${response.status}` };
    if (type.includes("text/html")) return { blocked: true };
    if (!response.ok) throw new Error(`The Library of Congress answered HTTP ${response.status}`);
    try {
      return { answer: await response.json() };
    } catch (error) {
      return { retry: String(error) };
    }
  };
  var answers = new Lru(16);
  var asking = /* @__PURE__ */ new Map();
  var STALL_RETRY_MS = 3e4;
  var STALLED_MS = 15e3;
  var ask = async (url) => {
    let firstStart = 0;
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const until = blockedUntil();
      if (until) throw blockedError(until);
      if (attempt > 0 && Date.now() - firstStart >= STALLED_MS) {
        await pause(firstStart + STALL_RETRY_MS - Date.now());
      }
      const outcome = await inTurn(() => {
        if (attempt === 0) firstStart = Date.now();
        return once(url);
      });
      if ("answer" in outcome) {
        answers.set(url, outcome.answer);
        return outcome.answer;
      }
      if ("missing" in outcome) return null;
      if ("blocked" in outcome) {
        const end = Date.now() + BLOCK_MS;
        raygun.storage.set(BLOCKED_KEY, end);
        throw blockedError(end);
      }
      raygun.log.warn(`loc.gov didn't answer ${url}: ${outcome.retry}`);
    }
    throw new Error("The Library of Congress is slow to answer; try again");
  };
  var locJson = async (path) => {
    const url = `${SITE}${path}${path.includes("?") ? "&" : "?"}fo=json`;
    const kept2 = answers.get(url);
    if (kept2 !== void 0) return kept2;
    let request = asking.get(url);
    if (!request) {
      request = ask(url).finally(() => asking.delete(url));
      asking.set(url, request);
    }
    return request;
  };
  var thisYear = () => (/* @__PURE__ */ new Date()).getUTCFullYear();
  var filmCutoff = () => thisYear() - 96;
  var recordingCutoff = () => thisYear() - 101;
  var latestYear = (date) => {
    const years = (date ?? "").match(/\b1[89]\d[\d?]|\b20\d[\d?]/g) ?? [];
    const numbers = years.map((year) => Number(year.replace("?", "9")));
    return numbers.length > 0 ? Math.max(...numbers) : null;
  };
  var firstYear = (date) => {
    const year = /\b(1[89]\d\d|20\d\d)\b/.exec(date ?? "");
    return year ? Number(year[1]) : null;
  };
  var jukebox = (partof) => (partof ?? []).some(
    (part) => /national jukebox/i.test(typeof part === "string" ? part : part.title ?? "")
  );
  var itemId = (listed) => {
    const match = /\/item\/([^/?#]+)\/?/.exec(listed.url ?? listed.id ?? "");
    return match?.[1] ?? null;
  };
  var https = (address) => {
    if (!address) return null;
    if (address.startsWith("//")) return `https:${address}`;
    return /^https?:\/\//i.test(address) ? address.replace(/^http:/i, "https:") : null;
  };
  var text = (value) => (Array.isArray(value) ? value.join(" ") : value ?? "").replace(/<[^>]+>/g, " ").replace(/&quot;/g, '"').replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
  var FREE = /not aware of any u\.?\s?s\.? copyright|no known (copyright )?restrictions|(is|are) in the public domain|free to use and reuse/i;
  var RESTRICTED = /rights status not evaluated|copyright(ed)? (protected|restricted|held)|may be (protected|restricted)|permission (is )?required|restricted access|for educational (purposes|use) only/i;
  var freeToUse = (item) => {
    const advisory = text(item?.rights_advisory);
    if (advisory && RESTRICTED.test(advisory)) return false;
    return FREE.test(advisory) || FREE.test(text(item?.rights));
  };
  var creditLine = (item) => {
    const rights = item?.rights;
    const html = Array.isArray(rights) ? rights.join("\n") : rights ?? "";
    for (const paragraph of html.split(/<\/p>|<br\s*\/?>|\r?\n/i)) {
      const credit = /credit line:\s*([^.]+)/i.exec(text(paragraph));
      if (credit?.[1]) return credit[1].trim();
    }
    return null;
  };
  var BRACKETED_ROLE = /\s*\((performer|composer|conductor|singer|speaker|lyricist|arranger|narrator)\)\s*$/i;
  var CODES = {
    prf: "performer",
    spk: "speaker",
    sng: "singer",
    voc: "singer",
    itr: "performer",
    lyr: "lyricist",
    cnd: "conductor",
    cmp: "composer",
    arr: "arranger",
    aut: "author",
    drt: "director",
    pro: "producer"
  };
  var CORPORATE = /^(inc|co|corp|ltd|llc|company|firm|bros|brothers|limited|incorporated|and company|& co)\b/i;
  var KNOWN_ROLE = /(?:,\s*|\s+)(former owner|production company|film distributor|copyright claimant|camera|cinematographer|director|producer|actor|performer|animator|scenarist|writer|writing|editor|narrator|composer|singer|speaker|lyricist|conductor|arranger|donor)\.?$/;
  var LOWER_CASE = /^[a-z][a-z .&'/-]*$/;
  var LIFE_DATES = /^(?:ca\.\s*)?(?:\d{3,4}\??)?-?(?:\d{3,4}\??)?$/;
  var contributor = (raw) => {
    let rest = raw.trim();
    const roles = [];
    const bracketed = BRACKETED_ROLE.exec(rest);
    if (bracketed) {
      roles.push(bracketed[1].toLowerCase());
      rest = rest.slice(0, bracketed.index);
    }
    const code = /(?:\.\s*|\s+)([a-z]{3})\.?$/.exec(rest);
    if (code && CODES[code[1]]) {
      roles.unshift(CODES[code[1]]);
      rest = rest.slice(0, code.index);
      if (/(?:^|[\s,])[A-Z]$/.test(rest)) rest += ".";
    }
    rest = rest.replace(/\s*\([^)]*\)/g, "").trim();
    for (let role = KNOWN_ROLE.exec(rest); role; role = KNOWN_ROLE.exec(rest)) {
      roles.unshift(role[1]);
      rest = rest.slice(0, role.index).trim();
    }
    const parts2 = rest.split(/\s*,\s*/).map((part) => part.trim()).filter(Boolean);
    while (parts2.length > 1 && LOWER_CASE.test(parts2[parts2.length - 1])) {
      roles.unshift(parts2.pop());
    }
    const named = parts2.filter((part) => !(/\d/.test(part) && LIFE_DATES.test(part)));
    let name = named.join(", ");
    if (named.length >= 2 && !CORPORATE.test(named[1])) {
      name = [`${named[1]} ${named[0]}`, ...named.slice(2)].join(", ");
    }
    if (name.endsWith(".") && !/\b[A-Z]\.$/.test(name)) name = name.slice(0, -1);
    return { name: name.replace(/\s+/g, " ").trim(), role: roles[0] ?? null };
  };
  var madeIt = ({ name, role }) => name !== "" && role !== "former owner" && role !== "donor" && !/collection|library of congress/i.test(name);
  var plain = text;

  // src/films.ts
  var PAGE_SIZE = 50;
  var EDISON = "edison-company-motion-pictures-and-sound-recordings";
  var COLLECTION = {
    kind: "select",
    key: "collection",
    label: "Collection",
    options: [
      { value: "all", label: "All films" },
      { value: EDISON, label: "Edison films" },
      { value: "america-at-work-and-leisure-1894-to-1915", label: "America at Work, at Leisure" },
      { value: "spanish-american-war-in-motion-pictures", label: "Spanish-American War" },
      { value: "mckinley-and-the-pan-american-expo-films-1901", label: "McKinley, 1901" },
      { value: "theodore-roosevelt-films", label: "Theodore Roosevelt" },
      { value: "films-of-westinghouse-works-1904", label: "Westinghouse Works, 1904" },
      { value: "early-films-of-new-york-1898-to-1906", label: "Early New York" },
      { value: "san-francisco-earthquake-and-fire-1897-to-1916", label: "Early San Francisco" },
      { value: "origins-of-american-animation", label: "Early animation" },
      { value: "variety-stage-sound-recordings-and-motion-pictures", label: "Variety stage" },
      { value: "national-screening-room", label: "National Screening Room" }
    ],
    default: "all"
  };
  var DECADE = {
    kind: "select",
    key: "decade",
    label: "Decade",
    options: [
      { value: "any", label: "Any time" },
      { value: "1880", label: "Before 1900" },
      { value: "1900", label: "1900s" },
      { value: "1910", label: "1910s" },
      { value: "1920", label: "1920s" }
    ],
    default: "any"
  };
  var SORT = {
    kind: "select",
    key: "sort",
    label: "Sort",
    options: [
      { value: "relevance", label: "Relevance" },
      { value: "date", label: "Oldest first" },
      { value: "date_desc", label: "Newest first" },
      { value: "title_s", label: "Title" }
    ],
    default: "relevance"
  };
  var datesFor = (decade) => {
    const cutoff = filmCutoff();
    if (decade === "any") return `1800/${cutoff}`;
    const start = Number(decade);
    const end = Math.min(start === 1880 ? 1899 : start + 9, cutoff);
    return `${start}/${end}`;
  };
  var videoOf = (resource) => resource?.video ?? resource?.video_stream;
  var publicFilm = (listed) => {
    const year = latestYear(listed.date);
    return year !== null && year <= filmCutoff() && listed.access_restricted !== true && !jukebox(listed.partof) && (listed.resources ?? []).some((resource) => Boolean(videoOf(resource)));
  };
  var posterOf = (resources, pictures) => {
    const resource = (resources ?? []).find((found) => videoOf(found));
    return https(resource?.poster) ?? https(resource?.image) ?? https(pictures?.[0]);
  };
  var makersOf = (listed) => [
    ...new Set(
      (listed.item?.contributors ?? []).filter((name) => typeof name === "string").map(contributor).filter(madeIt).map((person) => person.name)
    )
  ];
  var summaryOf = (listed) => {
    const id = itemId(listed);
    if (!id || !listed.title || !publicFilm(listed)) return [];
    return [
      {
        id,
        title: listed.title.trim(),
        coverUrl: posterOf(listed.resources, listed.image_url),
        authors: makersOf(listed).slice(0, 3)
      }
    ];
  };
  var pages = new Lru(60);
  var listPage = async (path, params) => {
    const address = `${path}?${query({
      ...params,
      fa: "online-format:video",
      at: "results,pagination",
      c: PAGE_SIZE
    })}`;
    const kept2 = pages.get(address);
    if (kept2) return kept2;
    const answer = await locJson(address);
    const results = answer?.results ?? [];
    const found = /* @__PURE__ */ new Map();
    for (const item of results.flatMap(summaryOf)) {
      if (!found.has(item.id)) found.set(item.id, item);
    }
    const page = {
      items: [...found.values()],
      hasNextPage: results.length > 0 && Boolean(answer?.pagination?.next)
    };
    pages.set(address, page);
    return page;
  };
  var items = new Lru(300);
  var film = async (id) => {
    const kept2 = items.get(id);
    if (kept2) return kept2;
    const answer = await locJson(
      `/item/${encodeURIComponent(id)}/?${query({ at: "item,resources" })}`
    );
    if (!answer?.item) throw new Error("This film is no longer at the Library of Congress");
    const year = latestYear(answer.item.date);
    if (year === null || year > filmCutoff() || jukebox(answer.item.partof)) {
      throw new Error("This film isn't in the public domain yet, so it isn't offered");
    }
    if (answer.item.access_restricted === true || !freeToUse(answer.item)) {
      throw new Error("This item has rights restrictions, so it isn't offered");
    }
    items.set(id, answer);
    return answer;
  };
  var parts = (answer) => (answer.resources ?? []).map((resource, index) => ({ resource, index })).filter(({ resource }) => videoOf(resource));
  var people = (answer) => (answer.item?.contributor_names ?? []).filter((name) => typeof name === "string").map(contributor).filter(madeIt);
  var DIRECTING = /camera|cinematograph|director|animator|scenarist|writ/;
  var ACTING = /actor|performer|cast|singer/;
  var videosOf = (resource) => {
    const videos = [];
    const mp4 = https(resource.video);
    const stream = https(resource.video_stream);
    if (mp4) {
      videos.push({
        url: mp4,
        label: resource.height ? `MP4 · ${resource.height}p` : "MP4",
        ...resource.height ? { quality: resource.height } : {}
      });
    }
    if (stream) videos.push({ url: stream, label: "Adaptive stream", format: "hls" });
    return videos;
  };
  var episodeIdOf = (id, index) => `${id}#${index}`;
  var resourceOf = async (episodeId) => {
    const cut = episodeId.lastIndexOf("#");
    const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
    const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
    const resource = (await film(id)).resources?.[index];
    if (!resource || !videoOf(resource)) {
      throw new Error("The Library of Congress no longer has this part of the film");
    }
    return resource;
  };
  var films = defineProvider({
    kind: "video.source",
    id: "films",
    name: "Library of Congress",
    // Edison's films: the Library's best-known early films, in its own order.
    popular: ({ page }) => listPage(`/collections/${EDISON}/`, { dates: datesFor("any"), sp: Math.max(1, page) }),
    search({ query: words, page, filters }) {
      const collection = selectValue(filters, COLLECTION);
      const decade = selectValue(filters, DECADE);
      const sort = selectValue(filters, SORT);
      const params = {
        dates: datesFor(decade),
        sp: Math.max(1, page)
      };
      const typed = words.trim();
      if (typed) params.q = typed;
      if (sort !== "relevance") params.sb = sort;
      const path = collection === "all" ? "/film-and-videos/" : `/collections/${collection}/`;
      return listPage(path, params);
    },
    filters: async () => [COLLECTION, DECADE, SORT],
    async details({ id }) {
      const answer = await film(id);
      const item = answer.item;
      const made = people(answer);
      const makers = made.filter((person) => !person.role || !ACTING.test(person.role));
      const credit = creditLine(item);
      const summary = plain(item.summary ?? item.description);
      const published = plain(item.created_published);
      return {
        id,
        title: item.title?.trim() || id,
        coverUrl: posterOf(answer.resources),
        description: [
          summary,
          published ? `Published: ${published}` : "",
          makers.length > 0 ? `Made by: ${makers.map((person) => person.role ? `${person.name} (${person.role})` : person.name).join(", ")}` : "",
          `Public domain in the United States.${credit ? ` Credit: ${credit}.` : ""}`
        ].filter(Boolean).join("\n\n") || null,
        genres: [...new Set(item.genre ?? [])].slice(0, 8),
        directors: made.filter((person) => person.role && DIRECTING.test(person.role)).map((person) => person.name),
        cast: made.filter((person) => person.role && ACTING.test(person.role)).map((person) => person.name),
        type: "movie",
        year: firstYear(item.date),
        status: "completed",
        url: `https://www.loc.gov/item/${encodeURIComponent(id)}/`
      };
    },
    async episodes({ id }) {
      const found = parts(await film(id));
      if (found.length === 0) throw new Error("The Library of Congress has no video of this film");
      if (found.length === 1)
        return { episodes: [{ id: episodeIdOf(id, found[0].index), name: "Film" }] };
      return {
        episodes: found.map(({ resource, index }, number) => ({
          id: episodeIdOf(id, index),
          name: resource.caption?.trim() || `Part ${number + 1}`,
          number: number + 1
        }))
      };
    },
    // One server, resolved at once: the Library serves the files itself.
    servers: async ({ episodeId }) => ({
      servers: [
        { id: episodeId, name: "Library of Congress", videos: videosOf(await resourceOf(episodeId)) }
      ]
    }),
    videos: async ({ serverId }) => ({ videos: videosOf(await resourceOf(serverId)) })
  });

  // src/recordings.ts
  var COLLECTIONS = [
    {
      id: "edison",
      slug: "edison-company-motion-pictures-and-sound-recordings",
      title: "Inventing Entertainment: Edison sound recordings",
      words: "edison inventing entertainment diamond disc records"
    },
    {
      id: "variety",
      slug: "variety-stage-sound-recordings-and-motion-pictures",
      title: "Variety Stage sound recordings",
      words: "variety stage vaudeville records"
    }
  ];
  var KEEP_MS = 7 * 24 * 60 * 60 * 1e3;
  var STORE_KEY = "recordings";
  var STORE_VERSION = 1;
  var folded = (value) => value.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  var wordsOf = (value) => folded(value).split(/[^a-z0-9]+/).filter(Boolean);
  var sideTitle = (caption) => {
    const title = (caption ?? "").replace(/\s*\((?:edison|victor|columbia|berliner)\b[^)]*\)\s*$/i, "").trim();
    return /digital audio|^online\b/i.test(title) ? "" : title;
  };
  var capitalized = (title) => title.charAt(0).toUpperCase() + title.slice(1);
  var artistOf = (listed) => {
    const people2 = (listed.item?.contributors ?? []).filter((name) => typeof name === "string").map(contributor).filter(madeIt);
    const heard = people2.filter(
      (person) => person.role && /perform|speak|sing|conduct/.test(person.role)
    );
    const credited = heard.length > 0 ? heard : people2.slice(0, 1);
    const names = [...new Set(credited.map((person) => person.name))];
    return names.length > 0 ? names.slice(0, 3).join(" & ") : null;
  };
  var recordingsOf = (listed, album) => {
    const id = itemId(listed);
    const year = latestYear(listed.date);
    if (!id || year === null || year > recordingCutoff()) return [];
    if (listed.access_restricted === true || jukebox(listed.partof)) return [];
    const sides = (listed.resources ?? []).filter((resource) => https(resource.audio));
    return sides.flatMap((resource, index) => {
      const title = sideTitle(resource.caption) || (sides.length === 1 ? listed.title?.trim() : "");
      if (!title) return [];
      return [
        {
          id: `${id}#${index}`,
          title: capitalized(title),
          artist: artistOf(listed),
          year: firstYear(listed.date),
          url: https(resource.audio),
          album,
          subjects: (listed.subject ?? []).join(" ")
        }
      ];
    });
  };
  var readCollection = async (collection) => {
    const found = [];
    for (let page = 1; page <= 3; page += 1) {
      const answer = await locJson(
        `/collections/${collection.slug}/?${query({
          fa: "online-format:audio",
          at: "results,pagination",
          c: 100,
          sp: page
        })}`
      );
      for (const listed of answer?.results ?? []) found.push(...recordingsOf(listed, collection.id));
      if (!answer?.pagination?.next) break;
    }
    return found;
  };
  var kept = null;
  var reading = null;
  var read = async () => {
    const lists = [];
    for (const collection of COLLECTIONS) lists.push(await readCollection(collection));
    const byUrl = /* @__PURE__ */ new Map();
    for (const recording of lists.flat()) {
      if (!byUrl.has(recording.url)) byUrl.set(recording.url, recording);
    }
    const recordings2 = [...byUrl.values()];
    try {
      raygun.storage.set(STORE_KEY, { version: STORE_VERSION, at: Date.now(), recordings: recordings2 });
    } catch (error) {
      raygun.log.warn(`the recordings couldn't be kept: ${String(error)}`);
    }
    return recordings2;
  };
  var recordings = async () => {
    if (kept) return kept;
    const stored = raygun.storage.get(STORE_KEY);
    const usable = stored?.version === STORE_VERSION && Array.isArray(stored.recordings) && stored.recordings.length > 0 ? stored : null;
    if (usable && Date.now() - usable.at < KEEP_MS) {
      kept = usable.recordings;
      return kept;
    }
    reading ??= read().finally(() => {
      reading = null;
    });
    try {
      kept = await reading;
    } catch (error) {
      if (!usable) throw error;
      kept = usable.recordings;
    }
    return kept;
  };
  var trackOf = (recording) => ({
    id: recording.id,
    title: recording.title,
    artist: recording.artist,
    album: COLLECTIONS.find((collection) => collection.id === recording.album)?.title ?? null,
    source: { kind: "url", url: recording.url, headers: {} }
  });
  var score = (recording, wanted) => {
    const title = wordsOf(recording.title);
    const artist = wordsOf(recording.artist ?? "");
    const rest = wordsOf(`${recording.subjects} ${recording.year ?? ""}`);
    let total = 0;
    for (const word of wanted) {
      const starts = (list) => list.some((found) => found.startsWith(word));
      if (title.includes(word)) total += 4;
      else if (starts(title)) total += 3;
      else if (artist.includes(word) || starts(artist)) total += 2;
      else if (starts(rest)) total += 1;
      else return 0;
    }
    return total;
  };
  var albumOf = (collection, all) => ({
    id: collection.id,
    title: collection.title,
    artist: "Library of Congress",
    trackCount: all.filter((recording) => recording.album === collection.id).length,
    kind: "compilation"
  });
  var byYear = (one, other) => (one.year ?? 9999) - (other.year ?? 9999) || one.title.localeCompare(other.title);
  var recordingSearch = defineProvider({
    kind: "music.metadata",
    id: "recordings",
    name: "Library of Congress",
    async search({ query: typed, limit }) {
      const wanted = wordsOf(typed);
      if (wanted.length === 0) return { tracks: [] };
      const found = (await recordings()).map((recording) => ({ recording, points: score(recording, wanted) })).filter(({ points }) => points > 0).sort((one, other) => other.points - one.points || byYear(one.recording, other.recording));
      return {
        tracks: found.slice(0, Math.max(1, limit)).map(({ recording }) => trackOf(recording))
      };
    },
    async searchAlbums({ query: typed, limit }) {
      const wanted = wordsOf(typed);
      if (wanted.length === 0) return { albums: [] };
      const matching = COLLECTIONS.filter((collection) => {
        const words = wordsOf(`${collection.title} ${collection.words} library of congress`);
        return wanted.every((word) => words.some((found) => found.startsWith(word)));
      });
      if (matching.length === 0) return { albums: [] };
      const all = await recordings();
      return { albums: matching.slice(0, Math.max(1, limit)).map((found) => albumOf(found, all)) };
    },
    async album({ id }) {
      const collection = COLLECTIONS.find((found) => found.id === id);
      if (!collection) throw new Error("The Library of Congress has no such collection");
      const all = await recordings();
      return {
        album: albumOf(collection, all),
        tracks: all.filter((recording) => recording.album === id).sort(byYear).map(trackOf)
      };
    }
  });

  // src/index.ts
  registerProvider(films);
  registerProvider(recordingSearch);
})();
