// dev.raygun.green-tea-press 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/books.ts
  var DOWNEY = "Allen B. Downey";
  var BOOKS = [
    {
      id: "think-python-3e",
      title: "Think Python (3rd edition)",
      authors: [DOWNEY],
      description: "An introduction to programming in Python for beginners, which teaches how to think like a computer scientist: values and expressions, functions, iteration, strings, lists, dictionaries, classes and debugging, with exercises in every chapter.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2024/09/think_python_3e.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/ThinkPython/",
      site: "https://allendowney.github.io/ThinkPython/",
      format: "jupyter",
      subject: "programming",
      year: 2024,
      skip: ["blank.html", "quizzes.html"]
    },
    {
      id: "think-stats-3e",
      title: "Think Stats (3rd edition)",
      authors: [DOWNEY],
      description: "Exploratory data analysis in Python: distributions, probability, relationships between variables, estimation, hypothesis testing, regression, time series and survival analysis, worked through with real data sets.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2025/04/think_stats_3e_cover.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/ThinkStats/chap01.html",
      site: "https://allendowney.github.io/ThinkStats/",
      format: "jupyter",
      subject: "data",
      year: 2025
    },
    {
      id: "think-bayes-2e",
      title: "Think Bayes (2nd edition)",
      authors: [DOWNEY],
      description: "Bayesian statistics in Python, taught with computation rather than calculus: Bayes's theorem, distributions, estimation, decision analysis, MCMC and approximate Bayesian computation, with many worked problems.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2021/04/think_bayes_2e_cover_medium.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/ThinkBayes2/",
      site: "https://allendowney.github.io/ThinkBayes2/",
      format: "jupyter",
      subject: "data",
      year: 2021
    },
    {
      id: "elements-of-data-science",
      title: "Elements of Data Science",
      authors: [DOWNEY],
      description: "An introduction to data science in Python for people with no programming experience: variables, lists and arrays, dictionaries, plotting, DataFrames, distributions, regression, resampling and hypothesis testing.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2026/01/eds_cover_small.png",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/ElementsOfDataScience/",
      site: "https://allendowney.github.io/ElementsOfDataScience/",
      format: "jupyter",
      subject: "data",
      year: 2024
    },
    {
      id: "modsimpy",
      title: "Modeling and Simulation in Python",
      authors: [DOWNEY],
      description: "Modeling physical systems with simulation in Python: population growth, epidemics, thermal systems, pharmacokinetics, projectiles, drag and rotation, from building a model to validating it.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2023/05/modsimpy_cover.png",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/ModSimPy/",
      site: "https://allendowney.github.io/ModSimPy/",
      format: "jupyter",
      subject: "science",
      year: 2023
    },
    {
      id: "think-java-2e",
      title: "Think Java (2nd edition)",
      authors: [DOWNEY, "Chris Mayfield"],
      description: "An introduction to computer science and programming in Java for beginners: variables, methods, conditionals, loops, arrays, recursion, objects and classes, with a strong focus on debugging.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2020/06/think_java_covr_large-229x300.png",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/wp/think-java-2e/",
      site: "https://chrismayfield.github.io/ThinkJava2/",
      format: "quarto",
      subject: "programming",
      year: 2020
    },
    {
      id: "think-dsp",
      title: "Think DSP",
      authors: [DOWNEY],
      description: "Digital signal processing in Python: sounds and signals, harmonics, noise, autocorrelation, the discrete cosine and Fourier transforms, filtering, convolution and modulation, explored with code and sound.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2016/01/think_dsp_cover-229x300.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/thinkdsp/html/index.html",
      site: "https://greenteapress.com/thinkdsp/html/",
      format: "hevea",
      subject: "science",
      year: 2016
    },
    {
      id: "think-complexity-2e",
      title: "Think Complexity (2nd edition)",
      authors: [DOWNEY],
      description: "Complexity science in Python: graphs, small-world and scale-free networks, cellular automata, the Game of Life, self-organized criticality, agent-based models and the evolution of cooperation.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2018/09/think_complexity_2e.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/complexity2/html/index.html",
      site: "https://greenteapress.com/complexity2/html/",
      format: "hevea",
      subject: "science",
      year: 2018
    },
    {
      id: "think-os",
      title: "Think OS",
      authors: [DOWNEY],
      description: "A brief introduction to operating systems for programmers: compilation, processes, virtual memory, file systems, memory management, caching, multitasking, threads and synchronization, with examples in C.",
      cover: null,
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/thinkos/html/index.html",
      site: "https://greenteapress.com/thinkos/html/",
      format: "hevea",
      subject: "systems",
      year: 2015
    },
    {
      id: "think-data-structures",
      title: "Think Data Structures",
      authors: [DOWNEY],
      description: "Algorithms and information retrieval in Java: interfaces, analysis of algorithms, lists, trees, maps, hashing and persistence, built up into a web crawler and search engine over Wikipedia's pages.",
      cover: "https://greenteapress.com/wp/wp-content/uploads/2016/11/think_data_structures_cover-229x300.png",
      licence: "CC BY-NC-SA 3.0",
      licenceAt: "https://greenteapress.com/thinkdast/html/index.html",
      site: "https://greenteapress.com/thinkdast/html/",
      format: "hevea",
      subject: "programming",
      year: 2017
    },
    {
      id: "dsirp",
      title: "Data Structures and Information Retrieval in Python",
      authors: [DOWNEY],
      description: "Data structures and algorithms in Python, from analysis of algorithms and recursion to hash maps, heaps, Huffman codes, graphs and search, building up to a web crawler, indexer and PageRank.",
      cover: null,
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://allendowney.github.io/DSIRP/",
      site: "https://allendowney.github.io/DSIRP/",
      format: "jupyter",
      subject: "programming",
      year: 2021
    },
    {
      id: "astronomical-data",
      title: "Astronomical Data in Python",
      authors: [DOWNEY],
      description: "Working with astronomical data in Python: querying the Gaia database, coordinates and units, proper motion, joining tables, photometry and visualization, replicating parts of a published analysis.",
      cover: null,
      licence: "CC BY 4.0",
      licenceAt: "https://allendowney.github.io/AstronomicalData/",
      site: "https://allendowney.github.io/AstronomicalData/",
      format: "jupyter",
      subject: "science",
      year: 2021
    },
    {
      id: "think-python-2e",
      title: "Think Python (2nd edition)",
      authors: [DOWNEY],
      description: "The earlier edition of Think Python, for Python 3: an introduction to programming and to thinking like a computer scientist, with case studies on interface design, word play and data structure selection.",
      cover: "https://greenteapress.com/thinkpython2/think_python2_medium.jpg",
      licence: "CC BY-NC 3.0",
      licenceAt: "https://greenteapress.com/wp/think-python-2e/",
      site: "https://greenteapress.com/thinkpython2/html/",
      format: "hevea",
      subject: "programming",
      year: 2015
    },
    {
      id: "think-stats-2e",
      title: "Think Stats (2nd edition)",
      authors: [DOWNEY],
      description: "The earlier edition of Think Stats: exploratory data analysis in Python, from distributions and probability to estimation, hypothesis testing, regression, time series and survival analysis.",
      cover: "https://greenteapress.com/thinkstats2/thinkstats2cover.jpg",
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/thinkstats2/html/index.html",
      site: "https://greenteapress.com/thinkstats2/html/",
      format: "hevea",
      subject: "data",
      year: 2014
    },
    {
      id: "think-bayes-1e",
      title: "Think Bayes (1st edition)",
      authors: [DOWNEY],
      description: "The first edition of Think Bayes: Bayesian statistics made simple with Python, from computational statistics and estimation to prediction, hypothesis testing and hierarchical models.",
      cover: null,
      licence: "CC BY-NC-SA 4.0",
      licenceAt: "https://greenteapress.com/thinkbayes/html/index.html",
      site: "https://greenteapress.com/thinkbayes/html/",
      format: "hevea",
      subject: "data",
      year: 2012
    }
  ];

  // src/index.ts
  var HEADERS = { "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )" };
  var PAGE = /^[\w.-]+\.html$/;
  var PAGE_SIZE = 30;
  var SUBJECTS = [
    { value: "all", label: "All" },
    { value: "programming", label: "Programming" },
    { value: "data", label: "Statistics and data science" },
    { value: "science", label: "Science and engineering" },
    { value: "systems", label: "Computer systems" }
  ];
  var SORTS = [
    { value: "featured", label: "Featured" },
    { value: "newest", label: "Newest" },
    { value: "title", label: "Title (A–Z)" }
  ];
  var decode = (text) => text.replace(/&nbsp;|&#160;|&#XA0;/gi, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var plainText = (html) => decode(html.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();
  var words = (text) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var VOID_ELEMENTS = /* @__PURE__ */ new Set([
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "source",
    "track",
    "wbr"
  ]);
  var TAG = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g;
  var dropElements = (html, drop) => {
    let kept = "";
    let from = 0;
    let open = null;
    for (const match of html.matchAll(TAG)) {
      const [whole, closing, name = "", attributes = ""] = match;
      const tag = name.toLowerCase();
      const at = match.index ?? 0;
      const lone = VOID_ELEMENTS.has(tag) || attributes.trimEnd().endsWith("/");
      if (open) {
        if (tag === open.tag && !lone) open.depth += closing ? -1 : 1;
        if (open.depth === 0) {
          open = null;
          from = at + whole.length;
        }
        continue;
      }
      if (closing || !drop(tag, attributes)) continue;
      kept += html.slice(from, at);
      from = at + whole.length;
      if (!lone) open = { tag, depth: 1 };
    }
    return open ? kept : kept + html.slice(from);
  };
  var element = (html, start) => {
    const at = html.search(start);
    if (at < 0) return null;
    let depth = 0;
    let tag = "";
    for (const match of html.slice(at).matchAll(TAG)) {
      const [whole, closing, name = ""] = match;
      const current = name.toLowerCase();
      if (!tag) tag = current;
      if (current !== tag || VOID_ELEMENTS.has(current)) continue;
      depth += closing ? -1 : 1;
      if (depth === 0) return html.slice(at, at + (match.index ?? 0) + whole.length);
    }
    return html.slice(at);
  };
  var classes = (attributes) => /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(attributes)?.slice(1).join(" ").split(/\s+/).filter(Boolean) ?? [];
  var fetchPage = async (url) => {
    let response;
    try {
      response = await raygun.http.fetch(url, { headers: HEADERS });
    } catch {
      throw new Error("The book's website didn't answer; try again in a minute");
    }
    if (response.status === 404) throw new Error("The book no longer has this page");
    if (!response.ok)
      throw new Error(`The book's website answered with an error (HTTP ${response.status})`);
    return { text: await response.text(), url: response.url || url };
  };
  var bookOf = (id) => {
    const book = BOOKS.find((known) => known.id === id);
    if (!book) throw new Error("Green Tea Press has no book by this name here");
    return book;
  };
  var contents = (book, html) => {
    const links = [];
    const add = (href, title) => {
      const page = decode(href).replace(/^\.\//, "");
      if (!PAGE.test(page) || links.some((link) => link.page === page)) return;
      if (book.skip?.includes(page) || /^(?:contents|index)$/i.test(title)) return;
      links.push({ page, title: title || page.replace(/\.html$/, "") });
    };
    const anchors = (part) => [...part.matchAll(/<a\b([^>]*)>([\s\S]*?)<\/a>/g)];
    if (book.format === "jupyter") {
      const nav = element(html, /<nav\b[^>]*\bbd-docs-nav\b/) ?? "";
      for (const match of anchors(nav)) {
        const href = /\bhref="([^"#]+)"/.exec(match[1] ?? "")?.[1];
        if (href && classes(match[1] ?? "").includes("internal"))
          add(href, plainText(match[2] ?? ""));
      }
    } else if (book.format === "quarto") {
      const sidebar = element(html, /<nav id="quarto-sidebar"/) ?? html;
      for (const match of anchors(sidebar)) {
        const href = /\bhref="([^"#]+)"/.exec(match[1] ?? "")?.[1];
        if (href && classes(match[1] ?? "").includes("sidebar-link")) {
          add(href, plainText(match[2] ?? "").replace(/^(\d+)\s+/, "$1 "));
        }
      }
    } else {
      const content = element(html, /<div id="content"/) ?? html;
      for (const match of anchors(content)) {
        const href = /\bhref="([^"#]+)"/.exec(match[1] ?? "")?.[1];
        if (href && !/^[a-z]+:/i.test(href)) add(href, plainText(match[2] ?? ""));
      }
    }
    return links;
  };
  var isNotice = (paragraph) => /colab\.research\.google\.com|mybinder\.org/.test(paragraph) || /^(?:you can order print|printed (?:and electronic )?copies|print and electronic versions|the (?:third|second) edition of .* is available now|if you find this book useful|and you might like this new book)/i.test(
    plainText(paragraph)
  );
  var LICENCE_LINE = /^(?:Copyright\s+(?:©\s*)?\d{4}|Code license:|Text license:|License:)/i;
  var dropLicenceLines = (html) => {
    const paragraphs = [...html.matchAll(/<p\b[^>]*>([\s\S]*?)<\/p>/g)];
    const dropped = /* @__PURE__ */ new Set();
    paragraphs.forEach((match, index) => {
      if (!LICENCE_LINE.test(plainText(match[1] ?? ""))) return;
      dropped.add(index);
      const before = paragraphs[index - 1];
      const title = plainText(before?.[1] ?? "");
      if (before && /^Copyright/i.test(plainText(match[1] ?? "")) && title.length < 90 && !/[.:?!]$/.test(title)) {
        dropped.add(index - 1);
      }
    });
    let kept = html;
    for (const index of [...dropped].sort((first, second) => second - first)) {
      const match = paragraphs[index];
      if (!match || match.index === void 0) continue;
      kept = kept.slice(0, match.index) + kept.slice(match.index + match[0].length);
    }
    return kept;
  };
  var jupyterText = (html) => {
    const article = element(html, /<article class="bd-article/) ?? element(html, /<main id="main-content"/) ?? "";
    const first = article.search(/<section\b/);
    const body = first >= 0 ? article.slice(first) : article;
    const cleaned = dropElements(body, (tag, attributes) => {
      const names = classes(attributes);
      return tag === "summary" || tag === "footer" || tag === "script" || names.includes("headerlink") || names.some((name) => name.startsWith("prev-next"));
    });
    return dropLicenceLines(
      cleaned.replace(
        /<p\b[^>]*>[\s\S]*?<\/p>/g,
        (paragraph) => isNotice(paragraph) ? "" : paragraph
      )
    );
  };
  var quartoText = (html) => {
    const main = element(html, /<main class="content"/) ?? "";
    return dropElements(main, (tag, attributes) => {
      const names = classes(attributes);
      return tag === "nav" || tag === "script" || names.includes("notice") || names.includes("anchorjs-link") || names.includes("quarto-title-meta");
    });
  };
  var HEVEA_NAVIGATION = new RegExp(
    String.raw`(?:<hr\b[^>]*>\s*)?(?:<a\b[^>]*>\s*<img\b[^>]*\balt="(?:Previous|Up|Next)"[^>]*>\s*<\/a>\s*)+(?:<hr\b[^>]*>)?`,
    "gi"
  );
  var heveaText = (html) => {
    const part = element(html, /<div id="content"/) ?? element(html, /<body\b/) ?? html;
    return dropElements(part, (tag, attributes) => {
      const names = classes(attributes);
      return tag === "script" || names.includes("notice") || names.includes("gtp-sidebar") || /\bid="(?:sidebar|ea-slot)"/.test(attributes);
    }).replace(HEVEA_NAVIGATION, "").replace(/<a\b[^>]*>\s*Buy this book[\s\S]*?<\/a>/gi, "");
  };
  var summary = (book) => ({
    id: book.id,
    title: book.title,
    coverUrl: book.cover,
    authors: book.authors
  });
  var pageOf = (books, page) => {
    const start = (Math.max(1, page) - 1) * PAGE_SIZE;
    return {
      items: books.slice(start, start + PAGE_SIZE).map(summary),
      hasNextPage: start + PAGE_SIZE < books.length
    };
  };
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var newest = (books) => [...books].sort((first, second) => second.year - first.year);
  var search = (query, page, values) => {
    const subject = pick(values, "subject", SUBJECTS);
    const sort = pick(values, "sort", SORTS);
    const terms = words(query).split(" ").filter(Boolean);
    const found = BOOKS.filter((book) => {
      if (subject && subject !== "all" && book.subject !== subject) return false;
      const text = words(`${book.title} ${book.authors.join(" ")} ${book.description}`);
      return terms.every((term) => text.includes(term));
    });
    const titled = (book) => terms.filter((term) => words(book.title).includes(term)).length;
    const ordered = sort === "newest" ? newest(found) : sort === "title" ? [...found].sort((first, second) => first.title.localeCompare(second.title)) : [...found].sort((first, second) => titled(second) - titled(first));
    return pageOf(ordered, page);
  };
  var SUBJECT_NAMES = {
    programming: "Programming",
    data: "Statistics and data science",
    science: "Science and engineering",
    systems: "Computer systems"
  };
  var details = (book) => ({
    id: book.id,
    title: book.title,
    coverUrl: book.cover,
    authors: book.authors,
    description: [
      book.description,
      `Published by Green Tea Press, ${book.year}. Licence: ${book.licence}, as stated at ${book.licenceAt}`
    ].join("\n\n"),
    genres: [SUBJECT_NAMES[book.subject]],
    status: "completed",
    url: book.site
  });
  var filters = [
    { kind: "select", key: "subject", label: "Subject", options: SUBJECTS },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "books",
      name: "Green Tea Press",
      popular: async ({ page }) => pageOf(BOOKS, page),
      latest: async ({ page }) => pageOf(newest(BOOKS), page),
      search: async ({ query, page, filters: values }) => search(query, page, values),
      filters: async () => filters,
      details: async ({ id }) => details(bookOf(id)),
      chapters: async ({ id }) => {
        const book = bookOf(id);
        const index = await fetchPage(`${book.site}index.html`);
        const chapters = contents(book, index.text).map((link, number) => ({
          id: `${book.id}/${link.page}`,
          name: link.title,
          number: number + 1
        }));
        if (chapters.length === 0) throw new Error("This book's contents list no chapters");
        return { chapters };
      },
      passage: async ({ chapterId }) => {
        const slash = chapterId.indexOf("/");
        const book = bookOf(chapterId.slice(0, slash));
        const page = chapterId.slice(slash + 1);
        if (slash < 0 || !PAGE.test(page)) throw new Error("Not a chapter of a Green Tea Press book");
        const found = await fetchPage(`${book.site}${page}`);
        const html = book.format === "jupyter" ? jupyterText(found.text) : book.format === "quarto" ? quartoText(found.text) : heveaText(found.text);
        if (!plainText(html)) throw new Error("This chapter has no text");
        return { html, baseUrl: found.url };
      }
    })
  );
})();
