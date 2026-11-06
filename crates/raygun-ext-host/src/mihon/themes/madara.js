// Madara (WordPress' Madara manga theme), ported to Raygun's Mihon runtime from Keiyoushi's
// `lib-multisrc/madara` (MadaraBase.kt, Madara.kt, MadaraNoAjax.kt; keiyoushi/extensions-source,
// Apache-2.0). Variants: `ajax` lists through `admin-ajax.php`'s `madara_load_more`, `noAjax`
// reads the archive pages. Titles are known by their path (`/manga/one/`), chapters by theirs.
// Raygun sends no view counts.
'use strict';
(() => {
  const { K, Jsoup, toDetails } = mihon;

  const COMPLETED = [
    'completed',
    'completo',
    'completado',
    'concluído',
    'concluido',
    'finalizado',
    'achevé',
    'terminé',
    'hoàn thành',
    'مكتملة',
    'مكتمل',
    '已完结',
    'tamamlandı',
    'đã hoàn thành',
    'завершено',
    'tamamlanan',
    'complété',
  ];
  const ONGOING = [
    'ongoing',
    'on going',
    'updating',
    'продолжается',
    'em lançamento',
    'em andamento',
    'en cours',
    'ativo',
    'lançando',
    'đang tiến hành',
    'còn nữa',
    'devam ediyor',
    'in corso',
    'in arrivo',
    'مستمرة',
    'مستمر',
    'en curso',
    'emision',
    'curso',
    'en marcha',
    'publicandose',
    'publicándose',
    'en emision',
    '连载中',
    'đang làm',
    'em postagem',
    'devam eden',
    'em progresso',
    'atualizações semanais',
    'виходить',
    'đang ra',
  ];
  const HIATUS = [
    'on hold',
    'hiatus',
    'pausado',
    'en espera',
    'durduruldu',
    'beklemede',
    'đang chờ',
    'متوقف',
    'en pause',
    'заморожено',
    'en attente',
    'tạm dừng',
  ];
  const CANCELLED = [
    'canceled',
    'cancelled',
    'cancelado',
    'iptal edildi',
    'đã hủy',
    'ملغي',
    'abandonné',
    'заброшено',
    'annulé',
  ];

  const defaults = {
    supportsPostId: () => true,
    chapterMode: () => 'MangaPage',
    mangaSubString: () => 'manga',
    genreDirectory: () => 'manga-genre',
    filterNonMangaItems: () => true,
    sendViewCount: () => true,
    archiveSelector: () => 'div.page-item-detail, .manga__item, .c-tabs-item__content',
    searchCardSelector: () => '.c-tabs-item__content',
    archiveTitleSelector: () => null,
    archiveUrlSelector: () => '.post-title a',
    mangaDetailsSelectorTitle: () => 'div.post-title h3, div.post-title h1, #manga-title > h1',
    mangaDetailsSelectorAuthor: () => 'div.author-content > a, div.manga-authors > a',
    mangaDetailsSelectorArtist: () => 'div.artist-content > a',
    mangaDetailsSelectorStatus: () =>
      'div.summary-content, div.summary-heading:contains(Status) + div',
    mangaDetailsSelectorDescription: () =>
      'div.description-summary div.summary__content, div.summary_content div.post-content_item > h5 + div, div.summary_content div.manga-excerpt',
    mangaDetailsSelectorThumbnail: () => 'div.summary_image img',
    mangaDetailsSelectorGenre: () => 'div.genres-content a',
    mangaDetailsSelectorTag: () => 'div.tags-content a',
    seriesTypeSelector: () => '.post-content_item:contains(Type) .summary-content',
    altNameSelector: () => '.post-content_item:contains(Alt) .summary-content',
    updatingRegex: () => K.regex('Updating|Atualizando', 'i'),
    chapterListSelector: () => 'li.wp-manga-chapter',
    chapterNameSelector: () => null,
    chapterUrlSelector: () => 'a',
    chapterDateSelector: () => 'span.chapter-release-date',
    pageListParseSelector: () =>
      'div.page-break, li.blocks-gallery-item, .reading-content .text-left:not(:has(.blocks-gallery-item))',
    filterGenresSelector: () => 'div.genres',
    chapterDateFormat: () => K.dateFormat('MMMM d, yyyy', K.locale('en')),
    completedStatus: () => COMPLETED,
    ongoingStatus: () => ONGOING,
    hiatusStatus: () => HIATUS,
    cancelledStatus: () => CANCELLED,
    statusFilterOptions: () => [
      ['Completed', 'end'],
      ['Ongoing', 'on-going'],
      ['Canceled', 'canceled'],
      ['On Hold', 'on-hold'],
    ],
    orderByFilterOptions: () => [
      ['Relevance', ''],
      ['Latest', 'latest'],
      ['A-Z', 'alphabet'],
      ['Rating', 'rating'],
      ['Trending', 'trending'],
      ['Most Views', 'views'],
      ['New', 'new-manga'],
    ],
    genreConditionFilterOptions: () => [
      ['OR', ''],
      ['AND', '1'],
    ],
    adultFilterOptions: () => [
      ['All', ''],
      ['None', '0'],
      ['Only', '1'],
    ],
    pageSize: () => 25,
    ajaxTemplate: () => 'madara-core/content/content-archive',
    nextPageSelector: () => 'div.nav-previous, a.nextpostslink, #navigation-ajax',
    orderQueryParameter: () => 'm_orderby',
    searchQueryParameter: () => 's',
    supportsLatest: () => true,
    supportsFilterFetching: () => true,
  };

  const ARABIC_DUALS = [
    ['ثانيتين', '2 ثوان'],
    ['دقيقتين', '2 دقائق'],
    ['ساعتين', '2 ساعات'],
    ['يومين', '2 أيام'],
    ['أسبوعين', '2 أسابيع'],
    ['شهرين', '2 أشهر'],
    ['سنتين', '2 سنوات'],
  ];
  // Keywords must not follow a letter, or "ay" would match inside "mayıs" and "days".
  const word = (...words) =>
    new RegExp(
      `(?<!\\p{L})(?:${words.map((one) => one.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})`,
      'u',
    );
  const UNITS = [
    [word('year', 'año', 'ano', 'năm', 'yıl', 'سنة', 'سنوات'), 365 * 86400000],
    [word('month', 'mes', 'tháng', 'ay', 'شهر', 'أشهر', 'شهور'), 30 * 86400000],
    [word('week', 'semana', 'tuần', 'hafta', 'أسبوع', 'أسابيع'), 7 * 86400000],
    [word('day', 'día', 'dia', 'jour', 'hari', 'gün', 'ngày', 'giorni', 'أيام', '天'), 86400000],
    [word('hour', 'hora', 'heure', 'jam', 'saat', 'giờ', 'ore', 'ساعة', 'ساعات', '小时'), 3600000],
    [word('minute', 'minuto', 'min', 'menit', 'dakika', 'phút', 'دقيقة', 'دقائق'), 60000],
    [word('second', 'segundo', 'sec', 'detik', 'giây', 'ثانية', 'ثوان'), 1000],
  ];

  const startOfDayUtc = (ms) => {
    const date = new Date(ms);
    return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
  };

  /** A chapter number from its name ("Chapter 12.5"), when it says one. */
  const chapterNumber = (name) => {
    const found = K.str(name).match(
      /(?:chapter|chap|ch\.?|cap[ií]tulo|cap\.?|episode|ep\.?|chương|bölüm|глава|الفصل|ตอนที่|第)\s*(\d+(?:[.,]\d+)?)/iu,
    );
    return found ? parseFloat(found[1].replace(',', '.')) : undefined;
  };

  const make = (t) => {
    const o = t.o;
    const http = t.http;
    const xhr = { 'X-Requested-With': 'XMLHttpRequest' };

    const isUpdating = (value) => o.updatingRegex.test(value);

    const toStatus = (text) => {
      const lower = K.str(text).toLowerCase();
      const has = (list) => list.some((one) => lower.includes(one.toLowerCase()));
      if (has(o.completedStatus)) return 2;
      if (has(o.ongoingStatus)) return 1;
      if (has(o.hiatusStatus)) return 6;
      if (has(o.cancelledStatus)) return 5;
      return 0;
    };

    const srcSetImage = (text, base) => {
      const images = text
        .split(',')
        .map((part) => part.trim().split(/\s+/, 2))
        // Each candidate resolved against the page (Jsoup resolved the whole list as one).
        .map((candidate) => [K.resolve(candidate[0], base), candidate[1]])
        .filter(
          (candidate) => candidate.length && /^https?:\/\/[^\s/$.?#].[^\s]*$/.test(candidate[0]),
        );
      let best = null;
      for (const candidate of images) {
        const descriptor = candidate[1] && candidate[1].match(/^(\d+|\d+\.\d+)[wx]$/);
        if (descriptor && (!best || parseFloat(descriptor[1]) > best[1]))
          best = [candidate[0], parseFloat(descriptor[1])];
      }
      if (best) return best[0];
      return (
        images
          .map((candidate) => candidate[0])
          .sort()
          .pop() ?? null
      );
    };

    const imageFromElement = (element) => {
      if (element.hasAttr('data-src')) return element.attr('abs:data-src');
      if (element.hasAttr('data-lazy-src')) return element.attr('abs:data-lazy-src');
      if (element.hasAttr('data-lzl-src')) return element.attr('abs:data-lzl-src');
      if (element.hasAttr('data-cfsrc')) return element.attr('abs:data-cfsrc');
      if (element.hasAttr('data-manga-src')) return element.attr('abs:data-manga-src');
      if (element.hasAttr('srcset'))
        return srcSetImage(element.attr('srcset'), element.doc?.location() ?? `${t.baseUrl}/`);
      return element.attr('abs:src');
    };

    const pathOf = (href) => {
      try {
        return K.url(href).path;
      } catch {
        return null;
      }
    };

    const postId = (element) =>
      element.attr('data-post-id') ||
      element.selectFirst('[data-post-id]')?.attr('data-post-id') ||
      '';

    const archiveManga = (element) => {
      const link = element.selectFirst(o.archiveUrlSelector);
      if (!link) return null;
      const href = link.attr('abs:href');
      const path = href ? pathOf(href) : null;
      if (!path) return null;
      const titleSelector = o.archiveTitleSelector;
      const title =
        (titleSelector ? element.selectFirst(titleSelector)?.text() : null) ?? link.text();
      const image = element.selectFirst('img');
      return {
        id: path,
        title,
        coverUrl: image ? imageFromElement(image) || undefined : undefined,
        coverHeaders: { Referer: `${t.baseUrl}/` },
      };
    };

    const parseArchive = (document) =>
      document
        .select(o.archiveSelector)
        .filter((element) => !o.supportsPostId || postId(element) !== '')
        .map(archiveManga)
        .filter((manga) => manga && manga.title);

    const parseSearchCards = (document) =>
      document
        .select(o.searchCardSelector)
        .map(archiveManga)
        .filter((manga) => manga && manga.title);

    const parseChapterDate = (date) => {
      if (date == null) return 0;
      let value = K.str(date);
      for (const [dual, replacement] of ARABIC_DUALS) value = value.split(dual).join(replacement);
      const normalized = value.toLowerCase();
      const today = startOfDayUtc(Date.now());
      if (['today', 'hoje', 'hoy'].some((start) => normalized.startsWith(start))) return today;
      if (['yesterday', 'ontem', 'ayer', 'يوم واحد'].some((start) => normalized.startsWith(start)))
        return today - 86400000;
      const amount = normalized.match(/\d+/);
      if (amount) {
        const unit = UNITS.find(([pattern]) => pattern.test(normalized));
        if (unit) return Date.now() - parseInt(amount[0], 10) * unit[1];
      }
      const parsed = o.chapterDateFormat.parse(value);
      return parsed == null ? 0 : startOfDayUtc(parsed);
    };

    const chapterFromElement = (element, mangaPath) => {
      const link = element.selectFirst(o.chapterUrlSelector);
      if (!link) return null;
      const href = link.attr('abs:href');
      const path = href ? pathOf(href) : null;
      const slug = path ? path.replace(/\/+$/, '').split('/').pop() : '';
      if (!slug) return null;
      const nameSelector = o.chapterNameSelector;
      const name = (
        (nameSelector ? element.selectFirst(nameSelector)?.text() : null) ?? link.text()
      ).trim();
      const dateText =
        element.selectFirst('img:not(.thumb)')?.attr('alt') ||
        element.selectFirst('span a')?.attr('title') ||
        element.selectFirst(o.chapterDateSelector)?.text();
      const dateUpload = parseChapterDate(dateText || null);
      return {
        id: `${mangaPath.replace(/\/+$/, '')}/${slug}/`,
        name: name || undefined,
        number: chapterNumber(name),
        dateUpload: dateUpload > 0 ? dateUpload : undefined,
      };
    };

    const parseChapterList = (document, mangaPath) =>
      document
        .select(o.chapterListSelector)
        .map((element) => chapterFromElement(element, mangaPath))
        .filter(Boolean);

    const mangaIdOf = (document) =>
      document.selectFirst('[id^=manga-chapters-holder]')?.attr('data-id') ||
      document.selectFirst('input.rating-post-id')?.attr('value') ||
      document.selectFirst('a[data-post]')?.attr('data-post') ||
      (() => {
        const shortlink = document.selectFirst('link[rel=shortlink]')?.attr('href') ?? '';
        return shortlink.includes('?p=')
          ? K.substringBefore(K.substringAfter(shortlink, '?p='), '&')
          : '';
      })();

    const pageCache = new Map();
    /** The title's page; details and chapters both read it, a minute apart at most. */
    const mangaPage = async (path) => {
      const cached = pageCache.get(path);
      if (cached && Date.now() - cached.at < 60000) return cached.document;
      const document = await http.document(K.resolve(path, `${t.baseUrl}/`));
      pageCache.set(path, { at: Date.now(), document });
      if (pageCache.size > 8) pageCache.delete(pageCache.keys().next().value);
      return document;
    };

    const ajaxChapters = async (path) => {
      const base = `${t.baseUrl}${path.replace(/\/+$/, '')}/ajax/chapters/`;
      const from = K.resolve(path, `${t.baseUrl}/`);
      switch (o.chapterMode) {
        case 'AdminAjax': {
          const document = await mangaPage(path);
          const id = mangaIdOf(document);
          if (!id) throw new Error('the title page has no Madara post id');
          const answer = await http.post(`${t.baseUrl}/wp-admin/admin-ajax.php`, {
            kind: 'xhr',
            from,
            headers: xhr,
            form: [
              ['action', 'manga_get_chapters'],
              ['manga', id],
            ],
          });
          return parseChapterList(await answer.document(), path);
        }
        case 'MangaAjax': {
          const answer = await http.post(base, { kind: 'xhr', from, headers: xhr, form: [] });
          return parseChapterList(await answer.document(), path);
        }
        case 'MangaAjaxPaginated': {
          const result = [];
          let last = null;
          for (let page = 1; page < 200; page++) {
            const answer = await http.post(`${base}?t=${page}`, {
              kind: 'xhr',
              from,
              headers: xhr,
              form: [],
              ensureSuccess: false,
            });
            if (answer.code === 404) return result;
            if (!answer.isSuccessful) throw new mihon.HttpError(answer.code, answer.url);
            const chapters = parseChapterList(await answer.document(), path);
            if (!chapters.length || chapters[chapters.length - 1].id === last) break;
            result.push(...chapters);
            last = chapters[chapters.length - 1].id;
          }
          return result;
        }
        case 'MangaAjaxQuery': {
          const slug = path.replace(/\/+$/, '').split('/').pop();
          const answer = await http.post(`${t.baseUrl}/index.php`, {
            kind: 'xhr',
            from,
            headers: xhr,
            form: [
              ['manga-core', slug],
              ['manga_ajax', '1'],
              ['maction', 'get_chapters'],
            ],
          });
          return parseChapterList(await answer.document(), path);
        }
        default:
          return parseChapterList(await mangaPage(path), path);
      }
    };

    const parseDetails = (document, id) => {
      const titleElement = document.selectFirst(o.mangaDetailsSelectorTitle);
      if (!titleElement)
        throw new Error("the title's page has no title the Madara theme reads (the site changed?)");
      const texts = (selector) =>
        document
          .select(selector)
          .eachText()
          .filter((text) => !isUpdating(text));
      const manga = {
        title: titleElement.ownText() || titleElement.text(),
        author: texts(o.mangaDetailsSelectorAuthor).join(', ') || null,
        artist: texts(o.mangaDetailsSelectorArtist).join(', ') || null,
      };
      const description = document.selectFirst(o.mangaDetailsSelectorDescription);
      if (description) {
        const paragraphs = description.select('p');
        manga.description = paragraphs.length
          ? paragraphs.map((p) => p.text()).join('\n\n')
          : description.text();
      }
      const alternative = document.selectFirst(o.altNameSelector)?.ownText();
      if (alternative && !isUpdating(alternative)) {
        manga.description = [manga.description, `Alternative Names: ${alternative}`]
          .filter(Boolean)
          .join('\n\n');
      }
      const thumbnail = document.selectFirst(o.mangaDetailsSelectorThumbnail);
      if (thumbnail) manga.thumbnail_url = imageFromElement(thumbnail);
      const status = document.select(o.mangaDetailsSelectorStatus).last();
      manga.status = status ? toStatus(status.text()) : 0;
      const genres = [
        ...document.select(o.mangaDetailsSelectorGenre).map((element) => element.text()),
        ...document.select(o.mangaDetailsSelectorTag).eachText(),
      ];
      const type = document.selectFirst(o.seriesTypeSelector)?.text();
      if (type) genres.push(type);
      const seen = new Set();
      manga.genre = genres
        .filter((genre) => genre && !seen.has(genre.toLowerCase()) && seen.add(genre.toLowerCase()))
        .join(', ');
      manga.webUrl = document.location();
      return toDetails(t, id, manga);
    };

    const parsePages = (document) => {
      const location = document.location();
      const headers = { Referer: location };
      const protector = document.selectFirst('#chapter-protector-data');
      if (!protector) {
        return document
          .select(o.pageListParseSelector)
          .map((element) => element.selectFirst('img'))
          .filter(Boolean)
          .map((image) => imageFromElement(image))
          .filter(Boolean)
          .map((address) => ({ url: address.trim(), headers }));
      }
      const src = protector.attr('src');
      const script = src.startsWith('data:text/javascript;base64,')
        ? K.base64Decode(src.slice(src.indexOf(',') + 1))
        : protector.data() || protector.html();
      const password = K.substringBefore(K.substringAfter(script, "wpmangaprotectornonce='"), "';");
      const encrypted = K.replace(
        K.substringBefore(K.substringAfter(script, "chapter_data='"), "';"),
        '\\/',
        '/',
      );
      const data = JSON.parse(encrypted);
      const raw = K.decrypt({ data: data.ct, password, salt: data.s });
      const list = JSON.parse(JSON.parse(raw));
      return list.map((address) => ({ url: K.str(address), headers }));
    };

    const pageList = async (chapterId) => {
      const chapterUrl = K.resolve(chapterId, `${t.baseUrl}/`);
      let document = await http.document(chapterUrl);
      if (document.selectFirst('#single-pager')) {
        document = await http.document(K.query(chapterUrl, [['style', 'list']]));
      }
      return parsePages(document);
    };

    const genreRoutes = async () => {
      const document = await http.document(`${t.baseUrl}/${o.mangaSubString}/`);
      const seen = new Set();
      return document
        .select(o.filterGenresSelector)
        .select(`a[href*='/${o.genreDirectory}/']`)
        .map((element) => {
          const href = element.attr('abs:href');
          const path = href ? pathOf(href) : null;
          const name = element.text();
          const slug = path ? path.replace(/\/+$/, '').split('/').pop() : '';
          return path && name && slug ? { name, slug, path } : null;
        })
        .filter((route) => route && !seen.has(route.slug) && seen.add(route.slug));
    };
    let genres = null;
    const knownGenres = async () => {
      if (genres === null && o.supportsFilterFetching) {
        try {
          genres = await genreRoutes();
        } catch {
          return [];
        }
      }
      return genres ?? [];
    };

    const optionList = (pairs) =>
      pairs.map(([label, value]) => ({ value: value === '' ? '_' : value, label }));
    const chosen = (value) => (value === '_' ? '' : value);

    const listing = (items, hasNextPage) => ({ items, hasNextPage });

    let calls;
    if (t.variant === 'noAjax') {
      const archivePage = async (page, order, path = `/${o.mangaSubString}/`, query = '') => {
        let address = K.resolve(path, `${t.baseUrl}/`);
        if (page > 1) address = `${address.replace(/\/?$/, '/')}page/${page}/`;
        const params = [];
        if (order) params.push([o.orderQueryParameter, order]);
        if (query) params.push([o.searchQueryParameter, query]);
        const document = await http.document(K.query(address, params));
        return listing(parseArchive(document), document.selectFirst(o.nextPageSelector) !== null);
      };
      calls = {
        popular: ({ page = 1 }) => archivePage(page, 'views'),
        latest: ({ page = 1 }) => archivePage(page, 'latest'),
        filters: async () => {
          const routes = await knownGenres();
          const filters = [
            {
              key: 'sort',
              label: 'Order By',
              kind: 'select',
              options: optionList(o.orderByFilterOptions),
            },
          ];
          if (routes.length) {
            filters.push({
              key: 'genre',
              label: 'Genres',
              kind: 'select',
              options: [
                { value: '_', label: 'All' },
                ...routes.map((route) => ({ value: route.path, label: route.name })),
              ],
            });
          }
          return filters;
        },
        search: async ({ query = '', page = 1, filters = {} }) => {
          const sort = chosen(filters.sort ?? '');
          const genre = chosen(filters.genre ?? '');
          if (genre) return archivePage(page, sort, genre, query);
          if (!query.trim()) return archivePage(page, sort);
          let address = `${t.baseUrl}/`;
          if (page > 1) address += `page/${page}/`;
          const document = await http.document(
            K.query(address, [
              [o.searchQueryParameter, query],
              ['post_type', 'wp-manga'],
            ]),
          );
          const next = document.selectFirst(o.nextPageSelector) !== null;
          const archive = parseArchive(document);
          return listing(archive.length ? archive : parseSearchCards(document), next);
        },
      };
    } else {
      const ajaxList = async (page, mode, query = '', filters = {}) => {
        const form = [
          ['action', 'madara_load_more'],
          ['page', String(page - 1)],
          ['template', o.ajaxTemplate],
          ['vars[paged]', '1'],
          ['vars[template]', 'archive'],
          ['vars[posts_per_page]', String(o.pageSize)],
          ['vars[post_type]', 'wp-manga'],
          ['vars[post_status]', 'publish'],
          ['vars[manga_archives_item_layout]', 'big_thumbnail'],
        ];
        if (o.filterNonMangaItems) {
          form.push(
            ['vars[meta_query][0][key]', '_wp_manga_chapter_type'],
            ['vars[meta_query][0][value]', 'manga'],
          );
        }
        const sort = (key) =>
          form.push(
            ['vars[orderby]', 'meta_value_num'],
            ['vars[meta_key]', key],
            ['vars[order]', 'DESC'],
          );
        if (mode === 'popular') sort('_wp_manga_views');
        else if (mode === 'latest') sort('_latest_update');
        else {
          if (query.trim()) form.push(['vars[s]', query]);
          let meta = o.filterNonMangaItems ? 1 : 0;
          let taxonomy = 0;
          const statuses = filters.status?.include ?? [];
          if (statuses.length) {
            form.push(
              [`vars[meta_query][${meta}][key]`, '_wp_manga_status'],
              [`vars[meta_query][${meta}][compare]`, 'IN'],
            );
            statuses.forEach((state, index) =>
              form.push([`vars[meta_query][${meta}][value][${index}]`, state]),
            );
            meta++;
          }
          switch (chosen(filters.sort ?? '')) {
            case 'latest':
              sort('_latest_update');
              break;
            case 'alphabet':
              form.push(['vars[orderby]', 'post_title'], ['vars[order]', 'ASC']);
              break;
            case 'rating':
              form.push(
                ['vars[meta_query][query_average_reviews][key]', '_manga_avarage_reviews'],
                ['vars[meta_query][query_average_reviews][compare]', 'EXISTS'],
                ['vars[meta_query][query_total_reviews][key]', '_manga_total_votes'],
                ['vars[meta_query][query_total_reviews][compare]', 'EXISTS'],
                ['vars[orderby][query_average_reviews]', 'DESC'],
                ['vars[orderby][query_total_reviews]', 'DESC'],
              );
              break;
            case 'trending':
              sort('_wp_manga_week_views_value');
              break;
            case 'views':
              sort('_wp_manga_views');
              break;
            case 'new-manga':
              form.push(['vars[orderby]', 'date'], ['vars[order]', 'DESC']);
              break;
            default:
          }
          const adult = chosen(filters.adult ?? '');
          if (adult) {
            form.push(
              [`vars[meta_query][${meta}][key]`, 'manga_adult_content'],
              [`vars[meta_query][${meta}][compare]`, adult === '0' ? 'not exists' : 'exists'],
            );
            meta++;
          }
          const genreSlugs = filters.genres?.include ?? [];
          if (genreSlugs.length) {
            if (chosen(filters.condition ?? '') === '1')
              form.push([`vars[tax_query][${taxonomy}][operation]`, 'AND']);
            form.push(
              [`vars[tax_query][${taxonomy}][taxonomy]`, 'wp-manga-genre'],
              [`vars[tax_query][${taxonomy}][field]`, 'slug'],
            );
            genreSlugs.forEach((slug, index) =>
              form.push([`vars[tax_query][${taxonomy}][terms][${index}]`, slug]),
            );
            taxonomy++;
          }
        }
        const answer = await http.post(`${t.baseUrl}/wp-admin/admin-ajax.php`, {
          kind: 'xhr',
          headers: xhr,
          form,
        });
        const mangas = parseArchive(await answer.document());
        return listing(mangas, mangas.length === o.pageSize);
      };
      calls = {
        popular: ({ page = 1 }) => ajaxList(page, 'popular'),
        latest: ({ page = 1 }) => ajaxList(page, 'latest'),
        filters: async () => {
          const routes = await knownGenres();
          const filters = [
            {
              key: 'status',
              label: 'Status',
              kind: 'tags',
              options: optionList(o.statusFilterOptions),
            },
            {
              key: 'sort',
              label: 'Order By',
              kind: 'select',
              options: optionList(o.orderByFilterOptions),
            },
            {
              key: 'adult',
              label: 'Adult Content',
              kind: 'select',
              options: optionList(o.adultFilterOptions),
            },
          ];
          if (routes.length) {
            filters.push({
              key: 'condition',
              label: 'Genre condition',
              kind: 'select',
              options: optionList(o.genreConditionFilterOptions),
            });
            filters.push({
              key: 'genres',
              label: 'Genres',
              kind: 'tags',
              options: routes.map((route) => ({ value: route.slug, label: route.name })),
            });
          }
          return filters;
        },
        search: ({ query = '', page = 1, filters = {} }) =>
          ajaxList(page, 'search', query, filters),
      };
    }

    if (!o.supportsLatest) delete calls.latest;
    return {
      ...calls,
      details: async ({ id }) => parseDetails(await mangaPage(id), id),
      chapters: async ({ id }) => ({ chapters: await ajaxChapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pageList(chapterId) }),
    };
  };

  const messages = {
    author_filter_title: 'Author',
    artist_filter_title: 'Artist',
    year_filter_title: 'Year of Released',
    status_filter_title: 'Status',
    status_filter_completed: 'Completed',
    status_filter_ongoing: 'Ongoing',
    status_filter_canceled: 'Canceled',
    status_filter_on_hold: 'On Hold',
    order_by_filter_title: 'Order By',
    order_by_filter_relevance: 'Relevance',
    order_by_filter_latest: 'Latest',
    order_by_filter_az: 'A-Z',
    order_by_filter_rating: 'Rating',
    order_by_filter_trending: 'Trending',
    order_by_filter_views: 'Most Views',
    order_by_filter_new: 'New',
    genre_condition_filter_title: 'Genre condition',
    genre_condition_filter_or: 'OR',
    genre_condition_filter_and: 'AND',
    adult_content_filter_title: 'Adult Content',
    adult_content_filter_all: 'All',
    adult_content_filter_none: 'None',
    adult_content_filter_only: 'Only',
    genre_filter_title: 'Genres',
    alt_names_heading: 'Alternative Names:',
  };

  mihon.defineTheme('madara', { defaults, make, messages });
})();
