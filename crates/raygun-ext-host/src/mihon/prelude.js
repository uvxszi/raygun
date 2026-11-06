// The runtime of converted Mihon extensions (`crate::mihon`): what their Kotlin leaned on, for the
// JavaScript they became. `K` holds Kotlin's helpers and Java's date formats; `Jsoup` parses pages
// into documents kept natively and queried with Raygun's Jsoup-compatible selectors; `mihon.http`
// makes a source's requests as Keiyoushi's `KeiSource` does (its Referer, its rate limit); and
// `mihon.theme` registers an extension's sources with the theme that runs them (`themes/*.js`).
'use strict';
(() => {
  const native = globalThis.__rgMihon;
  delete globalThis.__rgMihon;

  const unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== undefined) throw new Error(envelope.error);
    return envelope.value;
  };
  const dom = (op, args) => unwrap(native.dom(op, JSON.stringify(args)));

  // ---- Kotlin's helpers ----

  const K = {};

  /** Kotlin's `toString()` of a value: `null` for null, `1.5` and `2` for numbers. */
  K.str = (value) => {
    if (value === null || value === undefined) return 'null';
    if (typeof value === 'string') return value;
    if (Array.isArray(value)) return `[${value.map(K.str).join(', ')}]`;
    return String(value);
  };

  K.plus = (left, right) => {
    if (typeof left === 'string') return left + K.str(right);
    if (Array.isArray(left))
      return Array.isArray(right) ? left.concat(right) : left.concat([right]);
    return left + right;
  };

  /** Integer division when both sides are integers, as Kotlin's `Int / Int`. */
  K.div = (left, right) =>
    Number.isInteger(left) && Number.isInteger(right) ? Math.trunc(left / right) : left / right;

  K.eq = (left, right) => {
    if (left === right) return true;
    if (left == null || right == null) return left == null && right == null;
    if (Array.isArray(left) && Array.isArray(right)) {
      return left.length === right.length && left.every((item, index) => K.eq(item, right[index]));
    }
    if (typeof left.equals === 'function') return left.equals(right);
    return false;
  };

  K.raise = (message) => {
    throw new Error(message);
  };

  K.contains = (container, item) => {
    if (container == null) return false;
    if (typeof container === 'string') return container.includes(K.str(item));
    if (container instanceof Map) return container.has(item);
    if (typeof container.contains === 'function') return container.contains(item);
    if (Array.isArray(container)) return container.some((entry) => K.eq(entry, item));
    return false;
  };

  K.replace = (text, from, to) => K.str(text).split(K.str(from)).join(K.str(to));
  K.removePrefix = (text, prefix) => (text.startsWith(prefix) ? text.slice(prefix.length) : text);
  K.removeSuffix = (text, suffix) =>
    suffix && text.endsWith(suffix) ? text.slice(0, text.length - suffix.length) : text;
  K.substringAfter = (text, delimiter, missing = text) => {
    const at = text.indexOf(delimiter);
    return at < 0 ? missing : text.slice(at + delimiter.length);
  };
  K.substringBefore = (text, delimiter, missing = text) => {
    const at = text.indexOf(delimiter);
    return at < 0 ? missing : text.slice(0, at);
  };
  K.substringAfterLast = (text, delimiter, missing = text) => {
    const at = text.lastIndexOf(delimiter);
    return at < 0 ? missing : text.slice(at + delimiter.length);
  };
  K.substringBeforeLast = (text, delimiter, missing = text) => {
    const at = text.lastIndexOf(delimiter);
    return at < 0 ? missing : text.slice(0, at);
  };
  K.isBlank = (text) => text == null || K.str(text).trim() === '';
  K.ifBlank = (text, fallback) => (K.isBlank(text) ? fallback() : text);

  K.trimIndent = (text) => {
    const lines = text.split('\n');
    while (lines.length && lines[0].trim() === '') lines.shift();
    while (lines.length && lines[lines.length - 1].trim() === '') lines.pop();
    const indent = Math.min(
      ...lines.filter((line) => line.trim() !== '').map((line) => line.match(/^\s*/)[0].length),
    );
    return lines.map((line) => line.slice(Number.isFinite(indent) ? indent : 0)).join('\n');
  };
  K.trimMargin = (text, margin = '|') =>
    K.trimIndent(text)
      .split('\n')
      .map((line) => {
        const trimmed = line.trimStart();
        return trimmed.startsWith(margin) ? trimmed.slice(margin.length) : line;
      })
      .join('\n');

  /** Java's `String.format`, for `%s`, `%d` and `%%`. */
  K.format = (pattern, ...args) => {
    let next = 0;
    return pattern.replace(/%(\d+\$)?([sdf%])/g, (whole, position, kind) => {
      if (kind === '%') return '%';
      const value = position ? args[parseInt(position, 10) - 1] : args[next++];
      return kind === 'd' ? String(Math.trunc(Number(value))) : K.str(value);
    });
  };

  K.joinToString = (
    items,
    separator = ', ',
    prefix = '',
    postfix = '',
    limit = -1,
    truncated = '...',
    transform = null,
  ) => {
    let list = [...items];
    let tail = '';
    if (limit >= 0 && list.length > limit) {
      list = list.slice(0, limit);
      tail = separator + truncated;
    }
    return (
      prefix +
      list.map((item) => K.str(transform ? transform(item) : item)).join(separator) +
      tail +
      postfix
    );
  };

  K.mapOf = (pairs) =>
    new Map(pairs.map((pair) => (Array.isArray(pair) ? pair : [pair.first, pair.second])));

  // ---- Regular expressions: Java's syntax, as far as JavaScript takes it ----

  K.regex = (pattern, ...options) => {
    let source = K.str(pattern);
    let flags = 'u';
    for (const option of options.flat()) {
      if (option === 'i' || option === 'm' || option === 's') flags += option;
    }
    // Inline flags at the start: `(?i)`.
    const inline = source.match(/^\(\?([imsx]+)\)/);
    if (inline) {
      source = source.slice(inline[0].length);
      for (const flag of inline[1]) if (flag !== 'x' && !flags.includes(flag)) flags += flag;
    }
    // Java's possessive quantifiers and atomic groups match the same here, more slowly.
    source = source.replace(/([*+?}])\+/g, '$1').replace(/\(\?>/g, '(?:');
    source = source.replace(/\\Z/g, '$').replace(/\\A/g, '^').replace(/\\h/g, '[ \\t\\u00a0]');
    try {
      return new RegExp(source, flags);
    } catch (error) {
      // Without `u`, a few Java escapes (`\-` in a class) still read.
      return new RegExp(source, flags.replace('u', ''));
    }
  };

  // ---- Locales and Java's date formats ----

  K.locale = (language = 'en', country = '') => {
    const tag = K.str(language).replace('_', '-');
    const [lang, region] = tag.split('-');
    return {
      language: (lang || 'en').toLowerCase(),
      country: (country || region || '').toUpperCase(),
    };
  };

  const MONTHS = {
    en: [
      'january',
      'february',
      'march',
      'april',
      'may',
      'june',
      'july',
      'august',
      'september',
      'october',
      'november',
      'december',
    ],
    es: [
      'enero',
      'febrero',
      'marzo',
      'abril',
      'mayo',
      'junio',
      'julio',
      'agosto',
      'septiembre',
      'octubre',
      'noviembre',
      'diciembre',
    ],
    pt: [
      'janeiro',
      'fevereiro',
      'março',
      'abril',
      'maio',
      'junho',
      'julho',
      'agosto',
      'setembro',
      'outubro',
      'novembro',
      'dezembro',
    ],
    fr: [
      'janvier',
      'février',
      'mars',
      'avril',
      'mai',
      'juin',
      'juillet',
      'août',
      'septembre',
      'octobre',
      'novembre',
      'décembre',
    ],
    de: [
      'januar',
      'februar',
      'märz',
      'april',
      'mai',
      'juni',
      'juli',
      'august',
      'september',
      'oktober',
      'november',
      'dezember',
    ],
    it: [
      'gennaio',
      'febbraio',
      'marzo',
      'aprile',
      'maggio',
      'giugno',
      'luglio',
      'agosto',
      'settembre',
      'ottobre',
      'novembre',
      'dicembre',
    ],
    nl: [
      'januari',
      'februari',
      'maart',
      'april',
      'mei',
      'juni',
      'juli',
      'augustus',
      'september',
      'oktober',
      'november',
      'december',
    ],
    tr: [
      'ocak',
      'şubat',
      'mart',
      'nisan',
      'mayıs',
      'haziran',
      'temmuz',
      'ağustos',
      'eylül',
      'ekim',
      'kasım',
      'aralık',
    ],
    id: [
      'januari',
      'februari',
      'maret',
      'april',
      'mei',
      'juni',
      'juli',
      'agustus',
      'september',
      'oktober',
      'november',
      'desember',
    ],
    ms: [
      'januari',
      'februari',
      'mac',
      'april',
      'mei',
      'jun',
      'julai',
      'ogos',
      'september',
      'oktober',
      'november',
      'disember',
    ],
    ru: [
      'января',
      'февраля',
      'марта',
      'апреля',
      'мая',
      'июня',
      'июля',
      'августа',
      'сентября',
      'октября',
      'ноября',
      'декабря',
    ],
    ruStandalone: [
      'январь',
      'февраль',
      'март',
      'апрель',
      'май',
      'июнь',
      'июль',
      'август',
      'сентябрь',
      'октябрь',
      'ноябрь',
      'декабрь',
    ],
    uk: [
      'січня',
      'лютого',
      'березня',
      'квітня',
      'травня',
      'червня',
      'липня',
      'серпня',
      'вересня',
      'жовтня',
      'листопада',
      'грудня',
    ],
    pl: [
      'stycznia',
      'lutego',
      'marca',
      'kwietnia',
      'maja',
      'czerwca',
      'lipca',
      'sierpnia',
      'września',
      'października',
      'listopada',
      'grudnia',
    ],
    plStandalone: [
      'styczeń',
      'luty',
      'marzec',
      'kwiecień',
      'maj',
      'czerwiec',
      'lipiec',
      'sierpień',
      'wrzesień',
      'październik',
      'listopad',
      'grudzień',
    ],
    ar: [
      'يناير',
      'فبراير',
      'مارس',
      'أبريل',
      'مايو',
      'يونيو',
      'يوليو',
      'أغسطس',
      'سبتمبر',
      'أكتوبر',
      'نوفمبر',
      'ديسمبر',
    ],
    th: [
      'มกราคม',
      'กุมภาพันธ์',
      'มีนาคม',
      'เมษายน',
      'พฤษภาคม',
      'มิถุนายน',
      'กรกฎาคม',
      'สิงหาคม',
      'กันยายน',
      'ตุลาคม',
      'พฤศจิกายน',
      'ธันวาคม',
    ],
    thShort: [
      'ม.ค.',
      'ก.พ.',
      'มี.ค.',
      'เม.ย.',
      'พ.ค.',
      'มิ.ย.',
      'ก.ค.',
      'ส.ค.',
      'ก.ย.',
      'ต.ค.',
      'พ.ย.',
      'ธ.ค.',
    ],
    vi: [
      'tháng 1',
      'tháng 2',
      'tháng 3',
      'tháng 4',
      'tháng 5',
      'tháng 6',
      'tháng 7',
      'tháng 8',
      'tháng 9',
      'tháng 10',
      'tháng 11',
      'tháng 12',
    ],
    ca: [
      'gener',
      'febrer',
      'març',
      'abril',
      'maig',
      'juny',
      'juliol',
      'agost',
      'setembre',
      'octubre',
      'novembre',
      'desembre',
    ],
  };
  const SHORT_FIXES = {
    sept: 8,
    set: 8,
    ago: 7,
    agu: 7,
    agt: 7,
    okt: 9,
    ott: 9,
    out: 9,
    dic: 11,
    dez: 11,
    des: 11,
  };

  const fold = (text) => text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/\.$/, '');

  /** The month a name means (0-11), in the locale's language first, then in any. */
  const monthOf = (name, locale) => {
    const wanted = fold(name);
    if (!wanted) return -1;
    const languages = [
      locale.language,
      `${locale.language}Standalone`,
      `${locale.language}Short`,
      ...Object.keys(MONTHS),
    ];
    for (const language of languages) {
      const names = MONTHS[language];
      if (!names) continue;
      const index = names.findIndex((month) => {
        const folded = fold(month);
        return folded === wanted || (wanted.length >= 3 && folded.startsWith(wanted));
      });
      if (index >= 0) return index;
    }
    if (SHORT_FIXES[wanted] !== undefined) return SHORT_FIXES[wanted];
    return -1;
  };

  /**
   * A Java date format (`SimpleDateFormat`, `DateTimeFormatter.ofPattern`): `parse(text)` gives
   * Unix milliseconds (UTC) or null, `format(ms)` the text.
   */
  class DateFormat {
    constructor(pattern, locale = K.locale('en')) {
      this.pattern = K.str(pattern);
      this.locale = locale && locale.language ? locale : K.locale(K.str(locale));
      this.offsetMinutes = 0;
      // `timeZone = TimeZone.getTimeZone("UTC")`: an own property, as Kotlin receivers find one.
      Object.defineProperty(this, 'timeZone', {
        get: () => ({ offset: this.offsetMinutes, isZone: true }),
        set: (zone) => {
          this.offsetMinutes = zone && typeof zone.offset === 'number' ? zone.offset : 0;
        },
        enumerable: false,
      });
      const fields = [];
      let source = '';
      const tokens = this.pattern.match(/'[^']*'|''|([a-zA-Z])\1*|[^a-zA-Z']+/g) || [];
      for (const token of tokens) {
        if (token === "''") {
          source += "'";
        } else if (token.startsWith("'")) {
          source += token.slice(1, -1).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        } else if (/^[a-zA-Z]/.test(token)) {
          const letter = token[0];
          const width = token.length;
          if (letter === 'M' || letter === 'L') {
            if (width >= 3) {
              source += '([\\p{L}\\p{M}.]+(?: \\d{1,2})?)';
              fields.push('monthName');
            } else {
              source += '(\\d{1,2})';
              fields.push('month');
            }
          } else if (letter === 'y' || letter === 'u' || letter === 'Y') {
            source += width === 2 ? '(\\d{2})' : '(\\d{1,4})';
            fields.push(width === 2 ? 'year2' : 'year');
          } else if (letter === 'd') {
            source += '(\\d{1,2})';
            fields.push('day');
          } else if (letter === 'H' || letter === 'k') {
            source += '(\\d{1,2})';
            fields.push('hour');
          } else if (letter === 'h' || letter === 'K') {
            source += '(\\d{1,2})';
            fields.push('hour12');
          } else if (letter === 'm') {
            source += '(\\d{1,2})';
            fields.push('minute');
          } else if (letter === 's') {
            source += '(\\d{1,2})';
            fields.push('second');
          } else if (letter === 'S') {
            source += '(\\d{1,9})';
            fields.push('fraction');
          } else if (letter === 'a') {
            source += '([\\p{L}.]+)';
            fields.push('ampm');
          } else if (letter === 'E') {
            source += '[\\p{L}.]+';
          } else if (
            letter === 'z' ||
            letter === 'Z' ||
            letter === 'X' ||
            letter === 'x' ||
            letter === 'O'
          ) {
            source += '(Z|[+-]\\d{2}:?\\d{2}|[A-Za-z]{2,5}(?:[+-]\\d{1,2})?)?';
            fields.push('zone');
          } else {
            source += '.+?';
          }
        } else {
          source += token.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\s+/g, '\\s+');
        }
      }
      this.fields = fields;
      this.matcher = new RegExp(`^\\s*${source}\\s*$`, 'iu');
    }

    parse(text) {
      if (text == null) return null;
      const found = K.str(text).trim().match(this.matcher);
      if (!found) return null;
      const parts = {
        year: 1970,
        month: 0,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        ms: 0,
        pm: null,
        zone: null,
      };
      this.fields.forEach((field, index) => {
        const value = found[index + 1];
        if (value === undefined) return;
        switch (field) {
          case 'monthName': {
            const month = monthOf(value, this.locale);
            if (month < 0) parts.invalid = true;
            parts.month = month;
            break;
          }
          case 'month':
            parts.month = parseInt(value, 10) - 1;
            break;
          case 'year':
            parts.year = parseInt(value, 10);
            break;
          case 'year2':
            parts.year = 2000 + parseInt(value, 10);
            break;
          case 'day':
            parts.day = parseInt(value, 10);
            break;
          case 'hour':
            parts.hour = parseInt(value, 10);
            break;
          case 'hour12':
            parts.hour = parseInt(value, 10) % 12;
            break;
          case 'minute':
            parts.minute = parseInt(value, 10);
            break;
          case 'second':
            parts.second = parseInt(value, 10);
            break;
          case 'fraction':
            parts.ms = Math.round(parseInt(value.padEnd(3, '0').slice(0, 3), 10));
            break;
          case 'ampm':
            parts.pm = /^p|^ch|^ب|^م|下午/i.test(value);
            break;
          case 'zone':
            parts.zone = value;
            break;
          default:
        }
      });
      if (parts.invalid) return null;
      if (parts.pm === true) parts.hour += 12;
      // Thai dates count years from the Buddha's birth.
      if (this.locale.language === 'th' && parts.year > 2400) parts.year -= 543;
      let ms = Date.UTC(
        parts.year,
        parts.month,
        parts.day,
        parts.hour,
        parts.minute,
        parts.second,
        parts.ms,
      );
      if (Number.isNaN(ms)) return null;
      let offset = this.offsetMinutes;
      if (parts.zone && parts.zone !== 'Z') {
        const zone = parts.zone.match(/([+-])(\d{2}):?(\d{2})/);
        if (zone)
          offset =
            (zone[1] === '-' ? -1 : 1) * (parseInt(zone[2], 10) * 60 + parseInt(zone[3], 10));
      } else if (parts.zone === 'Z') {
        offset = 0;
      }
      ms -= offset * 60000;
      return ms;
    }

    /** Parses, or gives 0 (Mihon's "no date"); Keiyoushi's `tryParse`, `tryParseDate` and
     * `tryParseDateTime` alike. */
    tryParse(text) {
      return this.parse(text) ?? 0;
    }

    tryParseDate(text) {
      return this.tryParse(text);
    }

    tryParseDateTime(text) {
      return this.tryParse(text);
    }

    setTimeZone(zone) {
      this.timeZone = zone;
    }

    /** `DateTimeFormatter.withZone(…)`: a copy reading times in that zone. */
    withZone(zone) {
      return K.withZone(this, zone && typeof zone.offset === 'number' ? zone.offset : 0);
    }

    withLocale(locale) {
      return new DateFormat(this.pattern, locale).withOffset(this.offsetMinutes);
    }

    withOffset(offset) {
      this.offsetMinutes = offset;
      return this;
    }

    format(ms) {
      // A java.time value (`{ ms }`) too; a local one is already its wall clock.
      const local = ms && (ms.__class === 'LocalDate' || ms.__class === 'LocalDateTime');
      const value =
        typeof ms === 'number' ? ms : ms && typeof ms.ms === 'number' ? ms.ms : Number(ms);
      const date = new Date(value + (local ? 0 : this.offsetMinutes * 60000));
      const names = MONTHS[this.locale.language] || MONTHS.en;
      const pad = (value, width) => String(value).padStart(width, '0');
      return this.pattern.replace(/'[^']*'|([a-zA-Z])\1*/g, (token) => {
        if (token.startsWith("'")) return token.slice(1, -1);
        const width = token.length;
        switch (token[0]) {
          case 'y':
            return width === 2
              ? pad(date.getUTCFullYear() % 100, 2)
              : String(date.getUTCFullYear());
          case 'M': {
            const month = date.getUTCMonth();
            if (width >= 4) return names[month];
            if (width === 3) return names[month].slice(0, 3);
            return pad(month + 1, width);
          }
          case 'd':
            return pad(date.getUTCDate(), width);
          case 'H':
            return pad(date.getUTCHours(), width);
          case 'm':
            return pad(date.getUTCMinutes(), width);
          case 's':
            return pad(date.getUTCSeconds(), width);
          default:
            return token;
        }
      });
    }
  }

  K.dateFormat = (pattern, locale) => new DateFormat(pattern, locale);
  /** A time zone: an offset in minutes (`GMT+7`, `UTC`, `Asia/Jakarta` as best known). */
  K.zone = (name) => {
    const text = K.str(name);
    const offset = text.match(/([+-])(\d{1,2})(?::?(\d{2}))?$/);
    if (offset)
      return (
        (offset[1] === '-' ? -1 : 1) *
        (parseInt(offset[2], 10) * 60 + parseInt(offset[3] || '0', 10))
      );
    const known = {
      'Asia/Jakarta': 420,
      'Asia/Bangkok': 420,
      'Asia/Ho_Chi_Minh': 420,
      'Asia/Tokyo': 540,
      'Asia/Seoul': 540,
      'Asia/Shanghai': 480,
      'Europe/Istanbul': 180,
      'Asia/Riyadh': 180,
    };
    return known[text] ?? 0;
  };
  K.withZone = (format, offset) => {
    const zoned = new DateFormat(format.pattern, format.locale);
    zoned.offsetMinutes = offset;
    return zoned;
  };

  // ---- Addresses ----

  const url = (href, base = '') => unwrap(native.url(K.str(href), K.str(base)));
  K.url = url;
  /** `href` resolved against `base`; '' when it doesn't resolve (Jsoup's `absUrl`). */
  K.resolve = (href, base) => {
    if (href == null || href === '') return '';
    try {
      return url(href, base).href;
    } catch {
      return '';
    }
  };
  /**
   * The address without scheme and host, as Mihon's `setUrlWithoutDomain` keeps it: Java's
   * `URI` path, query and fragment, which come decoded (`%2Fmanga%2Fone` is `/manga/one`).
   */
  K.withoutDomain = (href) => {
    let rest = K.str(href).replace(/ /g, '%20');
    const decode = (part) => {
      try {
        return decodeURIComponent(part);
      } catch {
        return part;
      }
    };
    const origin = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\/[^/?#]*/.exec(rest) ?? /^\/\/[^/?#]*/.exec(rest);
    if (origin) rest = rest.slice(origin[0].length);
    let fragment = null;
    const hash = rest.indexOf('#');
    if (hash >= 0) {
      fragment = rest.slice(hash + 1);
      rest = rest.slice(0, hash);
    }
    let query = null;
    const question = rest.indexOf('?');
    if (question >= 0) {
      query = rest.slice(question + 1);
      rest = rest.slice(0, question);
    }
    return (
      decode(rest) +
      (query !== null ? `?${decode(query)}` : '') +
      (fragment !== null ? `#${decode(fragment)}` : '')
    );
  };
  K.encode = (text) => encodeURIComponent(K.str(text));
  /** A form body (`application/x-www-form-urlencoded`). */
  K.form = (pairs) =>
    pairs
      .map(([key, value]) =>
        `${encodeURIComponent(key)}=${encodeURIComponent(K.str(value))}`.replace(/%20/g, '+'),
      )
      .join('&');
  K.query = (base, pairs) => {
    const query = pairs
      .filter(([, value]) => value !== null && value !== undefined)
      .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(K.str(value))}`)
      .join('&');
    if (!query) return base;
    return base + (base.includes('?') ? '&' : '?') + query;
  };

  K.decrypt = (options) => unwrap(native.decrypt(JSON.stringify(options)));
  K.base64Decode = (text) => {
    const binary = atob(K.str(text).replace(/\s+/g, ''));
    // UTF-8 bytes to text.
    let escaped = '';
    for (let index = 0; index < binary.length; index++) {
      escaped += `%${binary.charCodeAt(index).toString(16).padStart(2, '0')}`;
    }
    try {
      return decodeURIComponent(escaped);
    } catch {
      return binary;
    }
  };

  // ---- Jsoup's documents ----

  class Elements extends Array {
    static of(handles, doc) {
      const list = new Elements();
      for (const handle of handles) list.push(new Element(handle, doc));
      return list;
    }
    get size() {
      return this.length;
    }
    first() {
      return this[0] ?? null;
    }
    last() {
      return this[this.length - 1] ?? null;
    }
    isEmpty() {
      return this.length === 0;
    }
    isNotEmpty() {
      return this.length > 0;
    }
    textOrNull() {
      const value = this.text();
      return value === '' ? null : value;
    }
    text() {
      return this.map((element) => element.text())
        .filter((text) => text !== '')
        .join(' ');
    }
    eachText() {
      return this.map((element) => element.text()).filter((text) => text !== '');
    }
    attr(name) {
      const found = this.find((element) => element.hasAttr(name));
      return found ? found.attr(name) : '';
    }
    hasAttr(name) {
      return this.some((element) => element.hasAttr(name));
    }
    eachAttr(name) {
      return this.filter((element) => element.hasAttr(name)).map((element) => element.attr(name));
    }
    html() {
      return this.map((element) => element.html()).join('\n');
    }
    outerHtml() {
      return this.map((element) => element.outerHtml()).join('\n');
    }
    select(css) {
      if (this.length === 0) return new Elements();
      const doc = this[0].doc;
      return Elements.of(
        dom('select', { nodes: this.map((element) => element.h), selector: css }),
        doc,
      );
    }
    selectFirst(css) {
      return this.select(css).first();
    }
    not(css) {
      const kept = new Set(
        dom('filter', { nodes: this.map((element) => element.h), selector: css }),
      );
      return Elements.of(
        this.filter((element) => !kept.has(element.h)).map((element) => element.h),
        this[0]?.doc,
      );
    }
    is(css) {
      return (
        this.length > 0 &&
        dom('filter', { nodes: this.map((element) => element.h), selector: css }).length > 0
      );
    }
    remove() {
      if (this.length > 0) dom('remove', { nodes: this.map((element) => element.h) });
      return this;
    }
    get(index) {
      return this[index];
    }
  }

  class Element {
    constructor(handle, doc) {
      this.h = handle;
      this.doc = doc;
    }
    get node() {
      if (this._node === undefined) this._node = dom('info', { nodes: [this.h] })[0];
      return this._node;
    }
    select(css) {
      return Elements.of(dom('select', { nodes: [this.h], selector: css }), this.doc);
    }
    selectFirst(css) {
      return this.select(css).first();
    }
    // Keiyoushi's Jsoup helpers (`keiyoushi.utils`).
    selectLast(css) {
      return this.select(css).last();
    }
    selectFirstOrNull(css) {
      return this.selectFirst(css);
    }
    attrOrNull(name) {
      const value = this.attr(name);
      return value.trim() === '' ? null : value.trim();
    }
    textOrNull() {
      const value = this.text();
      return value === '' ? null : value;
    }
    ownTextOrNull() {
      const value = this.ownText();
      return value === '' ? null : value;
    }
    firstElementChild() {
      return this.children().first();
    }
    lastElementChild() {
      return this.children().last();
    }
    elementSiblingIndex() {
      const parent = this.parent();
      if (!parent) return 0;
      return parent.children().findIndex((child) => child.h === this.h);
    }
    baseUri() {
      return this.doc ? this.doc.location() : '';
    }
    attr(name) {
      if (name.startsWith('abs:')) return this.absUrl(name.slice(4));
      const attribs = this.node.attribs || {};
      return attribs[name] ?? attribs[name.toLowerCase()] ?? '';
    }
    hasAttr(name) {
      if (name.startsWith('abs:')) return this.absUrl(name.slice(4)) !== '';
      const attribs = this.node.attribs || {};
      return (
        Object.prototype.hasOwnProperty.call(attribs, name) ||
        Object.prototype.hasOwnProperty.call(attribs, name.toLowerCase())
      );
    }
    attributes() {
      return { ...(this.node.attribs || {}) };
    }
    absUrl(name) {
      const value = this.attr(name);
      if (value === '') return '';
      return K.resolve(value, this.doc ? this.doc.location() : '');
    }
    text() {
      return dom('jsoupText', { nodes: [this.h] })[0];
    }
    ownText() {
      return dom('ownText', { nodes: [this.h] });
    }
    wholeText() {
      return dom('text', { nodes: [this.h] });
    }
    data() {
      return dom('data', { nodes: [this.h] });
    }
    html() {
      return dom('html', { nodes: [this.h] }) ?? '';
    }
    outerHtml() {
      return dom('outer', { nodes: [this.h] });
    }
    toString() {
      return this.outerHtml();
    }
    tagName() {
      return this.node.name ?? '';
    }
    nodeName() {
      return this.tagName();
    }
    id() {
      return this.attr('id');
    }
    className() {
      return this.attr('class');
    }
    classNames() {
      return this.className().split(/\s+/).filter(Boolean);
    }
    hasClass(name) {
      return this.classNames().some((own) => own.toLowerCase() === K.str(name).toLowerCase());
    }
    hasText() {
      return this.text() !== '';
    }
    is(css) {
      return dom('filter', { nodes: [this.h], selector: css }).length > 0;
    }
    children() {
      return Elements.of(dom('children', { nodes: [this.h] }), this.doc);
    }
    child(index) {
      return this.children()[index];
    }
    parent() {
      const found = dom('parent', { nodes: [this.h] });
      return found.length ? new Element(found[0], this.doc) : null;
    }
    parents() {
      return Elements.of(dom('parents', { nodes: [this.h] }), this.doc);
    }
    nextElementSibling() {
      const found = dom('siblings', { nodes: [this.h], forward: true });
      return found.length ? new Element(found[0], this.doc) : null;
    }
    previousElementSibling() {
      const found = dom('siblings', { nodes: [this.h], forward: false });
      return found.length ? new Element(found[0], this.doc) : null;
    }
    // Text nodes aren't kept apart: the elements beside it stand for its siblings.
    nextSibling() {
      return this.nextElementSibling();
    }
    previousSibling() {
      return this.previousElementSibling();
    }
    /** Jsoup's `closest(css)`: this element or the nearest parent matching. */
    closest(css) {
      for (let at = this; at; at = at.parent()) if (at.is(css)) return at;
      return null;
    }
    nextElementSiblings() {
      return Elements.of(dom('siblings', { nodes: [this.h], forward: true, all: true }), this.doc);
    }
    siblingElements() {
      const before = dom('siblings', { nodes: [this.h], forward: false, all: true }).reverse();
      const after = dom('siblings', { nodes: [this.h], forward: true, all: true });
      return Elements.of([...before, ...after], this.doc);
    }
    remove() {
      dom('remove', { nodes: [this.h] });
    }
    equals(other) {
      return other instanceof Element && other.h === this.h;
    }
  }

  class Document extends Element {
    constructor(handle, location) {
      super(handle, null);
      this.doc = this;
      this.loc = location ?? '';
    }
    location() {
      return this.loc;
    }
    body() {
      return this.selectFirst('body');
    }
    head() {
      return this.selectFirst('head');
    }
    title() {
      return this.selectFirst('title')?.text() ?? '';
    }
    html() {
      return this.outerHtml();
    }
    outerHtml() {
      return dom('html', { nodes: [this.h] }) ?? '';
    }
  }

  const Jsoup = Object.freeze({
    parse: (html, location = '') => new Document(dom('load', { html: K.str(html) }), location),
    /** A snippet's elements (`Jsoup.parseBodyFragment`). */
    fragment: (html, location = '') => {
      const doc = new Document(dom('load', { html: `<body>${K.str(html)}</body>` }), location);
      return doc.body() ?? doc;
    },
  });

  // ---- Requests ----

  const ACCEPT = {
    document:
      'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8',
    xhr: '*/*',
    script: '*/*',
    image: 'image/avif,image/webp,image/png,image/jpeg,*/*;q=0.8',
  };

  class HttpError extends Error {
    constructor(code, address) {
      let host = address;
      try {
        host = url(address).host;
      } catch {
        // as given
      }
      super(`${host} answered HTTP ${code}`);
      this.code = code;
    }
  }

  class Response {
    constructor(raw, request) {
      this.code = raw.status;
      this.url = raw.url || request;
      this.headers = raw.headers || {};
      this.raw = raw;
    }
    get isSuccessful() {
      return this.code >= 200 && this.code < 300;
    }
    header(name) {
      return this.headers[name.toLowerCase()] ?? null;
    }
    async text() {
      return this.raw.text();
    }
    async json() {
      return JSON.parse(await this.raw.text());
    }
    async document() {
      return Jsoup.parse(await this.raw.text(), this.url);
    }
  }

  /** A source's requests: its default headers, its rate limit, Raygun's network limits beneath. */
  class Http {
    constructor(source, options = {}) {
      this.source = source;
      this.limit = options.rateLimit ?? null;
      this.sent = [];
      this.extraHeaders = options.headers ?? [];
    }

    headers(kind, from) {
      const headers = { Referer: `${this.source.baseUrl}/` };
      if (ACCEPT[kind]) headers.Accept = ACCEPT[kind];
      if (from) headers.Referer = from.startsWith('http') ? from : this.source.baseUrl + from;
      for (const [name, value] of this.extraHeaders) headers[name] = K.str(value);
      return headers;
    }

    async pace() {
      if (!this.limit) return;
      const { permits, periodMs } = this.limit;
      for (;;) {
        const now = Date.now();
        this.sent = this.sent.filter((at) => now - at < periodMs);
        if (this.sent.length < permits) {
          this.sent.push(now);
          return;
        }
        await raygun.time.sleep(Math.min(10000, periodMs - (now - this.sent[0]) + 5));
      }
    }

    /**
     * Sends a request: `kind` ('document', 'xhr', 'image', 'none') sets what a browser would
     * accept, `from` the page it comes from. An answer that isn't 2xx throws unless `ensureSuccess`
     * is false.
     */
    async request(
      address,
      {
        method = 'GET',
        headers = {},
        body = null,
        kind = 'document',
        from = null,
        ensureSuccess = true,
        form = null,
      } = {},
    ) {
      await this.pace();
      const merged = { ...this.headers(kind, from), ...headers };
      let payload = body;
      if (form) {
        payload = K.form(form);
        merged['Content-Type'] = 'application/x-www-form-urlencoded';
      } else if (payload !== null && typeof payload === 'object') {
        payload = JSON.stringify(payload);
        merged['Content-Type'] = merged['Content-Type'] || 'application/json';
      }
      const raw = await raygun.http.fetch(address, { method, headers: merged, body: payload });
      const response = new Response(raw, address);
      if (ensureSuccess && !response.isSuccessful)
        throw new HttpError(response.code, response.url || address);
      return response;
    }

    get(address, options = {}) {
      return this.request(address, { ...options, method: 'GET' });
    }

    post(address, options = {}) {
      return this.request(address, { ...options, method: 'POST' });
    }

    async document(address, options = {}) {
      return (await this.get(address, options)).document();
    }
  }

  // ---- Statuses and results, as Raygun's `manga.source` takes them ----

  const STATUS = {
    0: 'unknown',
    1: 'ongoing',
    2: 'completed',
    3: 'unknown',
    4: 'publishingFinished',
    5: 'cancelled',
    6: 'hiatus',
  };
  const SManga = {
    UNKNOWN: 0,
    ONGOING: 1,
    COMPLETED: 2,
    LICENSED: 3,
    PUBLISHING_FINISHED: 4,
    CANCELLED: 5,
    ON_HIATUS: 6,
  };

  const splitList = (text) =>
    text == null
      ? []
      : K.str(text)
          .split(',')
          .map((part) => part.trim())
          .filter(Boolean);

  /** A theme's manga (Mihon's `SManga`) as a Raygun title. */
  const toDetails = (source, id, manga) => ({
    id,
    title: (manga.title ?? '').trim(),
    coverUrl: manga.thumbnail_url
      ? K.resolve(manga.thumbnail_url, `${source.baseUrl}/`)
      : undefined,
    coverHeaders: { Referer: `${source.baseUrl}/` },
    authors: splitList(manga.author),
    artists: splitList(manga.artist),
    description: manga.description ?? undefined,
    genres: splitList(manga.genre),
    status: STATUS[manga.status ?? 0] ?? 'unknown',
    url: manga.webUrl ?? K.resolve(id, `${source.baseUrl}/`),
  });

  // ---- Themes ----

  const themes = new Map();

  /** Options of a theme instance: each override's value, else the theme's default. */
  const options = (instance, defaults, overrides) => {
    const base = {};
    const own = {};
    const cache = new Map();
    for (const key of Object.keys(defaults)) {
      Object.defineProperty(base, key, { get: () => defaults[key](instance), enumerable: true });
    }
    for (const key of new Set([...Object.keys(defaults), ...Object.keys(overrides)])) {
      Object.defineProperty(own, key, {
        enumerable: true,
        get: () => {
          if (!cache.has(key)) {
            const value = Object.prototype.hasOwnProperty.call(overrides, key)
              ? overrides[key](instance)
              : defaults[key](instance);
            cache.set(key, value);
          }
          return cache.get(key);
        },
      });
    }
    return { base, own };
  };

  // Left open: the runtime of converted Kotlin (`kotlin.js`) adds `mihon.source`.
  const mihon = {
    K,
    Jsoup,
    Elements,
    Element,
    Document,
    Http,
    HttpError,
    SManga,
    toDetails,
    /** Registers a theme: `make(instance)` gives the provider's calls; `defaults` its values. */
    defineTheme: (name, theme) => {
      themes.set(name, theme);
    },
    /** Registers an extension's sources with the theme that runs them. */
    theme: (name, config) => {
      const theme = themes.get(name);
      if (!theme) throw new Error(`Raygun has no ${name} theme`);
      for (const source of config.sources) {
        const instance = {
          id: source.id,
          name: source.name,
          lang: source.lang,
          baseUrl: source.baseUrl,
          variant: config.variant ?? 'default',
        };
        const { base, own } = options(instance, theme.defaults, config.overrides ?? {});
        instance.base = base;
        instance.o = own;
        // The theme's messages (Keiyoushi's `intl`), in English.
        instance.intl = (key, ...args) => K.format(theme.messages?.[key] ?? K.str(key), ...args);
        instance.http = new Http(instance, {
          rateLimit: config.rateLimit,
          headers: config.headers ? config.headers(instance) : [],
        });
        const calls = theme.make(instance);
        raygun.providers.register({
          kind: 'manga.source',
          id: source.id,
          name: source.name,
          ...calls,
        });
      }
    },
  };

  globalThis.mihon = mihon;
  globalThis.K = K;
})();
