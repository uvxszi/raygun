// dev.raygun.open-webcomics 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var SCHEME = /^[a-z][a-z0-9+.-]*:/i;
  var PARTS = /^([a-z][a-z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/i;
  var normalizePath = (path) => {
    const segments = [];
    const parts = path.split("/");
    parts.forEach((part, index) => {
      const last = index === parts.length - 1;
      if (part === "..") {
        if (segments.length > 1) segments.pop();
        if (last) segments.push("");
      } else if (part === ".") {
        if (last) segments.push("");
      } else {
        segments.push(part);
      }
    });
    const joined = segments.join("/");
    return joined.startsWith("/") ? joined : `/${joined}`;
  };
  var resolveUrl = (base, href) => {
    const link = href.trim();
    if (SCHEME.test(link)) return link;
    const parts = PARTS.exec(base.trim());
    if (!parts) throw new Error(`resolveUrl needs an absolute base address, not ${base}`);
    const [, scheme = "", host = "", path = "", search2 = ""] = parts;
    if (link.startsWith("//")) return `${scheme}${link}`;
    const origin = `${scheme}//${host}`;
    if (link === "") return `${origin}${path || "/"}${search2}`;
    if (link.startsWith("#")) return `${origin}${path || "/"}${search2}${link}`;
    if (link.startsWith("?")) return `${origin}${path || "/"}${link}`;
    const cut = link.search(/[?#]/);
    const [linkPath, rest] = cut < 0 ? [link, ""] : [link.slice(0, cut), link.slice(cut)];
    const directory = path.slice(0, path.lastIndexOf("/") + 1) || "/";
    const joined = linkPath.startsWith("/") ? linkPath : `${directory}${linkPath}`;
    return `${origin}${normalizePath(joined)}${rest}`;
  };

  // src/index.ts
  var SANDRA_AND_WOO = "https://www.sandraandwoo.com";
  var TURNOFF = "https://turnoff.us";
  var REVOY = "https://www.davidrevoy.com";
  var SANDRA_AND_WOO_STRIPS = 1;
  var POSTS_PER_PAGE = 100;
  var LIST_TTL_MS = 30 * 6e4;
  var decode = (text) => text.replace(/<[^>]*>/g, "").replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(Number.parseInt(code, 16))
  ).replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").trim();
  var page = async (address) => {
    const response = await raygun.http.fetch(address);
    if (response.status === 404) throw new Error("This strip is no longer on the site");
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
    return { html: await response.text(), url: response.url || address };
  };
  var pictureAddresses = (base, sources) => [
    ...new Set(
      sources.filter((source) => Boolean(source)).map(
        (source) => resolveUrl(base, source).replace(
          /^http:\/\/(www\.sandraandwoo\.com|turnoff\.us)\//,
          "https://$1/"
        )
      ).filter((url) => /^https?:\/\//.test(url))
    )
  ];
  var MONTH_NAMES = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december"
  ];
  var dayOf = (text) => {
    const parts = /(\d{1,2})\s+([a-z]+)\s+(\d{4})/i.exec(text);
    const month = MONTH_NAMES.indexOf((parts?.[2] ?? "").toLowerCase());
    return parts && month >= 0 ? Date.UTC(Number(parts[3]), month, Number(parts[1])) : null;
  };
  var STRIP_TITLE = /^\[(\d+)\]\s*(.*)$/;
  var KEPT_STRIPS = "sandraAndWoo.strips";
  var NEWEST_POSTS = 20;
  var postsAddress = (order, perPage, at) => `${SANDRA_AND_WOO}/wp-json/wp/v2/posts?categories=${SANDRA_AND_WOO_STRIPS}&per_page=${perPage}&page=${at}&orderby=date&order=${order}&_fields=id,date_gmt,title`;
  var stripOf = (post) => {
    const title = STRIP_TITLE.exec(decode(post.title?.rendered ?? ""));
    if (!title) return [];
    const number = Number(title[1]);
    const date = post.date_gmt ? Date.parse(`${post.date_gmt}Z`) : NaN;
    return [
      {
        id: String(post.id),
        name: title[2]?.trim() || `Strip ${number}`,
        number,
        date: Number.isFinite(date) ? date : null
      }
    ];
  };
  var readAllStrips = async () => {
    const address = postsAddress("asc", POSTS_PER_PAGE, 1);
    const first = await raygun.http.fetch(address);
    if (!first.ok) throw new Error(`HTTP ${first.status} from ${address}`);
    const pages2 = Math.min(Number.parseInt(first.headers["x-wp-totalpages"] ?? "1", 10) || 1, 50);
    const posts = [...await first.json()];
    for (let at = 2; at <= pages2; at += 2) {
      const batch = await Promise.all(
        [at, at + 1].filter((page2) => page2 <= pages2).map((page2) => fetchJson(postsAddress("asc", POSTS_PER_PAGE, page2)))
      );
      posts.push(...batch.flat());
    }
    return posts.flatMap(stripOf);
  };
  var numbered = (strips2) => {
    const byNumber = /* @__PURE__ */ new Map();
    for (const strip of strips2) if (!byNumber.has(strip.number)) byNumber.set(strip.number, strip);
    return [...byNumber.values()].sort((a, b) => b.number - a.number);
  };
  var sandraAndWooStrips = async () => {
    const kept = (raygun.storage.get(KEPT_STRIPS) ?? []).map(
      ([id, number, name, date]) => ({ id, number, name, date })
    );
    let strips2;
    if (kept.length > 0) {
      try {
        const newest = await fetchJson(postsAddress("desc", NEWEST_POSTS, 1));
        strips2 = numbered([...kept, ...newest.flatMap(stripOf)]);
      } catch {
        return numbered(kept);
      }
    } else {
      strips2 = numbered(await readAllStrips());
    }
    if (strips2.length === 0) throw new Error("Sandra and Woo listed no strips");
    try {
      raygun.storage.set(
        KEPT_STRIPS,
        strips2.map((strip) => [strip.id, strip.number, strip.name, strip.date])
      );
    } catch {
    }
    return strips2;
  };
  var sandraAndWooPictures = async (strip) => {
    if (!/^\d+$/.test(strip)) throw new Error("Sandra and Woo has no such strip");
    const { html, url } = await page(`${SANDRA_AND_WOO}/?p=${strip}`);
    const pictures = pictureAddresses(
      url,
      raygun.html.select(html, "#comic img").map((image) => image.attr("src"))
    );
    if (pictures.length === 0) throw new Error("This post has no strip to show");
    return pictures;
  };
  var turnoffStrips = async () => {
    const { html } = await page(`${TURNOFF}/all/`);
    const links = raygun.html.select(html, "ul.post-list a.post-link[href]").map((link) => ({ slug: /^\/geek\/([^/]+)\/?$/.exec(link.attr("href") ?? "")?.[1], link })).filter(
      (found) => Boolean(found.slug)
    );
    if (links.length === 0) throw new Error("turnoff.us listed no comics");
    return links.map(({ slug, link }, index) => ({
      id: slug,
      name: decode(link.text) || slug.replace(/-/g, " "),
      number: links.length - index,
      date: null
    }));
  };
  var turnoffPictures = async (strip) => {
    if (!/^[\w.-]+$/.test(strip)) throw new Error("turnoff.us has no such comic");
    const { html, url } = await page(`${TURNOFF}/geek/${strip}/`);
    const pictures = pictureAddresses(
      url,
      raygun.html.select(html, "img[src]").map((image) => image.attr("src")).filter((source) => /\/image\/en\//.test(source ?? ""))
    );
    if (pictures.length === 0) throw new Error("This comic has no picture to show");
    return pictures;
  };
  var KEPT_EPISODES = "miniFantasyTheater.episodes";
  var TAG_PAGES_MOST = 30;
  var episodeStrips = /* @__PURE__ */ new Map();
  var tagPage = (at) => `${REVOY}/tag/minifantasytheater${at > 1 ? `/page${at}` : ""}`;
  var episodeCards = async (at) => {
    const response = await raygun.http.fetch(tagPage(at));
    if (response.status === 404) return [];
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${tagPage(at)}`);
    const html = await response.text();
    return [
      ...html.matchAll(
        /<figure class="thumbnail">\s*<a href="https:\/\/www\.davidrevoy\.com\/article(\d+)\/([a-z0-9-]+)" title="([^"]*)">\s*<img src="[^"]*?[?&]src=(https:\/\/www\.davidrevoy\.com\/data\/images\/[^"&]+)[\s\S]*?<span class="detail">([^<]*)/g
      )
    ].map((card) => {
      const post = `${card[1]}/${card[2]}`;
      return {
        post,
        picture: card[4],
        strip: {
          id: post,
          name: decode(card[3] ?? "") || post,
          number: 0,
          date: dayOf(card[5] ?? "")
        }
      };
    });
  };
  var readAllEpisodes = async () => {
    const cards = [];
    for (let at = 1; at <= TAG_PAGES_MOST; at += 2) {
      const pages2 = await Promise.all([episodeCards(at), episodeCards(at + 1)]);
      cards.push(...pages2.flat());
      if (pages2.some((page2) => page2.length === 0)) break;
    }
    return cards;
  };
  var miniFantasyTheaterStrips = async () => {
    const kept = raygun.storage.get(KEPT_EPISODES) ?? [];
    let cards = kept.map(([post, name, picture, date]) => ({
      post,
      picture,
      strip: { id: post, name, number: 0, date }
    }));
    try {
      if (cards.length > 0) {
        const newest = (await episodeCards(1)).filter(
          (card) => !cards.some((each) => each.post === card.post)
        );
        cards = [...newest, ...cards];
      } else {
        cards = await readAllEpisodes();
      }
    } catch (error) {
      if (cards.length === 0) throw error;
    }
    if (cards.length === 0) throw new Error("MiniFantasyTheater listed no episodes");
    for (const card of cards) episodeStrips.set(card.post, card.picture);
    try {
      raygun.storage.set(
        KEPT_EPISODES,
        cards.map((card) => [card.post, card.strip.name, card.picture, card.strip.date])
      );
    } catch {
    }
    return cards.map((card, index) => ({ ...card.strip, number: cards.length - index }));
  };
  var miniFantasyTheaterPictures = async (strip) => {
    if (!/^\d+\/[a-z0-9-]+$/.test(strip)) throw new Error("MiniFantasyTheater has no such episode");
    const { html, url } = await page(`${REVOY}/article${strip}`);
    const card = episodeStrips.get(strip) ?? (raygun.storage.get(KEPT_EPISODES) ?? []).find(([post]) => post === strip)?.[2];
    const name = (address) => address.split("/").pop() ?? "";
    const linked = pictureAddresses(
      url,
      raygun.html.select(html, "a[href]").map((link) => link.attr("href")).filter((href) => /\/data\/images\/blog\/.+\.(?:jpe?g|png|webp)$/i.test(href ?? ""))
    );
    const strips2 = linked.filter(
      (address) => /minifantasytheater/i.test(address) || card && name(address) === name(card)
    );
    if (strips2.length > 0) return strips2;
    if (card) return [card];
    if (linked.length > 0) return linked;
    throw new Error("This post has no strip to show");
  };
  var SERIES = [
    {
      id: "sandra-and-woo",
      title: "Sandra and Woo",
      coverUrl: `${SANDRA_AND_WOO}/images/cast/woo.png`,
      authors: ["Oliver Knörzer"],
      artists: ["Powree"],
      description: "A webcomic about Sandra, her pet raccoon Woo and their friends, written by Oliver Knörzer and drawn by Powree, begun in 2008.\n\nLicence: Creative Commons Attribution-NonCommercial-NoDerivs 3.0 (CC BY-NC-ND 3.0).",
      genres: ["Comedy", "Slice of life", "Webcomic"],
      status: "unknown",
      url: SANDRA_AND_WOO,
      keywords: "sandra woo raccoon knorzer knoerzer powree",
      readStrips: sandraAndWooStrips,
      pictures: sandraAndWooPictures
    },
    {
      id: "turnoff",
      title: "turnoff.us",
      coverUrl: `${TURNOFF}/image/en/bsod.jpeg`,
      authors: ["Daniel Stori"],
      artists: ["Daniel Stori"],
      description: "A webcomic about programming, operating systems and life in tech, by Daniel Stori.\n\nLicence: Creative Commons Attribution-NonCommercial-ShareAlike 4.0 (CC BY-NC-SA 4.0).",
      genres: ["Comedy", "Programming", "Webcomic"],
      status: "ongoing",
      url: TURNOFF,
      keywords: "turnoff turn off geek programming linux developer stori",
      readStrips: turnoffStrips,
      pictures: turnoffPictures
    },
    {
      id: "mini-fantasy-theater",
      title: "MiniFantasyTheater",
      coverUrl: `${REVOY}/data/images/blog/2024/en_007_minifantasytheater_by-david-revoy.jpg`,
      authors: ["David Revoy"],
      artists: ["David Revoy"],
      description: "Short fantasy gag strips by David Revoy, the author of Pepper&Carrot, begun in 2024.\n\nLicence: Creative Commons Attribution-ShareAlike 4.0 (CC BY-SA 4.0); the author prohibits AI-generated derivations of his work.",
      genres: ["Comedy", "Fantasy", "Webcomic"],
      status: "ongoing",
      url: `${REVOY}/tag/minifantasytheater`,
      keywords: "mini fantasy theater theatre minifantasytheater revoy pepper carrot",
      readStrips: miniFantasyTheaterStrips,
      pictures: miniFantasyTheaterPictures
    }
  ];
  var lists = /* @__PURE__ */ new Map();
  var strips = (series) => {
    const kept = lists.get(series.id);
    if (kept && Date.now() - kept.at < LIST_TTL_MS) return kept.strips;
    const current = { at: Date.now(), strips: series.readStrips() };
    current.strips.catch(() => {
      if (lists.get(series.id) === current) lists.delete(series.id);
    });
    lists.set(series.id, current);
    return current.strips;
  };
  var seriesOf = (id) => {
    const found = SERIES.find((series) => series.id === id);
    if (!found) throw new Error("Open webcomics has no such series");
    return found;
  };
  var summary = (series) => ({
    id: series.id,
    title: series.title,
    coverUrl: series.coverUrl,
    authors: series.authors
  });
  var ALL = { items: SERIES.map(summary), hasNextPage: false };
  var simple = (text) => text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var search = async (query, page2) => {
    if (page2 > 1) return { items: [], hasNextPage: false };
    const words = simple(query).split(" ").filter(Boolean);
    if (words.length === 0) return ALL;
    const matches = (text) => {
      const haystack = simple(text);
      return words.every((word) => haystack.includes(word));
    };
    const found = [];
    for (const series of SERIES) {
      const named = matches(`${series.title} ${series.authors.join(" ")} ${series.keywords}`);
      const kept = lists.get(series.id);
      const strip = !named && kept ? await kept.strips.then(
        (list) => list.some((each) => matches(each.name)),
        () => false
      ) : false;
      if (named || strip) found.push(series);
    }
    return { items: found.map(summary), hasNextPage: false };
  };
  var details = async (id) => {
    const series = seriesOf(id);
    return {
      id: series.id,
      title: series.title,
      coverUrl: series.coverUrl,
      authors: series.authors,
      artists: series.artists,
      description: series.description,
      genres: series.genres,
      status: series.status,
      url: series.url
    };
  };
  var chapterId = (series, strip) => `${series}/${strip}`;
  var chapters = async (id) => {
    const series = seriesOf(id);
    return (await strips(series)).map((strip) => ({
      id: chapterId(series.id, strip.id),
      name: strip.name,
      number: strip.number,
      dateUpload: strip.date
    }));
  };
  var pages = async (id) => {
    const cut = id.indexOf("/");
    if (cut <= 0) throw new Error("Open webcomics has no such strip");
    return seriesOf(id.slice(0, cut)).pictures(id.slice(cut + 1));
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "webcomics",
      name: "Open webcomics",
      popular: async () => ALL,
      latest: async () => ALL,
      search: ({ query, page: at }) => search(query, at),
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId: id }) => ({ pages: await pages(id) })
    })
  );
})();
