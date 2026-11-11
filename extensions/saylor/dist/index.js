// dev.raygun.saylor 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/books.ts
  var BOOKS = [
    {
      id: "advanced-business-law-and-the-legal-environment",
      title: "Advanced Business Law and the Legal Environment",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "american-government-and-politics-in-the-information-age",
      title: "American Government and Politics in the Information Age",
      edition: "1.0",
      subject: "history"
    },
    {
      id: "building-strategy-and-performance",
      title: "Building Strategy and Performance",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "business-communication-for-success",
      title: "Business Communication for Success",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "business-english-for-success",
      title: "Business English for Success",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "business-information-systems-design-an-app-for-that",
      title: "Business Information Systems: Design an App for That",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "business-law-and-the-legal-environment-v1.0",
      title: "Business Law and the Legal Environment",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "business-law-and-the-legal-environment-emba-edition",
      title: "Business Law and the Legal Environment (EMBA Edition)",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "business-law-and-the-legal-environment-macc-edition",
      title: "Business Law and the Legal Environment: MAcc Edition",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "college-success",
      title: "College Success",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "corporate-governance",
      title: "Corporate Governance",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "criminal-law",
      title: "Criminal Law",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "developing-new-products-and-services",
      title: "Developing New Products and Services",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "economics-theory-through-applications",
      title: "Economics: Theory Through Applications",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "emarketing-the-essential-guide-to-online-marketing",
      title: "eMarketing: The Essential Guide to Online Marketing",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "essentials-of-geographic-information-systems",
      title: "Essentials of Geographic Information Systems",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "essentials-of-nutrition-a-functional-approach",
      title: "Essentials of Nutrition: A Functional Approach",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "exploring-business",
      title: "Exploring Business (v1.0)",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "exploring-business-v2.0",
      title: "Exploring Business (v2.0)",
      edition: "2.0",
      subject: "business"
    },
    {
      id: "exploring-perspectives-a-concise-guide-to-analysis",
      title: "Exploring Perspectives: A Concise Guide to Analysis",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "financial-accounting",
      title: "Financial Accounting",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "focusing-on-organizational-change",
      title: "Focusing on Organizational Change",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "foundations-of-business-law-and-the-legal-environment",
      title: "Foundations of Business Law and the Legal Environment",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "foundations-of-music-theory",
      title: "Foundations of Music Theory",
      edition: "1.0",
      subject: "arts"
    },
    {
      id: "fundamentals-of-global-strategy",
      title: "Fundamentals of Global Strategy",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "general-chemistry-principles-patterns-and-applications-v1.0",
      title: "General Chemistry: Principles, Patterns, and Applications (v1.0)",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "general-chemistry-principles-patterns-and-applications-v1.0m",
      title: "General Chemistry: Principles, Patterns, and Applications (v1.0M)",
      edition: "1.0M",
      subject: "science"
    },
    {
      id: "government-regulation-and-the-legal-environment-of-business",
      title: "Government Regulation and the Legal Environment of Business",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "growth-and-competitive-strategy-in-3-circles",
      title: "Growth and Competitive Strategy in 3 Circles",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "handbook-for-writers",
      title: "Handbook for Writers",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "how-to-use-microsoft-excel-v1.0",
      title: "How to Use Microsoft Excel: The Careers in Practice Series (v1.0)",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "how-to-use-microsoft-excel-v1.1",
      title: "How to Use Microsoft Excel: The Careers in Practice Series (v1.1)",
      edition: "1.1",
      subject: "business"
    },
    {
      id: "human-relations",
      title: "Human Relations",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "human-resource-management",
      title: "Human Resource Management",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "information-systems-a-managers-guide-to-harnessing-technology-v1.0",
      title: "Information Systems: A Manager's Guide to Harnessing Technology (v1.0)",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "information-systems-a-managers-guide-to-harnessing-technology-v2.0",
      title: "Information Systems: A Manager's Guide to Harnessing Technology (v2.0)",
      edition: "2.0",
      subject: "business"
    },
    {
      id: "intermediate-algebra",
      title: "Intermediate Algebra",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "international-business",
      title: "International Business",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "international-economics-theory-and-policy",
      title: "International Economics: Theory and Policy",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "international-finance-theory-and-policy",
      title: "International Finance: Theory and Policy",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "international-trade-theory-and-policy",
      title: "International Trade: Theory and Policy",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "introduction-to-contracts-sales-and-product-liability",
      title: "Introduction to Contracts, Sales and Product Liability",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "introduction-to-economic-analysis",
      title: "Introduction to Economic Analysis",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "introduction-to-psychology",
      title: "Introduction to Psychology",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "introduction-to-the-law-of-property-estate-planning-and-insurance",
      title: "Introduction to the Law of Property, Estate Planning and Insurance",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "introductory-chemistry",
      title: "Introductory Chemistry",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "introductory-statistics",
      title: "Introductory Statistics",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "launch-advertising-and-promotion-in-real-time",
      title: "Launch! Advertising and Promotion in Real Time",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "law-for-entrepreneurs",
      title: "Law for Entrepreneurs",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "law-of-commercial-transactions",
      title: "Law of Commercial Transactions",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "leading-with-cultural-intelligence",
      title: "Leading with Cultural Intelligence",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "legal-aspects-of-corporate-management-and-finance",
      title: "Legal Aspects of Corporate Management and Finance",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "legal-aspects-of-marketing-and-sales",
      title: "Legal Aspects of Marketing and Sales",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "macroeconomics-theory-through-applications",
      title: "Macroeconomics: Theory Through Applications",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "managerial-accounting",
      title: "Managerial Accounting",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "mastering-public-relations",
      title: "Mastering Public Relations",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "mastering-strategic-management",
      title: "Mastering Strategic Management",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "microeconomics-theory-through-applications",
      title: "Microeconomics: Theory Through Applications",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "money-and-banking-v2.0",
      title: "Money and Banking",
      edition: "2.0",
      subject: "economics"
    },
    {
      id: "organizational-behavior-v1.1",
      title: "Organizational Behavior",
      edition: "1.1",
      subject: "business"
    },
    {
      id: "personal-finance",
      title: "Personal Finance",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "principles-of-economics-v2.0",
      title: "Principles of Economics",
      edition: "2.0",
      subject: "economics"
    },
    {
      id: "principles-of-macroeconomics-v1.0",
      title: "Principles of Macroeconomics (v1.0)",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "principles-of-macroeconomics-v1.1",
      title: "Principles of Macroeconomics (v1.1)",
      edition: "1.1",
      subject: "economics"
    },
    {
      id: "principles-of-macroeconomics-v2.0",
      title: "Principles of Macroeconomics (v2.0)",
      edition: "2.0",
      subject: "economics"
    },
    {
      id: "principles-of-management-v1.1",
      title: "Principles of Management",
      edition: "1.1",
      subject: "business"
    },
    {
      id: "principles-of-managerial-economics",
      title: "Principles of Managerial Economics",
      edition: "1.0",
      subject: "economics"
    },
    {
      id: "principles-of-marketing-v2.0",
      title: "Principles of Marketing",
      edition: "2.0",
      subject: "business"
    },
    {
      id: "principles-of-microeconomics-v2.0",
      title: "Principles of Microeconomics",
      edition: "2.0",
      subject: "economics"
    },
    {
      id: "principles-of-social-psychology",
      title: "Principles of Social Psychology",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "principles-of-sociological-inquiry-qualitative-and-quantitative-methods",
      title: "Principles of Sociological Inquiry: Qualitative and Quantitative Methods",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "project-management-from-simple-to-complex-v1.1",
      title: "Project Management from Simple to Complex",
      edition: "1.1",
      subject: "business"
    },
    {
      id: "research-methods-in-psychology",
      title: "Research Methods in Psychology",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "risk-management-for-enterprises-and-individuals",
      title: "Risk Management for Enterprises and Individuals",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "six-steps-to-job-search-success",
      title: "Six Steps To Job Search Success",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "small-business-management-in-the-21st-century",
      title: "Small Business Management in the 21st Century",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "social-problems-continuity-and-change",
      title: "Social Problems: Continuity and Change",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "sociology-understanding-and-changing-the-social-world-comprehensive-edition",
      title: "Sociology: Understanding and Changing the Social World, Comprehensive Edition",
      edition: "1.0",
      subject: "social"
    },
    {
      id: "stand-up-speak-out-the-practice-and-ethics-of-public-speaking",
      title: "Stand Up, Speak Out: The Practice and Ethics of Public Speaking",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "sustainability-innovation-and-entrepreneurship",
      title: "Sustainability, Innovation, and Entrepreneurship",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "the-basics-of-general-organic-and-biological-chemistry",
      title: "The Basics of General, Organic, and Biological Chemistry",
      edition: "1.0",
      subject: "science"
    },
    {
      id: "the-business-ethics-workshop",
      title: "The Business Ethics Workshop",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "the-legal-and-ethical-environment-of-business",
      title: "The Legal and Ethical Environment of Business",
      edition: "1.0",
      subject: "law"
    },
    {
      id: "the-power-of-selling",
      title: "The Power of Selling",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "the-sustainable-business-case-book",
      title: "The Sustainable Business Case Book",
      edition: "1.0",
      subject: "business"
    },
    {
      id: "understanding-media-and-culture-an-introduction-to-mass-communication",
      title: "Understanding Media and Culture: An Introduction to Mass Communication",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "world-regional-geography-people-places-and-globalization",
      title: "World Regional Geography: People, Places and Globalization",
      edition: "1.0",
      subject: "history"
    },
    {
      id: "writing-about-literature-through-theory",
      title: "Writing about Literature through Theory",
      edition: "1.0",
      subject: "writing"
    },
    {
      id: "writing-for-success",
      title: "Writing for Success",
      edition: "1.0",
      subject: "writing"
    }
  ];

  // src/index.ts
  var SITE = "https://saylordotorg.github.io";
  var HEADERS = { "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )" };
  var PAGE = /^s\d+[\w.-]*\.html$/;
  var LICENCE_PAGE = "s00-license.html";
  var PAGE_SIZE = 30;
  var SUBJECT_NAMES = {
    business: "Business and management",
    economics: "Economics and finance",
    law: "Law",
    social: "Psychology and sociology",
    science: "Science and mathematics",
    writing: "Writing and communication",
    history: "History, government and geography",
    arts: "Arts"
  };
  var SUBJECTS = [
    { value: "all", label: "All" },
    ...Object.entries(SUBJECT_NAMES).map(([value, label]) => ({ value, label }))
  ];
  var SORTS = [
    { value: "title", label: "Title (A–Z)" },
    { value: "relevance", label: "Best match" }
  ];
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
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
  var bookOf = (id) => {
    const book = BOOKS.find((known) => known.id === id);
    if (!book) throw new Error("Saylor Academy has no book by this name here");
    return book;
  };
  var siteOf = (book) => `${SITE}/text_${book.id}/`;
  var fetchPage = async (url) => {
    let response;
    try {
      response = await raygun.http.fetch(url, { headers: HEADERS });
    } catch {
      throw new Error("Saylor Academy's book site didn't answer; try again in a minute");
    }
    if (response.status === 404) throw new Error("Saylor Academy no longer has this page");
    if (!response.ok) {
      throw new Error(`Saylor Academy's book site answered with an error (HTTP ${response.status})`);
    }
    return { text: await response.text(), url: response.url || url };
  };
  var licenceOf = (html) => {
    const found = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d\.\d)/i.exec(html);
    return found ? `CC ${(found[1] ?? "").toUpperCase()} ${found[2] ?? ""}` : null;
  };
  var summary = (book) => ({ id: book.id, title: book.title });
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
  var search = (query, page, values) => {
    const subject = pick(values, "subject", SUBJECTS);
    const terms = words(query).split(" ").filter(Boolean);
    const found = BOOKS.filter(
      (book) => (!subject || subject === "all" || book.subject === subject) && terms.every((term) => words(`${book.title} ${SUBJECT_NAMES[book.subject]}`).includes(term))
    );
    const inTitle = (book) => terms.filter((term) => words(book.title).includes(term)).length;
    const ordered = terms.length > 0 && pick(values, "sort", SORTS) !== "title" ? [...found].sort((first, second) => inTitle(second) - inTitle(first)) : found;
    return pageOf(ordered, page);
  };
  var details = async (book) => {
    const page = await fetchPage(`${siteOf(book)}${LICENCE_PAGE}`);
    const licence = licenceOf(page.text);
    if (!licence) throw new Error("This book's licensing page doesn't name an open licence");
    const year = /Year Published:\s*(\d{4})/.exec(page.text)?.[1];
    return {
      id: book.id,
      title: book.title,
      coverUrl: null,
      authors: [],
      description: [
        `${SUBJECT_NAMES[book.subject]} textbook, version ${book.edition}, adapted and published by Saylor Academy${year ? ` in ${year}` : ""}. As its original authors asked, it doesn't name them.`,
        `Licence: ${licence}, as the book's licensing page states.`
      ].join("\n\n"),
      genres: [SUBJECT_NAMES[book.subject]],
      status: "completed",
      url: siteOf(book)
    };
  };
  var chapterList = async (book) => {
    const index = await fetchPage(`${siteOf(book)}index.html`);
    const toc = element(index.text, /<ul id="toc-top-ul"/) ?? "";
    const chapters = [];
    for (const match of toc.matchAll(/<h3><a href="([^"]+)">([\s\S]*?)<\/a><\/h3>/g)) {
      const page = decode(match[1] ?? "");
      if (!PAGE.test(page) || page === LICENCE_PAGE) continue;
      chapters.push({
        id: `${book.id}/${page}`,
        name: plainText(match[2] ?? "") || page,
        number: chapters.length + 1
      });
    }
    if (chapters.length === 0) throw new Error("This book's table of contents lists no chapters");
    return chapters;
  };
  var withNotes = (html) => {
    const notes = [];
    let text = "";
    let from = 0;
    for (; ; ) {
      const at = html.slice(from).search(/<span class="footnote"/);
      if (at < 0) break;
      const start = from + at;
      const note = element(html.slice(start), /<span/) ?? "";
      notes.push(note.replace(/^<span[^>]*>|<\/span>$/g, ""));
      text += `${html.slice(from, start)}<sup>[${notes.length}]</sup>`;
      from = start + note.length;
    }
    text += html.slice(from);
    if (notes.length === 0) return text;
    return `${text}<h3>Notes</h3>${notes.map((note, index) => `<p>[${index + 1}] ${note}</p>`).join("")}`;
  };
  var passage = async (chapterId) => {
    const slash = chapterId.indexOf("/");
    const book = bookOf(chapterId.slice(0, slash));
    const page = chapterId.slice(slash + 1);
    if (slash < 0 || !PAGE.test(page)) throw new Error("Not a chapter of a Saylor Academy book");
    const found = await fetchPage(`${siteOf(book)}${page}`);
    const content = element(found.text, /<div id="book-content"/) ?? "";
    if (/this content is permanently unavailable/i.test(content)) {
      throw new Error("Saylor Academy no longer has this chapter online");
    }
    const html = withNotes(
      dropElements(
        content,
        (tag, attributes) => tag === "script" || classes(attributes).includes("glossdef")
      )
    );
    if (!plainText(html)) throw new Error("This chapter has no text");
    return { html, baseUrl: found.url };
  };
  var filters = [
    { kind: "select", key: "subject", label: "Subject", options: SUBJECTS },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "textbooks",
      name: "Saylor Academy",
      popular: async ({ page }) => pageOf(BOOKS, page),
      search: async ({ query, page, filters: values }) => search(query, page, values),
      filters: async () => filters,
      details: ({ id }) => details(bookOf(id)),
      chapters: async ({ id }) => ({ chapters: await chapterList(bookOf(id)) }),
      passage: ({ chapterId }) => passage(chapterId)
    })
  );
})();
