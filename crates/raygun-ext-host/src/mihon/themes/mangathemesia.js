// MangaThemesia (WordPress' MangaReader/MangaStream themes), ported to Raygun's Mihon runtime
// from Keiyoushi's `lib-multisrc/mangathemesia` (MangaThemesia.kt; keiyoushi/extensions-source,
// Apache-2.0). Titles and chapters are known by their address without the domain. Raygun sends
// no view counts.
'use strict';
(() => {
  const { K, toDetails } = mihon;

  const selector = (pattern, words) =>
    words.map((word) => pattern.split('%s').join(word)).join(', ');

  const ONGOING = [
    'مستمرة',
    'en curso',
    'ongoing',
    'on going',
    'new season',
    'mass released',
    'ativo',
    'en cours',
    'en cours de publication',
    'đang tiến hành',
    'em lançamento',
    'онгоінг',
    'publishing',
    'devam ediyor',
    'em andamento',
    'in corso',
    'güncel',
    'berjalan',
    'продолжается',
    'updating',
    'lançando',
    'in arrivo',
    'emision',
    'en emision',
    'مستمر',
    'curso',
    'en marcha',
    'publicandose',
    'publicando',
    '连载中',
    'devam etmekte',
    '連載中',
  ];
  const COMPLETED = [
    'completed',
    'completo',
    'complété',
    'fini',
    'achevé',
    'terminé',
    'tamamlandı',
    'đã hoàn thành',
    'hoàn thành',
    'مكتملة',
    'завершено',
    'finished',
    'finalizad',
    'completata',
    'one-shot',
    'bitti',
    'tamat',
    'completado',
    'concluído',
    '完結',
    'concluido',
    '已完结',
    'bitmiş',
  ];
  const CANCELLED = [
    'canceled',
    'cancelled',
    'cancelado',
    'cancellato',
    'cancelados',
    'dropped',
    'discontinued',
    'abandonné',
  ];
  const HIATUS = [
    'hiatus',
    'on hold',
    'season end',
    'pausado',
    'en espera',
    'en pause',
    'en attente',
    'hiato',
  ];

  const defaults = {
    mangaUrlDirectory: () => '/manga',
    datePattern: () => 'MMMM d, yyyy',
    dateFormat: (t) => K.dateFormat(t.o.datePattern, K.locale(t.lang)),
    projectPageString: () => '/project',
    searchMangaSelector: () => '.utao .uta .imgu, .listupd .bs .bsx, .listo .bs .bsx',
    searchMangaNextPageSelector: () => 'div.pagination .next, div.hpage .r',
    seriesDetailsSelector: () => 'div.bigcontent, div.animefull, div.main-info, div.postbody',
    seriesTitleSelector: () => '.entry-title, .ts-breadcrumb li:last-child span',
    seriesArtistSelector: () =>
      selector(
        '.infotable tr:contains(%s) td:last-child, .tsinfo .imptdt:contains(%s) i, .fmed b:contains(%s)+span, span:contains(%s)',
        [
          'artist',
          'Artiste',
          'Artista',
          'الرسام',
          'الناشر',
          'İllüstratör',
          'Çizer',
          'Sanatçı',
          'นักวาด',
        ],
      ),
    seriesAuthorSelector: () =>
      selector(
        '.infotable tr:contains(%s) td:last-child, .tsinfo .imptdt:contains(%s) i, .fmed b:contains(%s)+span, span:contains(%s)',
        [
          'Author',
          'Auteur',
          'autor',
          'المؤلف',
          'Mangaka',
          'seniman',
          'Pengarang',
          'Yazar',
          'ผู้แต่ง',
          'นักเขียน',
        ],
      ),
    seriesDescriptionSelector: () => '.desc, .entry-content[itemprop=description]',
    seriesAltNameSelector: () =>
      '.alternative, .wd-full:contains(alt) span, .alter, .seriestualt, ' +
      selector('.infotable tr:contains(%s) td:last-child', [
        'Alternative',
        'Alternatif',
        'الأسماء الثانوية',
      ]),
    seriesGenreSelector: () =>
      'div.gnr a, .mgen a, .seriestugenre a, ' +
      selector('span:contains(%s)', ['genre', 'التصنيف']),
    seriesTypeSelector: () =>
      selector(
        '.infotable tr:contains(%s) td:last-child, .tsinfo .imptdt:contains(%s) i, .tsinfo .imptdt:contains(%s) a, .fmed b:contains(%s)+span, span:contains(%s) a',
        ['type', 'ประเภท', 'النوع', 'tipe', 'Türü'],
      ) + ', a[href*=type\\=]',
    seriesStatusSelector: () =>
      selector(
        '.infotable tr:contains(%s) td:last-child, .tsinfo .imptdt:contains(%s) i, .fmed b:contains(%s)+span span:contains(%s)',
        [
          'status',
          'Statut',
          'Durum',
          '連載状況',
          'Estado',
          'الحالة',
          'حالة العمل',
          'สถานะ',
          'stato',
          'Statüsü',
        ],
      ),
    seriesThumbnailSelector: () => '.infomanga > div[itemprop=image] img, .thumb img',
    altNamePrefix: () => 'Alternative Names: ',
    chapterListSelector: () =>
      'div.bxcl li, div.cl li, #chapterlist li, ul li:has(div.chbox):has(div.eph-num)',
    pageSelector: () => 'div#readerarea img',
    sendViewCount: () => true,
    statusOptions: () => [
      ['All', ''],
      ['Ongoing', 'ongoing'],
      ['Completed', 'completed'],
      ['Hiatus', 'hiatus'],
      ['Dropped', 'dropped'],
    ],
    typeFilterOptions: () => [
      ['All', ''],
      ['Manga', 'Manga'],
      ['Manhwa', 'Manhwa'],
      ['Manhua', 'Manhua'],
      ['Comic', 'Comic'],
    ],
    orderByFilterOptions: () => [
      ['Default', ''],
      ['A-Z', 'title'],
      ['Z-A', 'titlereverse'],
      ['Latest Update', 'update'],
      ['Latest Added', 'latest'],
      ['Popular', 'popular'],
    ],
    projectFilterOptions: () => [
      ['Show all manga', ''],
      ['Show only project manga', 'project-filter-on'],
    ],
    hasProjectPage: () => false,
    supportsFilterFetching: () => true,
    supportsLatest: () => true,
    listUrl: (t) => `${t.o.mangaUrlDirectory}/list-mode/`,
    listSelector: () => 'div#content div.soralist ul li a.series',
    slugRegex: () => K.regex('^(\\d+-)'),
  };

  const messages = {
    alt_names_heading: 'Alternative Names:',
    status_filter_option_all: 'All',
    status_filter_option_ongoing: 'Ongoing',
    status_filter_option_completed: 'Completed',
    status_filter_option_hiatus: 'Hiatus',
    status_filter_option_dropped: 'Dropped',
    type_filter_option_all: 'All',
    type_filter_option_manga: 'Manga',
    type_filter_option_manhwa: 'Manhwa',
    type_filter_option_manhua: 'Manhua',
    type_filter_option_comic: 'Comic',
    order_by_filter_default: 'Default',
    order_by_filter_az: 'A-Z',
    order_by_filter_za: 'Z-A',
    order_by_filter_latest_update: 'Latest Update',
    order_by_filter_latest_added: 'Latest Added',
    order_by_filter_popular: 'Popular',
    project_filter_all_manga: 'Show all manga',
    project_filter_only_project: 'Show only project manga',
  };

  const IMAGE_LIST = /["']?(?:images|imageUrls)["']?\s*[:=]\s*(\[.*?])/s;
  const ALT_NAME_SEPARATOR = /[|/•,;]/;

  const make = (t) => {
    const o = t.o;
    const http = t.http;
    const pageHeaders = { Referer: `${t.baseUrl}/` };

    const imgAttr = (element) => {
      if (!element) return '';
      if (element.hasAttr('data-lazy-src')) return element.attr('abs:data-lazy-src');
      if (element.hasAttr('data-src')) return element.attr('abs:data-src');
      if (element.hasAttr('data-cfsrc')) return element.attr('abs:data-cfsrc');
      return element.attr('abs:src');
    };
    const imgsAttr = (elements) => imgAttr(elements.first());

    const placeholder = (value) =>
      value == null || K.isBlank(value) || ['-', 'N/A', 'n/a', 'Unknown'].includes(value)
        ? null
        : value;

    const parseStatus = (text) => {
      if (text == null) return 0;
      const lower = text.toLowerCase();
      const has = (list) => list.some((word) => lower.includes(word.toLowerCase()));
      if (has(ONGOING)) return 1;
      if (has(COMPLETED)) return 2;
      if (has(CANCELLED)) return 5;
      if (has(HIATUS)) return 6;
      return 0;
    };

    // MangaThemesiaAlt: slugs carry a number that changes (`/manga/1234-title/`); titles are
    // known by the slug without it, looked up again in the site's list when they're read.
    const alt = t.variant === 'alt';
    const slugOf = (path) => path.replace(/\/+$/, '').split('/').pop() ?? '';
    const permanent = (path) =>
      alt ? `${o.mangaUrlDirectory}/${slugOf(path).replace(o.slugRegex, '')}/` : path;
    let slugs = null;
    let slugsAt = 0;
    const currentSlugs = async () => {
      if (slugs === null || Date.now() - slugsAt > 3600000) {
        const document = await http.document(`${t.baseUrl}${o.listUrl}`);
        slugs = new Map(
          document.select(o.listSelector).map((link) => {
            const slug = slugOf(link.absUrl('href'));
            return [slug.replace(o.slugRegex, ''), slug];
          }),
        );
        slugsAt = Date.now();
      }
      return slugs;
    };
    /** Where a title is now: its address with the slug the site's list gives today. */
    const current = async (id) => {
      if (!alt || !id.startsWith(o.mangaUrlDirectory)) return id;
      const slug = (await currentSlugs()).get(slugOf(id));
      return slug ? `${o.mangaUrlDirectory}/${slug}/` : id;
    };

    const searchMangaParse = (document) => {
      const items = document
        .select(o.searchMangaSelector)
        .map((element) => {
          const links = element.select('a');
          const href = links.attr('href');
          const cover = imgsAttr(element.select('img'));
          return {
            id: permanent(K.withoutDomain(K.resolve(href, document.location() || `${t.baseUrl}/`))),
            title: links.attr('title') || links.text(),
            coverUrl: cover || undefined,
            coverHeaders: pageHeaders,
          };
        })
        .filter((manga) => manga.id && manga.title);
      const next = o.searchMangaNextPageSelector;
      return { items, hasNextPage: next ? document.select(next).first() !== null : false };
    };

    const searchUrl = (page, query, filters) => {
      let path = o.mangaUrlDirectory.replace(/^\//, '');
      const params = [];
      if (query) params.push(['title', query]);
      params.push(['page', String(page)]);
      if (filters.status !== undefined) params.push(['status', filters.status]);
      if (filters.type !== undefined) params.push(['type', filters.type]);
      if (filters.order !== undefined) params.push(['order', filters.order]);
      for (const genre of filters.include ?? []) params.push(['genre[]', genre]);
      for (const genre of filters.exclude ?? []) params.push(['genre[]', `-${genre}`]);
      if (filters.project === 'project-filter-on') path = o.projectPageString.replace(/^\//, '');
      return K.query(`${t.baseUrl}/${path}/`, params);
    };

    const search = async (page, query, filters) =>
      searchMangaParse(await http.document(searchUrl(page, query, filters)));

    const parseChapterDate = (text) => {
      if (text == null) return 0;
      const parsed =
        o.dateFormat.parse(text) ?? K.dateFormat(o.datePattern, K.locale('en')).parse(text);
      return parsed ?? 0;
    };

    const chapterListParse = (document) => {
      const chapters = document
        .select(o.chapterListSelector)
        .map((element) => {
          const links = element.select('a');
          const href = links.attr('href');
          const name = element.select('.lch a, .chapternum').text() || links.first()?.text() || '';
          const date = parseChapterDate(element.selectFirst('.chapterdate')?.text() ?? null);
          return {
            id: K.withoutDomain(K.resolve(href, document.location() || `${t.baseUrl}/`)),
            name: name.trim() || undefined,
            number: (() => {
              const found = (element.selectFirst('[data-num]')?.attr('data-num') || name).match(
                /(\d+(?:\.\d+)?)/,
              );
              return found ? parseFloat(found[1]) : undefined;
            })(),
            dateUpload: date > 0 ? date : undefined,
          };
        })
        .filter((chapter) => chapter.id && chapter.id !== '/');
      // The newest chapter takes the "Updated on" date when none has its own.
      if (chapters.length && chapters[0].dateUpload === undefined) {
        const updated = document
          .select(
            '.listinfo time[itemprop=dateModified], .fmed:contains(update) time, span:contains(update) time',
          )
          .attr('datetime');
        const day = updated.match(/^(\d{4})-(\d{2})-(\d{2})/);
        if (day)
          chapters[0].dateUpload = Date.UTC(
            parseInt(day[1], 10),
            parseInt(day[2], 10) - 1,
            parseInt(day[3], 10),
          );
      }
      return chapters;
    };

    const mangaDetailsParse = (document, id) => {
      const details = document.selectFirst(o.seriesDetailsSelector);
      if (!details)
        throw new Error(
          "the title's page has no details the MangaThemesia theme reads (the site changed?)",
        );
      const title = details.selectFirst(o.seriesTitleSelector);
      if (!title)
        throw new Error(
          "the title's page has no title the MangaThemesia theme reads (the site changed?)",
        );
      const manga = {
        title: title.text(),
        artist: placeholder(details.selectFirst(o.seriesArtistSelector)?.ownText() ?? null),
        author: placeholder(details.selectFirst(o.seriesAuthorSelector)?.ownText() ?? null),
        description: details.selectFirst(o.seriesDescriptionSelector)?.text() || null,
      };
      const alternative = details.selectFirst(o.seriesAltNameSelector)?.ownText();
      if (alternative && !K.isBlank(alternative)) {
        const names = alternative
          .split(ALT_NAME_SEPARATOR)
          .map((name) => `- ${name.trim()}`)
          .join('\n');
        manga.description =
          (manga.description ? `${manga.description}\n\n` : '') +
          `${o.altNamePrefix}\n${names}`.trim();
      }
      const genres = details.select(o.seriesGenreSelector).map((element) => element.text());
      const type = details.selectFirst(o.seriesTypeSelector)?.ownText();
      if (type && !K.isBlank(type)) genres.push(type);
      manga.genre = genres
        .map((genre) => {
          const lower = genre.toLowerCase().trim();
          return lower.charAt(0).toUpperCase() + lower.slice(1);
        })
        .join(', ');
      manga.status = parseStatus(details.selectFirst(o.seriesStatusSelector)?.text() ?? null);
      manga.thumbnail_url = imgsAttr(details.select(o.seriesThumbnailSelector)) || null;
      manga.webUrl = document.location();
      return toDetails(t, id, manga);
    };

    const pageListParse = (document) => {
      const pages = document
        .select(o.pageSelector)
        .map(imgAttr)
        .filter((address) => address !== '');
      if (pages.length) return pages.map((address) => ({ url: address, headers: pageHeaders }));
      // Some sites load their pages with a script.
      const script = document.selectFirst(
        'script[src^=data:text/javascript;base64,dHNfcmVhZGVyLnJ1bih7]',
      );
      const text = script
        ? K.base64Decode(K.substringAfter(script.attr('src'), 'base64,'))
        : document.outerHtml();
      const found = text.match(IMAGE_LIST);
      let list = [];
      try {
        list = found ? JSON.parse(found[1]) : [];
      } catch {
        list = [];
      }
      return list
        .map((address) => K.str(address))
        .map((address) => (address.startsWith('/') ? t.baseUrl + address : address))
        .map((address) => ({ url: address, headers: pageHeaders }));
    };

    const pageCache = new Map();
    const mangaPage = async (id) => {
      const cached = pageCache.get(id);
      if (cached && Date.now() - cached.at < 60000) return cached.document;
      const document = await http.document(K.resolve(await current(id), `${t.baseUrl}/`));
      pageCache.set(id, { at: Date.now(), document });
      if (pageCache.size > 8) pageCache.delete(pageCache.keys().next().value);
      return document;
    };

    let genres = null;
    const knownGenres = async () => {
      if (genres === null && o.supportsFilterFetching) {
        try {
          const document = await http.document(
            `${t.baseUrl}/${o.mangaUrlDirectory.replace(/^\//, '')}`,
          );
          genres =
            document
              .selectFirst('ul.genrez')
              ?.select('li')
              .map((item) => ({
                label: item.selectFirst('label')?.text() ?? '',
                value: item.selectFirst('input[type=checkbox]')?.attr('value') ?? '',
              }))
              .filter((genre) => genre.label && genre.value) ?? [];
        } catch {
          return [];
        }
      }
      return genres ?? [];
    };

    const optionList = (pairs) =>
      pairs.map(([label, value]) => ({ value: value === '' ? '_' : value, label }));
    const chosen = (value) => (value === '_' ? '' : value);

    const calls = {
      popular: ({ page = 1 }) => search(page, '', { order: 'popular' }),
      latest: ({ page = 1 }) => search(page, '', { order: 'update' }),
      filters: async () => {
        const filters = [
          { key: 'status', label: 'Status', kind: 'select', options: optionList(o.statusOptions) },
          { key: 'type', label: 'Type', kind: 'select', options: optionList(o.typeFilterOptions) },
          {
            key: 'order',
            label: 'Sort By',
            kind: 'select',
            options: optionList(o.orderByFilterOptions),
          },
        ];
        const known = await knownGenres();
        if (known.length)
          filters.push({ key: 'genres', label: 'Genre', kind: 'tags', options: known });
        if (o.hasProjectPage)
          filters.push({
            key: 'project',
            label: 'Filter Project',
            kind: 'select',
            options: optionList(o.projectFilterOptions),
          });
        return filters;
      },
      search: ({ query = '', page = 1, filters = {} }) =>
        search(page, query, {
          status: chosen(filters.status ?? '_'),
          type: chosen(filters.type ?? '_'),
          order: chosen(filters.order ?? '_'),
          include: filters.genres?.include ?? [],
          exclude: filters.genres?.exclude ?? [],
          project: chosen(filters.project ?? '_'),
        }),
      details: async ({ id }) => mangaDetailsParse(await mangaPage(id), id),
      chapters: async ({ id }) => ({ chapters: chapterListParse(await mangaPage(id)) }),
      pages: async ({ chapterId }) => ({
        pages: pageListParse(await http.document(K.resolve(chapterId, `${t.baseUrl}/`))),
      }),
    };
    if (!o.supportsLatest) delete calls.latest;
    return calls;
  };

  mihon.defineTheme('mangathemesia', { defaults, make, messages });
})();
