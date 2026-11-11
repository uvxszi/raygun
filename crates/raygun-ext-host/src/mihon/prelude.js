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

  /** Java's nested character classes (`[^(['")]]` is one class in Java): their members merged
   * into the class around them, as JavaScript nests none; a `]` first in a class is a member. */
  const flattenClasses = (source) => {
    let out = '';
    let depth = 0;
    for (let at = 0; at < source.length; at++) {
      const char = source[at];
      if (char === '\\') {
        out += char + (source[at + 1] ?? '');
        at++;
        continue;
      }
      if (char === '[' && depth === 0) {
        depth = 1;
        out += char;
        if (source[at + 1] === '^') {
          out += '^';
          at++;
        }
        if (source[at + 1] === ']') {
          out += '\\]';
          at++;
        }
        continue;
      }
      if (char === '[' && depth > 0) {
        depth++;
        if (source[at + 1] === '^') at++;
        continue;
      }
      if (char === ']' && depth > 0) {
        depth--;
        if (depth === 0) out += char;
        continue;
      }
      out += char;
    }
    return out;
  };
  K.regex = (pattern, ...options) => {
    let source = flattenClasses(K.str(pattern));
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
    // Inline flags past the start (`^(?i)…`, `(?i:…)`): JavaScript has them for the whole
    // pattern only, so they're read that way.
    source = source.replace(/\(\?([imsx]+)(\)|:)/g, (match, inline, end, at, whole) => {
      // `\(?i)` is an optional parenthesis, not flags.
      if (whole[at - 1] === '\\') return match;
      for (const flag of inline) if (flag !== 'x' && !flags.includes(flag)) flags += flag;
      return end === ':' ? '(?:' : '';
    });
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
      // What `parseDefaulting` gives the fields a text leaves out, and month names a
      // `DateTimeFormatterBuilder.appendText` lists.
      this.defaults = {};
      this.monthNames = null;
      const fields = [];
      let source = '';
      // DateTimeFormatter's optional sections (`[dd ]MMM [yyyy][yy]`) are groups that may match
      // nothing; a section's four-letter year wants all four digits, as Java's does.
      let optional = 0;
      const tokens = this.pattern.match(/'[^']*'|''|([a-zA-Z])\1*|\[|\]|[^a-zA-Z'[\]]+/g) || [];
      for (const token of tokens) {
        if (token === '[') {
          source += '(?:';
          optional += 1;
        } else if (token === ']' && optional > 0) {
          source += ')?';
          optional -= 1;
        } else if (token === "''") {
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
            source += width === 2 ? '(\\d{2})' : optional && width >= 3 ? '(\\d{4})' : '(\\d{1,4})';
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
      source += ')?'.repeat(optional);
      this.fields = fields;
      this.matcher = new RegExp(`^\\s*${source}\\s*$`, 'iu');
    }

    /** The text's milliseconds; with a query (`formatter.parse(text, LocalDate::from)`), what it
     * makes of them. */
    parse(text, query) {
      const ms = this.read(text);
      return typeof query === 'function' && ms !== null ? query(ms) : ms;
    }

    read(text) {
      if (text == null) return null;
      const found = K.str(text).trim().match(this.matcher);
      if (!found) return null;
      const defaults = this.defaults || {};
      const parts = {
        year: defaults.year ?? 1970,
        month: defaults.month ?? 0,
        day: defaults.day ?? 1,
        hour: defaults.hour ?? 0,
        minute: defaults.minute ?? 0,
        second: defaults.second ?? 0,
        ms: 0,
        pm: null,
        zone: null,
      };
      this.fields.forEach((field, index) => {
        const value = found[index + 1];
        if (value === undefined) return;
        switch (field) {
          case 'monthName': {
            const listed = this.monthNames
              ? this.monthNames.findIndex((name) => fold(name) === fold(value))
              : -1;
            const month = listed >= 0 ? listed : monthOf(value, this.locale);
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

    tryParseZonedDateTime(text) {
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
      const copy = new DateFormat(this.pattern, locale).withOffset(this.offsetMinutes);
      copy.defaults = this.defaults;
      copy.monthNames = this.monthNames;
      return copy;
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
      return this.pattern.replace(/'[^']*'|([a-zA-Z])\1*|\[|\]/g, (token) => {
        if (token === '[' || token === ']') return '';
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
    zoned.defaults = format.defaults;
    zoned.monthNames = format.monthNames;
    return zoned;
  };

  // ---- Addresses ----

  const url = (href, base = '') => unwrap(native.url(K.str(href), K.str(base)));
  K.url = url;
  /** `href` resolved against `base`; '' when it doesn't resolve (Jsoup's `absUrl`). */
  K.resolve = (href, base) => {
    if (href == null || href === '') return '';
    const text = K.str(href);
    try {
      // Java's `URL` keeps spaces as they are: a `srcset` (`a.jpg 110w, b.jpg 175w`) read as
      // `abs:srcset` stays a list the source splits.
      const resolved = url(text.trim(), base).href;
      return /\s/.test(text.trim()) && !text.includes('%20')
        ? resolved.replace(/%20/g, ' ')
        : resolved;
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
  /** AES, digests, HMAC and PBKDF2 in Rust, bytes as base64 (`runtime.rs`'s `CryptoArgs`). */
  K.crypto = (args) => unwrap(native.crypto(JSON.stringify(args)));
  /** Java's public keys (`mihon/keys.rs`): P-256 and RSA, keys as their DER encodings. */
  K.keys = (args) => JSON.parse(unwrap(native.keys(JSON.stringify(args))));
  /** A site's script run in a JavaScript engine of its own (`QuickJs.create().evaluate`). */
  K.evaluate = (code) => unwrap(native.evaluate(K.str(code)));
  /** Bytes as `Uint8Array`s, converted in Rust: base64 both ways, UTF-8 both ways. */
  const failedOr = (value) => {
    if (typeof value === 'string') throw new Error(value);
    return value;
  };
  K.bin = Object.freeze({
    toBase64: (bytes) => native.base64Encode(bytes),
    fromBase64: (text) => failedOr(native.base64Decode(K.str(text))),
    utf8: (text) => native.utf8Encode(K.str(text)),
    text: (bytes) => native.utf8Decode(bytes),
  });
  /** The pictures a source draws (`raygun_net::bitmap`), by number: see `mihon/pictures.rs`. */
  K.bitmap = Object.freeze({
    run: (op, args) => unwrap(native.bitmap(JSON.stringify({ op, ...args }))),
    decode: (bytes) => unwrap(native.bitmapDecode(bytes)),
    encode: (id, format, quality) => failedOr(native.bitmapEncode(id, String(format), quality)),
  });
  /** The extension's cookie jar: what it sends to `url` (`[name, value]` pairs), and keeping
   * `Set-Cookie` lines for it. */
  K.cookies = (href) => unwrap(native.cookies(K.str(href)));
  K.setCookies = (href, lines) => unwrap(native.setCookies(K.str(href), JSON.stringify(lines)));
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

  /** What an edit puts in the page: an element's or a text node's HTML, or HTML as given. */
  const htmlOf = (node) => {
    if (node && node.__class === 'TextNode') return node.outerHtml();
    if (node && typeof node.outerHtml === 'function') return node.outerHtml();
    return K.str(node);
  };

  /** A Jsoup `TextNode`: `text()` with its whitespace runs made one space, `wholeText` as written,
   * and the elements beside it. */
  const textNode = (found, doc) => {
    const raw = found.text;
    const beside = (handle) =>
      handle === null || handle === undefined ? null : new Element(handle, doc);
    return {
      __class: 'TextNode',
      __classes: ['TextNode', 'Node'],
      wholeText: raw,
      nodeName: () => '#text',
      text: () => raw.replace(/\s+/g, ' '),
      getWholeText: () => raw,
      isBlank: () => raw.trim() === '',
      nextElementSibling: () => beside(found.next),
      previousElementSibling: () => beside(found.previous),
      nextSibling: () => beside(found.next),
      previousSibling: () => beside(found.previous),
      outerHtml: () => raw.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'),
      toString: () => raw,
    };
  };

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
    ownerDocument() {
      return this.doc ?? null;
    }
    // Jsoup's edits: HTML (or a node's) put beside, inside or in place of the element.
    replaceWith(node) {
      dom('replace', { nodes: [this.h], html: htmlOf(node) });
      return this;
    }
    prepend(node) {
      dom('insert', { nodes: [this.h], html: htmlOf(node), place: 'prepend' });
      return this;
    }
    append(node) {
      dom('insert', { nodes: [this.h], html: htmlOf(node), place: 'append' });
      return this;
    }
    before(node) {
      dom('insert', { nodes: [this.h], html: htmlOf(node), place: 'before' });
      return this;
    }
    after(node) {
      dom('insert', { nodes: [this.h], html: htmlOf(node), place: 'after' });
      return this;
    }
    appendText(text) {
      return this.append(textNode({ text: K.str(text) }));
    }
    prependText(text) {
      return this.prepend(textNode({ text: K.str(text) }));
    }
    empty() {
      dom('empty', { nodes: [this.h] });
      return this;
    }
    attr(name, ...value) {
      // `attr(key, value)` sets it, as Jsoup's does, and gives the element.
      if (value.length && typeof value[0] === 'string') {
        dom('setAttr', { nodes: [this.h], name: K.str(name), value: value[0] });
        this._node = undefined;
        return this;
      }
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
    /** Jsoup's `textNodes()`: the element's own text children. */
    textNodes() {
      return dom('textNodes', { nodes: [this.h] }).map((found) => textNode(found, this.doc));
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
    /** Jsoup's `previousElementSiblings()`: the nearest first. */
    previousElementSiblings() {
      return Elements.of(dom('siblings', { nodes: [this.h], forward: false, all: true }), this.doc);
    }
    getElementsByTag(name) {
      return this.select(K.str(name).replace(/[^A-Za-z0-9_-]/g, '') || '*');
    }
    getElementsByClass(name) {
      return this.select(`.${K.str(name).replace(/([^A-Za-z0-9_-])/g, '\\$1')}`);
    }
    getElementById(id) {
      return this.selectFirst(`[id="${K.str(id).replace(/"/g, '\\"')}"]`);
    }
    /** Jsoup's `wholeOwnText()`: the element's own text children, as written. */
    wholeOwnText() {
      return this.textNodes()
        .map((node) => node.wholeText)
        .join('');
    }
    /** Jsoup's `childNodes()`: elements and text, in order (comments left out). */
    childNodes() {
      const handles = dom('children', { nodes: [this.h], all: true });
      const infos = handles.length ? dom('info', { nodes: handles }) : [];
      const elementAfter = (index, step) => {
        for (let at = index + step; at >= 0 && at < handles.length; at += step)
          if (['tag', 'script', 'style'].includes(infos[at].type)) return handles[at];
        return null;
      };
      const out = [];
      handles.forEach((handle, index) => {
        const info = infos[index];
        if (info.type === 'text')
          out.push(
            textNode(
              {
                text: info.data,
                next: elementAfter(index, 1),
                previous: elementAfter(index, -1),
              },
              this.doc,
            ),
          );
        else if (['tag', 'script', 'style'].includes(info.type))
          out.push(new Element(handle, this.doc));
      });
      return out;
    }
    childNodeSize() {
      return this.childNodes().length;
    }
    /** Jsoup's `getElementsContainingText(text)`: elements whose text has it. */
    getElementsContainingText(text) {
      return this.select(`*:contains(${K.str(text).replace(/[()]/g, '\\$&')})`);
    }
    /** Jsoup's `siblingIndex()`: its place among its parent's child nodes. */
    siblingIndex() {
      const parent = this.parent();
      if (!parent) return 0;
      return Math.max(
        0,
        parent.childNodes().findIndex((node) => node instanceof Element && node.h === this.h),
      );
    }
    /** Jsoup's `tag()`: the element's tag, by name. */
    tag() {
      const name = this.tagName();
      return {
        getName: () => name,
        normalName: () => name.toLowerCase(),
        name,
        toString: () => name,
      };
    }
    /** Jsoup's `insertChildren(index, nodes)`: put before the element child at `index`. */
    insertChildren(index, nodes) {
      const list = Array.isArray(nodes) ? nodes : [...nodes];
      const kids = this.children();
      const at = index < 0 ? kids.length + 1 + index : index;
      if (at >= kids.length) for (const node of list) this.append(node);
      else for (const node of list) kids[at].before(node);
      return this;
    }
    /** Jsoup's `val()`: a form field's value. */
    val() {
      return this.tagName() === 'textarea' ? this.text() : this.attr('value');
    }
    /** Jsoup's `setBaseUri(uri)`: the address relative links are read against. */
    setBaseUri(uri) {
      if (this.doc) this.doc.loc = K.str(uri);
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
    /** Jsoup's `createElement(tag)`: an element of its own with this document's address, for
     * `attr("href", …)` then `absUrl("href")`. */
    createElement(tag) {
      const name = K.str(tag).replace(/[^a-zA-Z0-9-]/g, '') || 'div';
      const holder = new Document(
        dom('load', { html: `<body><${name}></${name}></body>` }),
        this.loc,
      );
      const element = holder.selectFirst(name);
      return element ?? holder;
    }
  }

  const Jsoup = Object.freeze({
    /** A page; with `xml`, an XML document (`Parser.xmlParser()`). */
    parse: (html, location = '', xml = false) =>
      new Document(dom('load', { html: K.str(html), xml: Boolean(xml) }), location),
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
    /** The answer's bytes (a `Uint8Array`): as they came, or the text's UTF-8. */
    async bytes() {
      const bytes = this.raw.bytes ? await this.raw.bytes() : null;
      return bytes ?? K.bin.utf8(await this.raw.text());
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
        redirect = null,
      } = {},
    ) {
      await this.pace();
      const merged = { ...this.headers(kind, from), ...headers };
      let payload = body;
      let bodyBase64 = null;
      if (form) {
        payload = K.form(form);
        merged['Content-Type'] = 'application/x-www-form-urlencoded';
      } else if (payload instanceof Uint8Array) {
        // A body of bytes (protobuf, a file).
        bodyBase64 = K.bin.toBase64(payload);
        payload = null;
      } else if (payload !== null && typeof payload === 'object') {
        payload = JSON.stringify(payload);
        merged['Content-Type'] = merged['Content-Type'] || 'application/json';
      }
      // Answers that aren't text keep their bytes (pictures, protobuf).
      const raw = await raygun.http.fetch(address, {
        method,
        headers: merged,
        body: payload,
        bodyBase64,
        binary: true,
        redirect: redirect ?? undefined,
      });
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
    textNode: (text) => textNode({ text: K.str(text) }),
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
