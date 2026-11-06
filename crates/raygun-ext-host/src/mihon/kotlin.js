// The runtime of sources converted from Kotlin (`crate::mihon::transpile`, `kotlin::emit`): what
// their code calls, as tables by the kind of value (strings, lists, maps, Jsoup's documents,
// OkHttp's requests and answers, Mihon's models), and `mihon.source`, which answers Raygun's
// `manga.source` calls with a converted source's own functions (`getPopularManga`,
// `fetchMangaUpdate`, `getPageList`…). Method names here match `kotlin::emit::METHODS`.
'use strict';
(() => {
  const { K, Jsoup, Http, HttpError, toDetails } = mihon;
  const KT = {};
  const NAMED = Symbol('named');

  class Return {
    constructor(value, label) {
      this.value = value;
      this.label = label;
    }
  }
  KT.Return = Return;
  KT.unit = undefined;
  KT.ret = (value, label) => {
    throw new Return(value, label);
  };

  class KotlinError extends Error {
    constructor(message, kind = 'Exception') {
      super(message == null ? kind : K.str(message));
      this.kind = kind;
    }
  }
  KT.error = (value) => (value instanceof Error ? value : new KotlinError(value));
  KT.raise = (value) => {
    throw KT.error(value);
  };
  KT.caught = (error, type) => {
    if (error instanceof Return) return false;
    const name = K.str(type).split('.').pop();
    if (
      ['Exception', 'Throwable', 'Error', 'RuntimeException', 'IOException', 'Any'].includes(name)
    )
      return true;
    return (
      (error && error.kind === name) ||
      name === 'IllegalStateException' ||
      name === 'IllegalArgumentException'
    );
  };
  KT.nn = (value) => {
    if (value === null || value === undefined)
      throw new KotlinError(
        'a value was null where the source expected one',
        'NullPointerException',
      );
    return value;
  };
  KT.truthy = (value) =>
    value === true ||
    (value !== false && value !== null && value !== undefined && value !== 0 && value !== ''
      ? Boolean(value)
      : false);
  KT.str = (value) => {
    if (
      value &&
      typeof value === 'object' &&
      typeof value.toString === 'function' &&
      value.toString !== Object.prototype.toString
    )
      return value.toString();
    return K.str(value);
  };
  KT.plus = (left, right) => {
    if (typeof left === 'string') return left + KT.str(right);
    if (Array.isArray(left))
      return Array.isArray(right) ? left.concat(right) : left.concat([right]);
    if (left instanceof Map)
      return new Map([...left, ...(right instanceof Map ? right : [[right.first, right.second]])]);
    if (left && typeof left.plus === 'function') return left.plus(right);
    return left + right;
  };
  KT.minus = (left, right) => {
    if (Array.isArray(left)) {
      const drop = Array.isArray(right) ? right : [right];
      return left.filter((item) => !drop.some((other) => KT.eq(item, other)));
    }
    return left - right;
  };
  KT.plusAssign = (current, value) => {
    if (Array.isArray(current)) {
      if (Array.isArray(value)) current.push(...value);
      else current.push(value);
      return current;
    }
    if (current instanceof Map) {
      const pairs = value instanceof Map ? [...value] : [[value.first, value.second]];
      for (const [key, item] of pairs) current.set(key, item);
      return current;
    }
    if (current && current.isStringBuilder) return current.append(value);
    return KT.plus(current, value);
  };
  KT.minusAssign = (current, value) => {
    if (Array.isArray(current)) {
      const index = current.findIndex((item) => KT.eq(item, value));
      if (index >= 0) current.splice(index, 1);
      return current;
    }
    return current - value;
  };
  KT.div = K.div;
  KT.eq = (left, right) => {
    if (left === right) return true;
    if (left == null || right == null) return left == null && right == null;
    if (left && left.isPair && right && right.isPair)
      return KT.eq(left.first, right.first) && KT.eq(left.second, right.second);
    if (left && left.__fields && right && right.__fields)
      return left.__fields.every((field) => KT.eq(left[field], right[field]));
    return K.eq(left, right);
  };
  KT.cmp = (left, right) => {
    if (typeof left === 'string' && typeof right === 'string')
      return left < right ? -1 : left > right ? 1 : 0;
    return left - right;
  };
  KT.and = (left, right) => (typeof left === 'boolean' ? left && right : left & right);
  KT.or = (left, right) => (typeof left === 'boolean' ? left || right : left | right);

  KT.pair = (first, second) => ({
    first,
    second,
    isPair: true,
    toString: () => `(${KT.str(first)}, ${KT.str(second)})`,
  });
  KT.component = (value, index) => {
    if (value == null) throw new KotlinError('destructured a null value', 'NullPointerException');
    if (value.isPair) return index === 0 ? value.first : index === 1 ? value.second : value.third;
    if (Array.isArray(value)) return value[index];
    if (value.isIndexed) return index === 0 ? value.index : value.value;
    if (value.isEntry) return index === 0 ? value.key : value.value;
    if (value.__fields) return value[value.__fields[index]];
    if (value.isMatchGroups) return value.groupValues[index + 1];
    throw new KotlinError('this value has no parts to destructure');
  };
  KT.range = (from, to, step = 1) => ({ isRange: true, first: from, last: to, step });
  KT.step = (range, step) => ({ ...range, step: range.step < 0 ? -step : step });
  KT.iter = (value) => {
    if (value == null) return [];
    if (value.isRange) {
      const out = [];
      if (value.step > 0)
        for (let index = value.first; index <= value.last; index += value.step) out.push(index);
      else for (let index = value.first; index >= value.last; index += value.step) out.push(index);
      return out;
    }
    if (value instanceof Map)
      return [...value].map(([key, item]) => ({ key, value: item, isEntry: true }));
    if (typeof value === 'string') return [...value];
    if (Array.isArray(value)) return value;
    if (value.isMatchSequence) return value.matches;
    if (typeof value === 'object' && !value.__class)
      return Object.entries(value).map(([key, item]) => ({ key, value: item, isEntry: true }));
    if (typeof value[Symbol.iterator] === 'function') return [...value];
    throw new KotlinError('this value can’t be gone through');
  };
  KT.contains = (container, item) => {
    if (container && container.isRange) return item >= container.first && item <= container.last;
    if (
      container &&
      typeof container === 'object' &&
      !Array.isArray(container) &&
      !(container instanceof Map) &&
      !container.__class
    )
      return Object.prototype.hasOwnProperty.call(container, item);
    return K.contains(container, item);
  };
  KT.safe = async (value, then) => (value === null || value === undefined ? null : then(value));
  KT.also = async (value, then) => {
    await then();
    return value;
  };
  KT.runCatching = async (work) => {
    try {
      return { isSuccess: true, value: await work(), isResult: true };
    } catch (error) {
      if (error instanceof Return) throw error;
      return { isSuccess: false, error, isResult: true };
    }
  };
  KT.repeat = async (times, work) => {
    for (let index = 0; index < times; index++) await work(index);
  };
  KT.deferred = (work) => {
    let promise = null;
    return {
      isDeferred: true,
      await: () => (promise ??= work()),
      get value() {
        return (promise ??= work());
      },
    };
  };
  KT.invoke = async (function_, args) => function_(...args);
  KT.getter = (function_) => ({ isGetter: true, function_ });

  // ---- What a value is ----

  const kindOf = (value) => {
    if (typeof value === 'string') return 'string';
    if (typeof value === 'number') return 'number';
    if (typeof value === 'boolean') return 'boolean';
    if (typeof value === 'function') return 'function';
    if (value instanceof mihon.Elements) return 'elements';
    if (Array.isArray(value)) return 'list';
    if (value instanceof Map) return 'map';
    if (value instanceof Set) return 'set';
    return 'object';
  };

  const BUILT_IN_TYPES = {
    String: (value) => typeof value === 'string',
    CharSequence: (value) => typeof value === 'string',
    Int: Number.isInteger,
    Long: Number.isInteger,
    Number: (value) => typeof value === 'number',
    Double: (value) => typeof value === 'number',
    Float: (value) => typeof value === 'number',
    Boolean: (value) => typeof value === 'boolean',
    List: Array.isArray,
    MutableList: Array.isArray,
    Collection: Array.isArray,
    Iterable: Array.isArray,
    Array: Array.isArray,
    Map: (value) =>
      value instanceof Map ||
      (value && typeof value === 'object' && !Array.isArray(value) && !value.__class),
    JsonObject: (value) =>
      value && typeof value === 'object' && !Array.isArray(value) && !value.__class,
    JsonArray: Array.isArray,
    JsonPrimitive: (value) =>
      value === null || ['string', 'number', 'boolean'].includes(typeof value),
    JsonNull: (value) => value === null,
    JsonElement: () => true,
    Element: (value) => value && typeof value.select === 'function' && !Array.isArray(value),
    Document: (value) => value && typeof value.location === 'function',
    Any: (value) => value !== null && value !== undefined,
  };
  KT.is = (value, type) => {
    const name = K.str(type).split('<')[0].split('.').pop();
    if (BUILT_IN_TYPES[name]) return Boolean(BUILT_IN_TYPES[name](value));
    return Boolean(value && value.__classes && value.__classes.includes(name));
  };
  KT.asOrNull = (value, type) => (KT.is(value, type) ? value : null);

  // ---- Strings ----

  const ignoreCase = (named, args, position) =>
    Boolean((named && named.ignoreCase) ?? args[position]);
  const regexOf = (value, flags = 'g') => {
    if (value instanceof RegExp)
      return new RegExp(
        value.source,
        value.flags.includes('g') || !flags.includes('g') ? value.flags : value.flags + 'g',
      );
    return null;
  };
  const toInt = (text) => {
    const trimmed = K.str(text).trim();
    if (!/^[+-]?\d+$/.test(trimmed)) return null;
    return parseInt(trimmed, 10);
  };
  const toFloat = (text) => {
    const trimmed = K.str(text).trim();
    if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(trimmed)) return null;
    return parseFloat(trimmed);
  };
  const STRING = {
    substringAfter: (s, [delimiter, missing = s]) =>
      K.substringAfter(s, KT.str(delimiter), missing),
    substringBefore: (s, [delimiter, missing = s]) =>
      K.substringBefore(s, KT.str(delimiter), missing),
    substringAfterLast: (s, [delimiter, missing = s]) =>
      K.substringAfterLast(s, KT.str(delimiter), missing),
    substringBeforeLast: (s, [delimiter, missing = s]) =>
      K.substringBeforeLast(s, KT.str(delimiter), missing),
    removePrefix: (s, [prefix]) => K.removePrefix(s, KT.str(prefix)),
    removeSuffix: (s, [suffix]) => K.removeSuffix(s, KT.str(suffix)),
    removeSurrounding: (s, [prefix, suffix = prefix]) =>
      s.startsWith(prefix) && s.endsWith(suffix) && s.length >= prefix.length + suffix.length
        ? s.slice(prefix.length, s.length - suffix.length)
        : s,
    trim: (s, args, lambda) =>
      args.length ? trimChars(s, args, true, true) : lambda ? trimWith(s, lambda) : s.trim(),
    trimStart: (s, args) => (args.length ? trimChars(s, args, true, false) : s.trimStart()),
    trimEnd: (s, args) => (args.length ? trimChars(s, args, false, true) : s.trimEnd()),
    split: (s, args, _lambda, named) => {
      const limit = (named && named.limit) ?? 0;
      let parts;
      const regex = regexOf(args[0]);
      if (regex) parts = s.split(new RegExp(regex.source, regex.flags.replace('g', '')));
      else if (args.length > 1 && typeof args[1] === 'string')
        parts = splitMany(
          s,
          args.filter((arg) => typeof arg === 'string'),
        );
      else parts = s.split(KT.str(args[0]));
      if (limit > 0 && parts.length > limit)
        parts = [...parts.slice(0, limit - 1), parts.slice(limit - 1).join(KT.str(args[0]))];
      return parts;
    },
    lowercase: (s) => s.toLowerCase(),
    uppercase: (s) => s.toUpperCase(),
    contains: (s, args, _lambda, named) => {
      const regex = regexOf(args[0], '');
      if (regex) return regex.test(s);
      return ignoreCase(named, args, 1)
        ? s.toLowerCase().includes(KT.str(args[0]).toLowerCase())
        : s.includes(KT.str(args[0]));
    },
    startsWith: (s, args, _lambda, named) =>
      ignoreCase(named, args, 1)
        ? s.toLowerCase().startsWith(KT.str(args[0]).toLowerCase())
        : s.startsWith(KT.str(args[0])),
    endsWith: (s, args, _lambda, named) =>
      ignoreCase(named, args, 1)
        ? s.toLowerCase().endsWith(KT.str(args[0]).toLowerCase())
        : s.endsWith(KT.str(args[0])),
    replace: async (s, args, lambda, named) => {
      const regex = regexOf(args[0]);
      if (regex) {
        if (lambda) {
          let out = '';
          let last = 0;
          for (const match of s.matchAll(regex)) {
            out += s.slice(last, match.index) + KT.str(await lambda(matchResult(match)));
            last = match.index + match[0].length;
          }
          return out + s.slice(last);
        }
        return s.replace(regex, KT.str(args[1]).replace(/\$(\d)/g, '$$$1'));
      }
      if (ignoreCase(named, args, 2))
        return s.replace(new RegExp(escapeRegex(KT.str(args[0])), 'gi'), () => KT.str(args[1]));
      return K.replace(s, args[0], args[1]);
    },
    replaceFirst: (s, args) => {
      const regex = regexOf(args[0], '');
      if (regex)
        return s.replace(new RegExp(regex.source, regex.flags.replace('g', '')), KT.str(args[1]));
      return s.replace(KT.str(args[0]), () => KT.str(args[1]));
    },
    isEmpty: (s) => s.length === 0,
    isNotEmpty: (s) => s.length > 0,
    isBlank: (s) => s.trim() === '',
    isNotBlank: (s) => s.trim() !== '',
    isNullOrBlank: (s) => s.trim() === '',
    isNullOrEmpty: (s) => s.length === 0,
    ifBlank: async (s, args, lambda) => (s.trim() === '' ? (lambda ? lambda() : args[0]) : s),
    ifEmpty: async (s, args, lambda) => (s.length === 0 ? (lambda ? lambda() : args[0]) : s),
    orEmpty: (s) => s,
    toInt: (s) => KT.nn(toInt(s)),
    toIntOrNull: (s) => toInt(s),
    toLong: (s) => KT.nn(toInt(s)),
    toLongOrNull: (s) => toInt(s),
    toFloat: (s) => KT.nn(toFloat(s)),
    toFloatOrNull: (s) => toFloat(s),
    toDouble: (s) => KT.nn(toFloat(s)),
    toDoubleOrNull: (s) => toFloat(s),
    toBoolean: (s) => s.toLowerCase() === 'true',
    substring: (s, [start, end]) => (end === undefined ? s.slice(start) : s.slice(start, end)),
    take: (s, [count]) => s.slice(0, count),
    drop: (s, [count]) => s.slice(count),
    takeLast: (s, [count]) => s.slice(Math.max(0, s.length - count)),
    dropLast: (s, [count]) => s.slice(0, Math.max(0, s.length - count)),
    indexOf: (s, [text, from = 0]) => s.indexOf(KT.str(text), from),
    lastIndexOf: (s, [text]) => s.lastIndexOf(KT.str(text)),
    padStart: (s, [length, char = ' ']) => s.padStart(length, char),
    padEnd: (s, [length, char = ' ']) => s.padEnd(length, char),
    repeat: (s, [times]) => s.repeat(times),
    lines: (s) => s.split(/\r\n|\r|\n/),
    matches: (s, [regex]) => {
      const pattern = regex instanceof RegExp ? regex : K.regex(regex);
      return new RegExp(`^(?:${pattern.source})$`, pattern.flags.replace('g', '')).test(s);
    },
    toRegex: (s, args) => K.regex(s, ...args),
    format: (s, args) => K.format(s, ...args),
    capitalize: (s) => s.charAt(0).toUpperCase() + s.slice(1),
    replaceFirstChar: async (s, _args, lambda) =>
      s ? KT.str(await lambda(s.charAt(0))) + s.slice(1) : s,
    titlecase: (s) => s.toUpperCase(),
    toHttpUrl: (s) => HttpUrl.parse(s),
    toHttpUrlOrNull: (s) => {
      try {
        return HttpUrl.parse(s);
      } catch {
        return null;
      }
    },
    asJsoup: (s, [base = '']) => Jsoup.parse(s, KT.str(base)),
    parseAs: (s, _args, _lambda, _named, type) => decode(JSON.parse(s), type),
    trimIndent: (s) => K.trimIndent(s),
    trimMargin: (s) => K.trimMargin(s),
    toCharArray: (s) => [...s],
    chunked: (s, [size]) => s.match(new RegExp(`[\\s\\S]{1,${size}}`, 'g')) ?? [],
    count: async (s, _args, lambda) =>
      lambda ? (await mapAll([...s], lambda)).filter(KT.truthy).length : s.length,
    isDigit: (s) => /^\p{Nd}$/u.test(s),
    isLetter: (s) => /^\p{L}$/u.test(s),
    isLetterOrDigit: (s) => /^[\p{L}\p{Nd}]$/u.test(s),
    isWhitespace: (s) => /^\s$/.test(s),
    isUpperCase: (s) => s === s.toUpperCase() && s !== s.toLowerCase(),
    isLowerCase: (s) => s === s.toLowerCase() && s !== s.toUpperCase(),
    digitToInt: (s) => parseInt(s, 10),
    toRequestBody: (s, [type = 'text/plain']) => ({ isBody: true, text: s, type: KT.str(type) }),
    toJsonRequestBody: (s) => ({
      isBody: true,
      text: KT.str(s),
      type: 'application/json; charset=utf-8',
    }),
    toMediaType: (s) => s,
    toResponseBody: (s, [type]) => KT.textBody(s, type ?? null),
    toMediaTypeOrNull: (s) => s,
    encodeToByteArray: (s) => s,
    // Bytes are kept as the text they spell (`Base64.encodeToString(id.toByteArray(), …)`).
    toByteArray: (s) => s,
    decodeToString: (s) => s,
    compareTo: (s, [other]) => KT.cmp(s, other),
    reversed: (s) => [...s].reverse().join(''),
    first: (s) => s.charAt(0),
    last: (s) => s.charAt(s.length - 1),
    firstOrNull: (s) => s.charAt(0) || null,
    lastOrNull: (s) => s.charAt(s.length - 1) || null,
    get: (s, [index]) => s.charAt(index),
    getOrNull: (s, [index]) => s.charAt(index) || null,
    filter: async (s, _args, lambda) => (await filterAll([...s], lambda)).join(''),
    map: async (s, _args, lambda) => mapAll([...s], lambda),
    any: async (s, _args, lambda) => (await mapAll([...s], lambda)).some(KT.truthy),
    all: async (s, _args, lambda) => (await mapAll([...s], lambda)).every(KT.truthy),
    none: async (s, _args, lambda) => !(await mapAll([...s], lambda)).some(KT.truthy),
    forEach: async (s, _args, lambda) => {
      for (const char of s) await lambda(char);
    },
    code: (s) => s.codePointAt(0),
    plus: (s, [other]) => s + KT.str(other),
    toList: (s) => [...s],
    single: (s) => s,
    equals: (s, args, _lambda, named) =>
      ignoreCase(named, args, 1)
        ? s.toLowerCase() === KT.str(args[0]).toLowerCase()
        : s === args[0],
  };
  const escapeRegex = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const trimChars = (s, chars, start, end) => {
    const set = new Set(chars.flatMap((value) => [...KT.str(value)]));
    let from = 0;
    let to = s.length;
    if (start) while (from < to && set.has(s[from])) from++;
    if (end) while (to > from && set.has(s[to - 1])) to--;
    return s.slice(from, to);
  };
  const trimWith = async (s, lambda) => {
    const chars = [...s];
    let from = 0;
    let to = chars.length;
    while (from < to && KT.truthy(await lambda(chars[from]))) from++;
    while (to > from && KT.truthy(await lambda(chars[to - 1]))) to--;
    return chars.slice(from, to).join('');
  };
  const splitMany = (s, delimiters) => {
    const pattern = new RegExp(delimiters.map(escapeRegex).join('|'));
    return s.split(pattern);
  };

  // ---- Regular expressions ----

  const matchResult = (match) => ({
    isMatch: true,
    value: match[0],
    groupValues: [...match].map((group) => group ?? ''),
    groups: [...match].map((group) => (group === undefined ? null : { value: group })),
    range: { first: match.index, last: match.index + match[0].length - 1, isRange: true, step: 1 },
    destructured: { isMatchGroups: true, groupValues: [...match].map((group) => group ?? '') },
  });
  const REGEX = {
    find: (regex, [text, start = 0]) => {
      const pattern = new RegExp(regex.source, regex.flags.replace('g', '') + 'g');
      pattern.lastIndex = start;
      const match = pattern.exec(KT.str(text));
      return match ? matchResult(match) : null;
    },
    findAll: (regex, [text]) =>
      [
        ...KT.str(text).matchAll(
          new RegExp(regex.source, regex.flags.includes('g') ? regex.flags : regex.flags + 'g'),
        ),
      ].map(matchResult),
    containsMatchIn: (regex, [text]) =>
      new RegExp(regex.source, regex.flags.replace('g', '')).test(KT.str(text)),
    matches: (regex, [text]) =>
      new RegExp(`^(?:${regex.source})$`, regex.flags.replace('g', '')).test(KT.str(text)),
    matchEntire: (regex, [text]) => {
      const match = new RegExp(`^(?:${regex.source})$`, regex.flags.replace('g', '')).exec(
        KT.str(text),
      );
      return match ? matchResult(match) : null;
    },
    replace: (regex, [text, replacement]) => STRING.replace(KT.str(text), [regex, replacement]),
    split: (regex, [text]) =>
      KT.str(text).split(new RegExp(regex.source, regex.flags.replace('g', ''))),
    toString: (regex) => regex.source,
  };

  // ---- Collections ----

  const mapAll = async (items, lambda, indexed = false) => {
    const out = [];
    let index = 0;
    for (const item of items) out.push(await (indexed ? lambda(index++, item) : lambda(item)));
    return out;
  };
  const filterAll = async (items, lambda) => {
    const out = [];
    for (const item of items) if (KT.truthy(await lambda(item))) out.push(item);
    return out;
  };
  const keyOf = (value) =>
    value !== null && typeof value === 'object' ? JSON.stringify(value) : value;
  const sortBy = async (items, lambda, descending = false) => {
    const keyed = [];
    for (const item of items) keyed.push([await lambda(item), item]);
    keyed.sort((a, b) => KT.cmp(a[0], b[0]) * (descending ? -1 : 1));
    return keyed.map(([, item]) => item);
  };
  const noSuch = (what) => {
    throw new KotlinError(what, 'NoSuchElementException');
  };
  /// A comparator (`compareBy { … }.thenByDescending { … }`): keys to sort by, in order.
  const comparator = (keys) => ({
    isComparator: true,
    keys,
    thenBy: (select) => comparator([...keys, { select, descending: false }]),
    thenByDescending: (select) => comparator([...keys, { select, descending: true }]),
    reversed: () => comparator(keys.map((key) => ({ ...key, descending: !key.descending }))),
  });
  /// Items in the order a comparator, or a `Comparator { a, b -> … }`, gives.
  const sortWith = async (items, order) => {
    if (typeof order === 'function') {
      const out = [];
      for (const item of items) {
        let at = out.length;
        while (at > 0 && (await order(out[at - 1], item)) > 0) at--;
        out.splice(at, 0, item);
      }
      return out;
    }
    const keyed = [];
    for (const item of items) {
      const values = [];
      for (const key of order.keys) values.push(key.select ? await key.select(item) : item);
      keyed.push([values, item]);
    }
    keyed.sort((a, b) => {
      for (let index = 0; index < order.keys.length; index++) {
        const result = KT.cmp(a[0][index], b[0][index]);
        if (result) return order.keys[index].descending ? -result : result;
      }
      return 0;
    });
    return keyed.map(([, item]) => item);
  };
  /// `List(size) { index -> … }`.
  const listOfSize = async (size, init) => {
    const out = [];
    for (let index = 0; index < size; index++) out.push(init ? await init(index) : null);
    return out;
  };
  /// `DateTimeFormatterBuilder()…toFormatter()`: its pattern pieces, as one date format.
  const dateFormatterBuilder = () => {
    let pattern = '';
    const builder = {
      appendPattern: (text) => ((pattern += KT.str(text)), builder),
      appendLiteral: (text) => ((pattern += `'${KT.str(text).replace(/'/g, "''")}'`), builder),
      parseCaseInsensitive: () => builder,
      parseCaseSensitive: () => builder,
      parseLenient: () => builder,
      parseStrict: () => builder,
      optionalStart: () => builder,
      optionalEnd: () => builder,
      parseDefaulting: () => builder,
      toFormatter: (locale) => K.dateFormat(pattern, locale),
    };
    return builder;
  };
  const LIST = {
    size: (l) => l.length,
    map: async (l, _args, lambda) => mapAll(l, lambda),
    mapNotNull: async (l, _args, lambda) =>
      (await mapAll(l, lambda)).filter((value) => value !== null && value !== undefined),
    mapIndexed: async (l, _args, lambda) => mapAll(l, lambda, true),
    mapIndexedNotNull: async (l, _args, lambda) =>
      (await mapAll(l, lambda, true)).filter((value) => value !== null && value !== undefined),
    flatMap: async (l, _args, lambda) =>
      (await mapAll(l, lambda)).flatMap((value) => KT.iter(value)),
    filter: async (l, _args, lambda) => filterAll(l, lambda),
    filterNot: async (l, _args, lambda) => {
      const out = [];
      for (const item of l) if (!KT.truthy(await lambda(item))) out.push(item);
      return out;
    },
    filterIndexed: async (l, _args, lambda) => {
      const out = [];
      for (let index = 0; index < l.length; index++)
        if (KT.truthy(await lambda(index, l[index]))) out.push(l[index]);
      return out;
    },
    filterNotNull: (l) => l.filter((value) => value !== null && value !== undefined),
    filterIsInstance: (l, _args, _lambda, _named, type) => l.filter((value) => KT.is(value, type)),
    awaitAll: (l) => awaitEach(l),
    firstInstanceOrNull: (l, _args, _lambda, _named, type) =>
      l.find((value) => KT.is(value, type)) ?? null,
    firstInstance: (l, _args, _lambda, _named, type) =>
      l.find((value) => KT.is(value, type)) ?? noSuch(`no ${type}`),
    mapTo: async (l, [destination], lambda) => {
      for (const item of l) destination.push(await lambda(item));
      return destination;
    },
    mapNotNullTo: async (l, [destination], lambda) => {
      for (const item of l) {
        const value = await lambda(item);
        if (value !== null && value !== undefined) destination.push(value);
      }
      return destination;
    },
    filterTo: async (l, [destination], lambda) => {
      for (const item of l) if (KT.truthy(await lambda(item))) destination.push(item);
      return destination;
    },
    forEach: async (l, _args, lambda) => {
      for (const item of l) await lambda(item);
    },
    forEachIndexed: async (l, _args, lambda) => {
      for (let index = 0; index < l.length; index++) await lambda(index, l[index]);
    },
    onEach: async (l, _args, lambda) => {
      for (const item of l) await lambda(item);
      return l;
    },
    first: async (l, _args, lambda) => {
      if (lambda) {
        for (const item of l) if (KT.truthy(await lambda(item))) return item;
        return noSuch('no item matched');
      }
      return l.length ? l[0] : noSuch('the list is empty');
    },
    firstOrNull: async (l, _args, lambda) => {
      if (lambda) {
        for (const item of l) if (KT.truthy(await lambda(item))) return item;
        return null;
      }
      return l.length ? l[0] : null;
    },
    find: async (l, args, lambda) => LIST.firstOrNull(l, args, lambda),
    last: async (l, _args, lambda) => {
      if (lambda) {
        for (let index = l.length - 1; index >= 0; index--)
          if (KT.truthy(await lambda(l[index]))) return l[index];
        return noSuch('no item matched');
      }
      return l.length ? l[l.length - 1] : noSuch('the list is empty');
    },
    lastOrNull: async (l, _args, lambda) => {
      if (lambda) {
        for (let index = l.length - 1; index >= 0; index--)
          if (KT.truthy(await lambda(l[index]))) return l[index];
        return null;
      }
      return l.length ? l[l.length - 1] : null;
    },
    findLast: async (l, args, lambda) => LIST.lastOrNull(l, args, lambda),
    single: (l) => (l.length === 1 ? l[0] : noSuch('the list doesn’t hold one item')),
    singleOrNull: (l) => (l.length === 1 ? l[0] : null),
    firstNotNullOfOrNull: async (l, _args, lambda) => {
      for (const item of l) {
        const value = await lambda(item);
        if (value !== null && value !== undefined) return value;
      }
      return null;
    },
    firstNotNullOf: async (l, args, lambda) =>
      KT.nn(await LIST.firstNotNullOfOrNull(l, args, lambda)),
    any: async (l, _args, lambda) =>
      lambda ? (await mapAll(l, lambda)).some(KT.truthy) : l.length > 0,
    all: async (l, _args, lambda) => (await mapAll(l, lambda)).every(KT.truthy),
    none: async (l, _args, lambda) =>
      lambda ? !(await mapAll(l, lambda)).some(KT.truthy) : l.length === 0,
    count: async (l, _args, lambda) =>
      lambda ? (await mapAll(l, lambda)).filter(KT.truthy).length : l.length,
    isEmpty: (l) => l.length === 0,
    isNotEmpty: (l) => l.length > 0,
    isNullOrEmpty: (l) => l.length === 0,
    ifEmpty: async (l, args, lambda) => (l.length === 0 ? (lambda ? lambda() : args[0]) : l),
    orEmpty: (l) => l,
    get: (l, [index]) => (index >= 0 && index < l.length ? l[index] : noSuch(`no item ${index}`)),
    getOrNull: (l, [index]) => l[index] ?? null,
    getOrElse: async (l, [index], lambda) =>
      index >= 0 && index < l.length ? l[index] : lambda(index),
    elementAt: (l, [index]) => l[index],
    elementAtOrNull: (l, [index]) => l[index] ?? null,
    indexOf: (l, [item]) => l.findIndex((other) => KT.eq(other, item)),
    lastIndexOf: (l, [item]) => {
      for (let index = l.length - 1; index >= 0; index--) if (KT.eq(l[index], item)) return index;
      return -1;
    },
    indexOfFirst: async (l, _args, lambda) => {
      for (let index = 0; index < l.length; index++)
        if (KT.truthy(await lambda(l[index]))) return index;
      return -1;
    },
    indexOfLast: async (l, _args, lambda) => {
      for (let index = l.length - 1; index >= 0; index--)
        if (KT.truthy(await lambda(l[index]))) return index;
      return -1;
    },
    contains: (l, [item]) => l.some((other) => KT.eq(other, item)),
    joinToString: async (l, args, lambda, named) => {
      const options = named ?? {};
      const separator = options.separator ?? args[0] ?? ', ';
      const prefix = options.prefix ?? args[1] ?? '';
      const postfix = options.postfix ?? args[2] ?? '';
      const parts = [];
      for (const item of l) parts.push(KT.str(lambda ? await lambda(item) : item));
      return prefix + parts.join(KT.str(separator)) + postfix;
    },
    /// `joinTo(builder, separator, …)`: the same text, appended to `builder`.
    joinTo: async (l, [buffer, ...rest], lambda, named) => {
      const text = await LIST.joinToString(l, rest, lambda, named);
      buffer.append(text);
      return buffer;
    },
    take: (l, [count]) => l.slice(0, count),
    drop: (l, [count]) => l.slice(count),
    takeLast: (l, [count]) => l.slice(Math.max(0, l.length - count)),
    dropLast: (l, [count]) => l.slice(0, Math.max(0, l.length - count)),
    takeWhile: async (l, _args, lambda) => {
      const out = [];
      for (const item of l) {
        if (!KT.truthy(await lambda(item))) break;
        out.push(item);
      }
      return out;
    },
    dropWhile: async (l, _args, lambda) => {
      let index = 0;
      while (index < l.length && KT.truthy(await lambda(l[index]))) index++;
      return l.slice(index);
    },
    distinct: (l) => {
      const seen = new Set();
      return l.filter((item) => {
        const key = keyOf(item);
        return !seen.has(key) && seen.add(key);
      });
    },
    distinctBy: async (l, _args, lambda) => {
      const seen = new Set();
      const out = [];
      for (const item of l) {
        const key = keyOf(await lambda(item));
        if (!seen.has(key)) {
          seen.add(key);
          out.push(item);
        }
      }
      return out;
    },
    sorted: (l) => [...l].sort(KT.cmp),
    sortedDescending: (l) => [...l].sort((a, b) => KT.cmp(b, a)),
    sortedBy: async (l, _args, lambda) => sortBy(l, lambda),
    sortedByDescending: async (l, _args, lambda) => sortBy(l, lambda, true),
    sortedWith: async (l, [order], lambda) => sortWith(l, order ?? lambda),
    sortWith: async (l, [order], lambda) => {
      const sorted = await sortWith(l, order ?? lambda);
      l.splice(0, l.length, ...sorted);
    },
    sortBy: async (l, _args, lambda) => {
      const sorted = await sortBy(l, lambda);
      l.splice(0, l.length, ...sorted);
    },
    sortByDescending: async (l, _args, lambda) => {
      const sorted = await sortBy(l, lambda, true);
      l.splice(0, l.length, ...sorted);
    },
    reversed: (l) => [...l].reverse(),
    asReversed: (l) => [...l].reverse(),
    flatten: (l) => l.flatMap((value) => KT.iter(value)),
    groupBy: async (l, _args, lambda) => {
      const groups = new Map();
      for (const item of l) {
        const key = await lambda(item);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(item);
      }
      return groups;
    },
    associate: async (l, _args, lambda) =>
      new Map((await mapAll(l, lambda)).map((pair) => [pair.first, pair.second])),
    associateBy: async (l, _args, lambda) => {
      const map = new Map();
      for (const item of l) map.set(await lambda(item), item);
      return map;
    },
    associateWith: async (l, _args, lambda) => {
      const map = new Map();
      for (const item of l) map.set(item, await lambda(item));
      return map;
    },
    toMap: (l) => new Map(l.map((pair) => [pair.first, pair.second])),
    toList: (l) => [...l],
    toMutableList: (l) => [...l],
    toTypedArray: (l) => [...l],
    toByteArray: (l) => [...l],
    toIntArray: (l) => [...l],
    toSet: (l) => LIST.distinct(l),
    toMutableSet: (l) => LIST.distinct(l),
    asSequence: (l) => l,
    asIterable: (l) => l,
    sum: (l) => l.reduce((total, value) => total + value, 0),
    sumOf: async (l, _args, lambda) =>
      (await mapAll(l, lambda)).reduce((total, value) => total + value, 0),
    maxOf: async (l, _args, lambda) => Math.max(...(await mapAll(l, lambda))),
    minOf: async (l, _args, lambda) => Math.min(...(await mapAll(l, lambda))),
    maxOrNull: (l) =>
      l.length ? l.reduce((best, value) => (KT.cmp(value, best) > 0 ? value : best)) : null,
    minOrNull: (l) =>
      l.length ? l.reduce((best, value) => (KT.cmp(value, best) < 0 ? value : best)) : null,
    max: (l) => KT.nn(LIST.maxOrNull(l)),
    min: (l) => KT.nn(LIST.minOrNull(l)),
    maxByOrNull: async (l, _args, lambda) => (l.length ? (await sortBy(l, lambda, true))[0] : null),
    minByOrNull: async (l, _args, lambda) => (l.length ? (await sortBy(l, lambda))[0] : null),
    maxBy: async (l, _args, lambda) =>
      l.length ? (await sortBy(l, lambda, true))[0] : noSuch('an empty list'),
    minBy: async (l, _args, lambda) =>
      l.length ? (await sortBy(l, lambda))[0] : noSuch('an empty list'),
    maxOfOrNull: async (l, _args, lambda) => LIST.maxOrNull(await mapAll(l, lambda)),
    minOfOrNull: async (l, _args, lambda) => LIST.minOrNull(await mapAll(l, lambda)),
    average: (l) => (l.length ? l.reduce((total, value) => total + value, 0) / l.length : NaN),
    reduce: async (l, _args, lambda) => {
      let total = l[0];
      for (let index = 1; index < l.length; index++) total = await lambda(total, l[index]);
      return total;
    },
    fold: async (l, [initial], lambda) => {
      let total = initial;
      for (const item of l) total = await lambda(total, item);
      return total;
    },
    chunked: (l, [size]) => {
      const out = [];
      for (let index = 0; index < l.length; index += size) out.push(l.slice(index, index + size));
      return out;
    },
    windowed: (l, [size, step = 1]) => {
      const out = [];
      for (let index = 0; index + size <= l.length; index += step)
        out.push(l.slice(index, index + size));
      return out;
    },
    zip: async (l, [other], lambda) => {
      const out = [];
      const second = KT.iter(other);
      for (let index = 0; index < Math.min(l.length, second.length); index++) {
        out.push(lambda ? await lambda(l[index], second[index]) : KT.pair(l[index], second[index]));
      }
      return out;
    },
    withIndex: (l) => l.map((value, index) => ({ index, value, isIndexed: true })),
    partition: async (l, _args, lambda) => {
      const yes = [];
      const no = [];
      for (const item of l) (KT.truthy(await lambda(item)) ? yes : no).push(item);
      return KT.pair(yes, no);
    },
    plus: (l, [other]) => KT.plus(l, other),
    minus: (l, [other]) => KT.minus(l, other),
    plusElement: (l, [item]) => [...l, item],
    add: (l, args) => {
      if (args.length === 2) l.splice(args[0], 0, args[1]);
      else l.push(args[0]);
      return true;
    },
    addAll: (l, [items]) => {
      l.push(...KT.iter(items));
      return true;
    },
    remove: (l, [item]) => {
      const index = l.findIndex((other) => KT.eq(other, item));
      if (index >= 0) l.splice(index, 1);
      return index >= 0;
    },
    removeAt: (l, [index]) => l.splice(index, 1)[0],
    removeAll: async (l, args, lambda) => {
      const drop = lambda
        ? await mapAll(l, lambda)
        : l.map((item) => KT.iter(args[0]).some((other) => KT.eq(item, other)));
      for (let index = l.length - 1; index >= 0; index--)
        if (KT.truthy(drop[index])) l.splice(index, 1);
      return true;
    },
    removeIf: async (l, args, lambda) => LIST.removeAll(l, args, lambda),
    clear: (l) => {
      l.length = 0;
    },
    set: (l, [index, value]) => {
      l[index] = value;
    },
    subList: (l, [from, to]) => l.slice(from, to),
    slice: (l, [range]) => KT.iter(range).map((index) => l[index]),
    shuffled: (l) => [...l].sort(() => Math.random() - 0.5),
    random: (l) => l[Math.floor(Math.random() * l.length)],
    component1: (l) => l[0],
    component2: (l) => l[1],
    component3: (l) => l[2],
    lastIndex: (l) => l.length - 1,
    indices: (l) => KT.range(0, l.length - 1),
  };

  const MAP = {
    get: (m, [key]) => (m.has(key) ? m.get(key) : null),
    getOrDefault: (m, [key, fallback]) => (m.has(key) ? m.get(key) : fallback),
    getOrElse: async (m, [key], lambda) => (m.has(key) ? m.get(key) : lambda()),
    getOrPut: async (m, [key], lambda) => {
      if (!m.has(key)) m.set(key, await lambda());
      return m.get(key);
    },
    containsKey: (m, [key]) => m.has(key),
    containsValue: (m, [value]) => [...m.values()].some((other) => KT.eq(other, value)),
    put: (m, [key, value]) => {
      m.set(key, value);
    },
    set: (m, [key, value]) => {
      m.set(key, value);
    },
    putAll: (m, [other]) => {
      for (const entry of KT.iter(other))
        m.set(entry.key ?? entry.first, entry.value ?? entry.second);
    },
    putIfAbsent: (m, [key, value]) => {
      if (!m.has(key)) m.set(key, value);
    },
    remove: (m, [key]) => {
      const value = m.get(key);
      m.delete(key);
      return value ?? null;
    },
    isEmpty: (m) => m.size === 0,
    isNotEmpty: (m) => m.size > 0,
    forEach: async (m, _args, lambda) => {
      for (const entry of KT.iter(m)) await lambda(entry);
    },
    map: async (m, _args, lambda) => mapAll(KT.iter(m), lambda),
    mapNotNull: async (m, _args, lambda) =>
      (await mapAll(KT.iter(m), lambda)).filter((value) => value != null),
    filter: async (m, _args, lambda) =>
      new Map((await filterAll(KT.iter(m), lambda)).map((entry) => [entry.key, entry.value])),
    mapValues: async (m, _args, lambda) => {
      const out = new Map();
      for (const entry of KT.iter(m)) out.set(entry.key, await lambda(entry));
      return out;
    },
    mapKeys: async (m, _args, lambda) => {
      const out = new Map();
      for (const entry of KT.iter(m)) out.set(await lambda(entry), entry.value);
      return out;
    },
    toList: (m) => KT.iter(m).map((entry) => KT.pair(entry.key, entry.value)),
    toMap: (m) => new Map(m),
    toMutableMap: (m) => new Map(m),
    clear: (m) => m.clear(),
    entries: (m) => KT.iter(m),
    keys: (m) => [...m.keys()],
    values: (m) => [...m.values()],
    size: (m) => m.size,
  };

  /// A JSON object, as kotlinx.serialization's `JsonObject` or a DTO.
  const OBJECT = {
    get: (o, [key]) => (Object.prototype.hasOwnProperty.call(o, key) ? o[key] : null),
    getOrDefault: (o, [key, fallback]) => o[key] ?? fallback,
    containsKey: (o, [key]) => Object.prototype.hasOwnProperty.call(o, key),
    keys: (o) => Object.keys(o),
    values: (o) => Object.values(o),
    entries: (o) => KT.iter(o),
    isEmpty: (o) => Object.keys(o).length === 0,
    isNotEmpty: (o) => Object.keys(o).length > 0,
    size: (o) => Object.keys(o).length,
    forEach: async (o, _args, lambda) => {
      for (const entry of KT.iter(o)) await lambda(entry);
    },
    map: async (o, _args, lambda) => mapAll(KT.iter(o), lambda),
    getValue: (o, [key]) =>
      Object.prototype.hasOwnProperty.call(o, key) ? o[key] : noSuch(`no key ${key}`),
    // Keiyoushi's `JsonObject.getString("html")` and its kin.
    getString: (o, [key]) => (o[key] === null || o[key] === undefined ? null : KT.str(o[key])),
    getInt: (o, [key]) => (o[key] === null || o[key] === undefined ? null : toInt(o[key])),
    getLong: (o, [key]) => (o[key] === null || o[key] === undefined ? null : toInt(o[key])),
    getBoolean: (o, [key]) =>
      o[key] === null || o[key] === undefined ? null : o[key] === true || o[key] === 'true',
    getStringOrNull: (o, [key]) =>
      o[key] === null || o[key] === undefined ? null : KT.str(o[key]),
    getIntOrNull: (o, [key]) => (o[key] === null || o[key] === undefined ? null : toInt(o[key])),
    getLongOrNull: (o, [key]) => (o[key] === null || o[key] === undefined ? null : toInt(o[key])),
    getBooleanOrNull: (o, [key]) =>
      o[key] === null || o[key] === undefined ? null : o[key] === true || o[key] === 'true',
    toMap: (o) => new Map(Object.entries(o)),
    toMutableMap: (o) => new Map(Object.entries(o)),
  };

  const NUMBER = {
    toString: (n, [radix]) => (radix ? n.toString(radix) : K.str(n)),
    toInt: (n) => Math.trunc(n),
    toLong: (n) => Math.trunc(n),
    toFloat: (n) => n,
    toDouble: (n) => n,
    toChar: (n) => String.fromCharCode(n),
    toByte: (n) => (Math.trunc(n) << 24) >> 24,
    toShort: (n) => (Math.trunc(n) << 16) >> 16,
    roundToInt: (n) => Math.round(n),
    roundToLong: (n) => Math.round(n),
    coerceAtLeast: (n, [min]) => Math.max(n, min),
    coerceAtMost: (n, [max]) => Math.min(n, max),
    coerceIn: (n, [min, max]) => Math.min(Math.max(n, min), max),
    plus: (n, [other]) => n + other,
    minus: (n, [other]) => n - other,
    times: (n, [other]) => n * other,
    div: (n, [other]) => KT.div(n, other),
    rem: (n, [other]) => n % other,
    compareTo: (n, [other]) => n - other,
    time: (n) => n,
    getTime: (n) => n,
    before: (n, [other]) => n < msOf(other),
    after: (n, [other]) => n > msOf(other),
    toLocaleString: (n) => String(n),
  };

  const ANY = {
    toString: (value) => KT.str(value),
    equals: (value, [other]) => KT.eq(value, other),
    hashCode: (value) => {
      let hash = 0;
      for (const char of KT.str(value)) hash = (Math.imul(31, hash) + char.charCodeAt(0)) | 0;
      return hash;
    },
    use: async (value, _args, lambda) => lambda(value),
    closeQuietly: () => KT.unit,
    toJsonString: (value) => JSON.stringify(plain(value)),
    toJsonRequestBody: (value) => ({
      isBody: true,
      text: JSON.stringify(plain(value)),
      type: 'application/json; charset=utf-8',
    }),
    toJsonElement: (value) => plain(value),
    isNullOrBlank: (value) => K.isBlank(value),
    isNullOrEmpty: (value) => value == null || value.length === 0,
    orEmpty: (value) => value,
    not: (value) => !value,
    // Mihon's three-state filters.
    isIncluded: (value) => value.state === 1,
    isExcluded: (value) => value.state === 2,
    isIgnored: (value) => value.state === 0,
    toEpochMilliseconds: (value) => value.ms,
    toEpochMilli: (value) => value.ms,
    // Scope functions given a function reference (`?.let(::parse)`).
    let: async (value, args, lambda) => (lambda ?? args[0])(value),
    also: async (value, args, lambda) => {
      await (lambda ?? args[0])(value);
      return value;
    },
    takeIf: async (value, args, lambda) =>
      KT.truthy(await (lambda ?? args[0])(value)) ? value : null,
    takeUnless: async (value, args, lambda) =>
      KT.truthy(await (lambda ?? args[0])(value)) ? null : value,
  };

  /// Kotlin's extensions callable on null (`text?.isNullOrBlank()` reads `text.isNullOrBlank()`).
  const NULLABLE = {
    isNullOrBlank: () => true,
    isNullOrEmpty: () => true,
    orEmpty: (_value, _args, _lambda, _named, type) => (type && type.startsWith('List') ? [] : ''),
    toString: () => 'null',
    let: () => null,
    equals: (_value, [other]) => other == null,
    ifEmpty: async (_value, args, lambda) => (lambda ? lambda() : args[0]),
  };

  /// kotlinx.serialization's JSON elements, read off plain JSON values.
  const JSON_PROPS = {
    jsonObject: (value) =>
      value && typeof value === 'object' && !Array.isArray(value)
        ? value
        : KT.raise('not a JSON object'),
    jsonArray: (value) => (Array.isArray(value) ? value : KT.raise('not a JSON array')),
    jsonPrimitive: (value) =>
      value === null || typeof value !== 'object' ? value : KT.raise('not a JSON primitive'),
    jsonObjectOrNull: (value) =>
      value && typeof value === 'object' && !Array.isArray(value) ? value : null,
    jsonArrayOrNull: (value) => (Array.isArray(value) ? value : null),
    content: (value) => (value === null ? 'null' : KT.str(value)),
    contentOrNull: (value) => (value === null ? null : KT.str(value)),
    string: (value) => (value === null ? null : KT.str(value)),
    int: (value) => KT.nn(toInt(value)),
    intOrNull: (value) => (value === null ? null : toInt(value)),
    long: (value) => KT.nn(toInt(value)),
    longOrNull: (value) => (value === null ? null : toInt(value)),
    double: (value) => KT.nn(toFloat(value)),
    doubleOrNull: (value) => (value === null ? null : toFloat(value)),
    float: (value) => KT.nn(toFloat(value)),
    floatOrNull: (value) => (value === null ? null : toFloat(value)),
    boolean: (value) => value === true || value === 'true',
    booleanOrNull: (value) => (value === null ? null : value === true || value === 'true'),
    isString: (value) => typeof value === 'string',
    length: (value) => value.length,
    size: (value) => value.length,
    lastIndex: (value) => value.length - 1,
    indices: (value) => KT.range(0, value.length - 1),
  };

  // ---- Reading values ----

  let renames = {};
  /// Keiyoushi's `assets/i18n/messages_<lang>.properties`, by `<lang>` (`pt_br`).
  let messages = {};
  /// Keiyoushi's `Intl` (lib:i18n): messages in the source's language, else the base one's.
  const intl = (language, availableLanguages, baseLanguage) => {
    const snake = (lang) => KT.str(lang).replace(/-/g, '_').toLowerCase();
    const available = [...(availableLanguages ?? [])].map(KT.str);
    const chosen = available.includes(KT.str(language)) ? KT.str(language) : KT.str(baseLanguage);
    const bundle = messages[snake(chosen)] ?? {};
    const baseBundle = messages[snake(baseLanguage)] ?? {};
    const get = (key) => bundle[key] ?? baseBundle[key] ?? `[${key}]`;
    return {
      chosenLanguage: chosen,
      get,
      format: (key, ...args) => K.format(get(key), ...args),
      languageDisplayName: (code) => KT.str(code),
    };
  };
  /// A DTO's property by its Kotlin name, or the JSON name it's read by.
  const field = (object, name) => {
    if (Object.prototype.hasOwnProperty.call(object, name)) return object[name];
    const serial = renames[name];
    if (serial !== undefined && Object.prototype.hasOwnProperty.call(object, serial))
      return object[serial];
    return undefined;
  };

  /// A getter a class of the runtime defines (not a plain object's, not a method).
  const classGetter = (value, name) => {
    const prototype = Object.getPrototypeOf(value);
    if (
      prototype === null ||
      prototype === Object.prototype ||
      Object.prototype.hasOwnProperty.call(value, name)
    )
      return false;
    for (let at = prototype; at && at !== Object.prototype; at = Object.getPrototypeOf(at)) {
      const descriptor = Object.getOwnPropertyDescriptor(at, name);
      if (descriptor) return typeof descriptor.get === 'function';
    }
    return false;
  };
  KT.p = async (value, name) => {
    if (value === null || value === undefined)
      throw new KotlinError(`read ${name} of null`, 'NullPointerException');
    const kind = kindOf(value);
    if (kind === 'string' || kind === 'number' || kind === 'boolean') {
      if (name === 'length' && kind === 'string') return value.length;
      if (JSON_PROPS[name]) return JSON_PROPS[name](value);
      if (kind === 'number' && NUMBER[name]) return NUMBER[name](value, []);
      if (kind === 'string' && name === 'lastIndex') return value.length - 1;
      if (kind === 'string' && name === 'indices') return KT.range(0, value.length - 1);
      throw new KotlinError(`a value has no ${name}`);
    }
    if (kind === 'list' || kind === 'elements') {
      if (LIST[name] && ['size', 'lastIndex', 'indices'].includes(name)) return LIST[name](value);
      if (JSON_PROPS[name]) return JSON_PROPS[name](value);
    }
    if (kind === 'map') {
      if (['size', 'keys', 'values', 'entries'].includes(name)) return MAP[name](value);
    }
    if (kind === 'object') {
      if (value.isGetterHost && value.getters && value.getters[name])
        return value.getters[name](value);
      const own = field(value, name);
      if (own !== undefined) {
        if (own && own.isGetter) return own.function_(value);
        if (own && own.isDeferred) return own.value;
        return own;
      }
      // The runtime's classes' getters (`url.pathSegments`, `response.isSuccessful`).
      if (classGetter(value, name)) return value[name];
      if (value.__class === undefined && JSON_PROPS[name] && !['size', 'length'].includes(name))
        return JSON_PROPS[name](value);
      if (['size', 'keys', 'values', 'entries'].includes(name) && !value.__class)
        return OBJECT[name](value);
      if (value.isResult) {
        if (name === 'isSuccess') return value.isSuccess;
        if (name === 'isFailure') return !value.isSuccess;
      }
      const getter = value[`get${name.charAt(0).toUpperCase()}${name.slice(1)}`];
      if (typeof getter === 'function') return getter.call(value);
      if (value.__class !== undefined && !(name in value))
        throw new KotlinError(`${value.__class} has no ${name}`);
      return null;
    }
    throw new KotlinError(`a value has no ${name}`);
  };
  KT.ps = async (value, name) => (value === null || value === undefined ? null : KT.p(value, name));
  /// A callable reference's call (`GenreRoute::name`, `Element::text`): a property read when the
  /// value has one by that name, else the method.
  KT.member = async (receiver, name, args) => {
    if (
      args.length === 0 &&
      receiver !== null &&
      typeof receiver === 'object' &&
      !Array.isArray(receiver) &&
      !(receiver instanceof mihon.Elements)
    ) {
      const own = field(receiver, name);
      if ((own !== undefined && typeof own !== 'function') || classGetter(receiver, name))
        return KT.p(receiver, name);
    }
    return KT.m(receiver, name, args);
  };
  KT.setp = async (value, name, item) => {
    if (value === null || value === undefined)
      throw new KotlinError(`set ${name} of null`, 'NullPointerException');
    if (value.isGetterHost && value.setters && value.setters[name])
      return value.setters[name](value, item);
    value[name] = item;
  };

  KT.m = async (receiver, name, args = [], named = null, lambda = null, type = null) => {
    if (receiver === null || receiver === undefined) {
      if (NULLABLE[name]) return NULLABLE[name](receiver, args, lambda, named, type);
      throw new KotlinError(`${name}() was called on null`, 'NullPointerException');
    }
    const kind = kindOf(receiver);
    if (receiver instanceof RegExp && REGEX[name]) return REGEX[name](receiver, args, lambda);
    // `response.parseAs<List<MangaDto>>()`, `json.decodeFromString<T>(text)`: as the type asked.
    if (type && DECODERS.has(name) && kind === 'object' && typeof receiver[name] === 'function') {
      return decode(await receiver[name](...args), type);
    }
    if (kind === 'elements' && typeof receiver[name] === 'function' && ELEMENTS_OWN.has(name))
      return receiver[name](...args);
    const table = { string: STRING, list: LIST, elements: LIST, map: MAP, number: NUMBER }[kind];
    if (table && table[name]) return table[name](receiver, args, lambda, named, type);
    if (kind === 'object') {
      if (receiver.isMatch && name === 'groupValues') return receiver.groupValues;
      const own = receiver[name];
      if (typeof own === 'function') {
        const all = [...args];
        if (named) all.push({ [NAMED]: true, ...named });
        if (lambda) all.push(lambda);
        return own.call(receiver, ...all);
      }
      if (own && own.isMethod) return own.function_(receiver, args, lambda, named, type);
      if (receiver.__class === undefined && OBJECT[name])
        return OBJECT[name](receiver, args, lambda, named, type);
    }
    if (kind === 'function' && name === 'invoke') return receiver(...args);
    if (ANY[name]) return ANY[name](receiver, args, lambda, named, type);
    throw new KotlinError(`Raygun's runtime has no ${name}() for this value`);
  };
  const ELEMENTS_OWN = new Set([
    'first',
    'last',
    'text',
    'eachText',
    'attr',
    'hasAttr',
    'eachAttr',
    'html',
    'outerHtml',
    'select',
    'selectFirst',
    'not',
    'is',
    'remove',
    'isEmpty',
    'isNotEmpty',
  ]);
  const named = (args) => {
    const last = args[args.length - 1];
    return last && last[NAMED] ? args.pop() : {};
  };

  // ---- Builders ----

  KT.stringBuilder = () => {
    const builder = {
      isStringBuilder: true,
      text: '',
      append: (value) => {
        builder.text += KT.str(value);
        return builder;
      },
      appendLine: (value = '') => {
        builder.text += `${KT.str(value)}\n`;
        return builder;
      },
      insert: (at, value) => {
        builder.text = builder.text.slice(0, at) + KT.str(value) + builder.text.slice(at);
        return builder;
      },
      setLength: (length) => {
        builder.text = builder.text.slice(0, length);
      },
      toString: () => builder.text,
      length: 0,
    };
    Object.defineProperty(builder, 'length', { get: () => builder.text.length });
    return builder;
  };
  KT.jsonBuilder = () => {
    const object = {};
    return {
      isJsonBuilder: true,
      object,
      put: (key, value) => {
        object[key] = plain(value);
      },
      putJsonObject: async (key, lambda) => {
        const inner = KT.jsonBuilder();
        await lambda.call(null, inner);
        object[key] = inner.object;
      },
      putJsonArray: async (key, lambda) => {
        const inner = KT.jsonArrayBuilder();
        await lambda.call(null, inner);
        object[key] = inner.array;
      },
    };
  };
  KT.jsonArrayBuilder = () => {
    const array = [];
    return { isJsonBuilder: true, array, add: (value) => array.push(plain(value)) };
  };
  KT.built = (builder) => (builder.isJsonBuilder ? (builder.object ?? builder.array) : builder);
  const plain = (value) => {
    if (value === undefined) return null;
    if (value instanceof Map)
      return Object.fromEntries([...value].map(([key, item]) => [key, plain(item)]));
    if (Array.isArray(value)) return value.map(plain);
    if (value && value.__fields)
      return Object.fromEntries(
        value.__fields.map((name) => [renames[name] ?? name, plain(value[name])]),
      );
    return value;
  };

  // ---- OkHttp: addresses, requests, answers ----

  const encodeSegment = (text) => encodeURIComponent(KT.str(text)).replace(/%2F/gi, '%2F');
  class HttpUrl {
    constructor(parsed) {
      this.parts = parsed;
    }
    static parse(text) {
      return new HttpUrl(K.url(KT.str(text)));
    }
    get scheme() {
      return this.parts.scheme;
    }
    get host() {
      return this.parts.host;
    }
    get port() {
      return this.parts.port ?? (this.parts.scheme === 'https' ? 443 : 80);
    }
    get encodedPath() {
      return this.parts.path;
    }
    get pathSegments() {
      return this.parts.path
        .split('/')
        .slice(1)
        .map((part) => decodeURIComponent(part));
    }
    get encodedPathSegments() {
      return this.parts.path.split('/').slice(1);
    }
    get query() {
      return this.parts.query == null
        ? null
        : decodeURIComponent(this.parts.query.replace(/\+/g, ' '));
    }
    get encodedQuery() {
      return this.parts.query;
    }
    get fragment() {
      return this.parts.fragment;
    }
    get queryParameterNames() {
      return [...new Set(this.pairs().map(([key]) => key))];
    }
    pairs() {
      if (!this.parts.query) return [];
      return this.parts.query.split('&').map((pair) => {
        const at = pair.indexOf('=');
        const decode = (text) => decodeURIComponent(text.replace(/\+/g, ' '));
        return at < 0
          ? [decode(pair), null]
          : [decode(pair.slice(0, at)), decode(pair.slice(at + 1))];
      });
    }
    queryParameter(name) {
      const found = this.pairs().find(([key]) => key === name);
      return found ? found[1] : null;
    }
    queryParameterValues(name) {
      return this.pairs()
        .filter(([key]) => key === name)
        .map(([, value]) => value);
    }
    newBuilder() {
      return new HttpUrlBuilder(this);
    }
    resolve(link) {
      try {
        return HttpUrl.parse(K.resolve(link, this.toString()));
      } catch {
        return null;
      }
    }
    toString() {
      return this.parts.href;
    }
    equals(other) {
      return KT.str(other) === this.toString();
    }
  }
  class HttpUrlBuilder {
    constructor(url) {
      const parts = url
        ? url.parts
        : { scheme: 'https', host: '', port: null, path: '/', query: null, fragment: null };
      this.scheme_ = parts.scheme;
      this.host_ = parts.host;
      this.port_ = parts.port;
      this.segments = parts.path.split('/').slice(1);
      this.query = parts.query ? parts.query.split('&') : [];
      this.fragment_ = parts.fragment;
    }
    scheme(value) {
      this.scheme_ = value;
      return this;
    }
    host(value) {
      this.host_ = value;
      return this;
    }
    port(value) {
      this.port_ = value;
      return this;
    }
    addPathSegment(segment) {
      if (this.segments.length && this.segments[this.segments.length - 1] === '')
        this.segments.pop();
      this.segments.push(encodeSegment(segment));
      return this;
    }
    addEncodedPathSegment(segment) {
      if (this.segments.length && this.segments[this.segments.length - 1] === '')
        this.segments.pop();
      this.segments.push(KT.str(segment));
      return this;
    }
    addPathSegments(segments) {
      for (const segment of KT.str(segments).split('/')) {
        if (this.segments.length && this.segments[this.segments.length - 1] === '')
          this.segments.pop();
        this.segments.push(encodeSegment(segment));
      }
      return this;
    }
    addEncodedPathSegments(segments) {
      for (const segment of KT.str(segments).split('/')) {
        if (this.segments.length && this.segments[this.segments.length - 1] === '')
          this.segments.pop();
        this.segments.push(segment);
      }
      return this;
    }
    setPathSegment(index, segment) {
      this.segments[index] = encodeSegment(segment);
      return this;
    }
    encodedPath(path) {
      this.segments = KT.str(path).split('/').slice(1);
      return this;
    }
    addQueryParameter(name, value) {
      if (value === null || value === undefined) this.query.push(encodeURIComponent(KT.str(name)));
      else
        this.query.push(`${encodeURIComponent(KT.str(name))}=${encodeURIComponent(KT.str(value))}`);
      return this;
    }
    addEncodedQueryParameter(name, value) {
      this.query.push(value == null ? KT.str(name) : `${KT.str(name)}=${KT.str(value)}`);
      return this;
    }
    removeAllQueryParameters(name) {
      const key = encodeURIComponent(KT.str(name));
      this.query = this.query.filter((pair) => pair.split('=')[0] !== key);
      return this;
    }
    setQueryParameter(name, value) {
      return this.removeAllQueryParameters(name).addQueryParameter(name, value);
    }
    setEncodedQueryParameter(name, value) {
      this.query = this.query.filter((pair) => pair.split('=')[0] !== KT.str(name));
      return this.addEncodedQueryParameter(name, value);
    }
    query(text) {
      this.query = text ? KT.str(text).split('&') : [];
      return this;
    }
    fragment(value) {
      this.fragment_ = value;
      return this;
    }
    build() {
      const port =
        this.port_ &&
        !(
          (this.scheme_ === 'https' && this.port_ === 443) ||
          (this.scheme_ === 'http' && this.port_ === 80)
        )
          ? `:${this.port_}`
          : '';
      const path = `/${this.segments.join('/')}`;
      const query = this.query.length ? `?${this.query.join('&')}` : '';
      const fragment = this.fragment_ ? `#${this.fragment_}` : '';
      return HttpUrl.parse(`${this.scheme_}://${this.host_}${port}${path}${query}${fragment}`);
    }
    toString() {
      return this.build().toString();
    }
  }

  class Headers {
    constructor(pairs = []) {
      this.pairs = pairs;
    }
    get(name) {
      const found = [...this.pairs]
        .reverse()
        .find(([key]) => key.toLowerCase() === KT.str(name).toLowerCase());
      return found ? found[1] : null;
    }
    values(name) {
      return this.pairs
        .filter(([key]) => key.toLowerCase() === KT.str(name).toLowerCase())
        .map(([, value]) => value);
    }
    newBuilder() {
      return new HeadersBuilder([...this.pairs]);
    }
    get size() {
      return this.pairs.length;
    }
    toMultimap() {
      return Object.fromEntries(this.pairs.map(([key, value]) => [key, [value]]));
    }
    object() {
      const object = {};
      for (const [key, value] of this.pairs) object[key] = value;
      return object;
    }
  }
  class HeadersBuilder {
    constructor(pairs = []) {
      this.pairs = pairs;
    }
    add(name, value) {
      if (value === undefined && KT.str(name).includes(':')) {
        const [key, ...rest] = KT.str(name).split(':');
        this.pairs.push([key.trim(), rest.join(':').trim()]);
      } else {
        this.pairs.push([KT.str(name), KT.str(value)]);
      }
      return this;
    }
    set(name, value) {
      this.removeAll(name);
      this.pairs.push([KT.str(name), KT.str(value)]);
      return this;
    }
    removeAll(name) {
      this.pairs = this.pairs.filter(([key]) => key.toLowerCase() !== KT.str(name).toLowerCase());
      return this;
    }
    get(name) {
      return new Headers(this.pairs).get(name);
    }
    build() {
      return new Headers([...this.pairs]);
    }
  }

  const FETCH_KINDS = ['DOCUMENT', 'XHR', 'SCRIPT', 'IMAGE', 'NONE'];
  const Fetch = {};
  for (const kind of FETCH_KINDS) {
    Fetch[kind] = {
      isFetch: true,
      kind: kind.toLowerCase(),
      from: (page) => ({ isFetch: true, kind: kind.toLowerCase(), page: KT.str(page) }),
    };
  }

  class Request {
    constructor({ url, method = 'GET', headers = new Headers(), body = null, kind = null }) {
      this.url = url instanceof HttpUrl ? url : HttpUrl.parse(url);
      this.method = method;
      this.headers = headers;
      this.body = body;
      this.kind = kind;
    }
    header(name) {
      return this.headers.get(name);
    }
    newBuilder() {
      const builder = new RequestBuilder();
      builder.url_ = this.url;
      builder.method_ = this.method;
      builder.headers_ = this.headers.newBuilder();
      builder.body_ = this.body;
      builder.kind_ = this.kind;
      return builder;
    }
  }
  class RequestBuilder {
    constructor() {
      this.url_ = null;
      this.method_ = 'GET';
      this.headers_ = new HeadersBuilder();
      this.body_ = null;
      this.kind_ = null;
    }
    url(url) {
      this.url_ = url instanceof HttpUrl ? url : HttpUrl.parse(url);
      return this;
    }
    header(name, value) {
      this.headers_.set(name, value);
      return this;
    }
    addHeader(name, value) {
      this.headers_.add(name, value);
      return this;
    }
    removeHeader(name) {
      this.headers_.removeAll(name);
      return this;
    }
    headers(headers) {
      this.headers_ = headers.newBuilder();
      return this;
    }
    get() {
      this.method_ = 'GET';
      return this;
    }
    head() {
      this.method_ = 'HEAD';
      return this;
    }
    post(body) {
      this.method_ = 'POST';
      this.body_ = body;
      return this;
    }
    put(body) {
      this.method_ = 'PUT';
      this.body_ = body;
      return this;
    }
    delete(body = null) {
      this.method_ = 'DELETE';
      this.body_ = body;
      return this;
    }
    method(method, body) {
      this.method_ = method;
      this.body_ = body ?? null;
      return this;
    }
    cacheControl() {
      return this;
    }
    tag() {
      return this;
    }
    fetch(fetch) {
      this.kind_ = fetch;
      return this;
    }
    build() {
      return new Request({
        url: this.url_,
        method: this.method_,
        headers: this.headers_.build(),
        body: this.body_,
        kind: this.kind_,
      });
    }
  }
  const formBody = () => {
    const pairs = [];
    const builder = {
      add: (name, value) => {
        pairs.push([KT.str(name), KT.str(value)]);
        return builder;
      },
      addEncoded: (name, value) => {
        pairs.push([decodeURIComponent(KT.str(name)), decodeURIComponent(KT.str(value))]);
        return builder;
      },
      build: () => ({
        isBody: true,
        text: K.form(pairs),
        type: 'application/x-www-form-urlencoded',
      }),
    };
    return builder;
  };

  class Response {
    constructor(raw, request) {
      this.raw = raw;
      this.code = raw.code;
      this.request = {
        url: HttpUrl.parse(raw.url || request.url.toString()),
        method: request.method,
        headers: request.headers,
      };
      this.headers = new Headers(Object.entries(raw.headers || {}));
      const text = () => raw.text();
      this.body = {
        string: text,
        text,
        close: () => {},
        contentType: () => raw.header('content-type'),
      };
    }
    /// An answer an interceptor made (`response.newBuilder()…build()`).
    static made(state) {
      const body = state.body ?? textBody('');
      const response = Object.create(Response.prototype);
      response.code = state.code;
      response.message = state.message;
      response.request = state.request;
      response.headers = state.headers;
      response.body = body;
      response.raw = {
        text: () => body.string(),
        header: (name) => state.headers.get(name),
        code: state.code,
      };
      return response;
    }
    get isSuccessful() {
      return this.code >= 200 && this.code < 300;
    }
    header(name, fallback = null) {
      return this.headers.get(name) ?? fallback;
    }
    newBuilder() {
      return responseBuilder(this);
    }
    peekBody() {
      return this.body;
    }
    async asJsoup() {
      return Jsoup.parse(await this.raw.text(), this.request.url.toString());
    }
    /// Plain JSON; `KT.m` decodes it as the type the call asks for.
    async parseAs() {
      return JSON.parse(await this.raw.text());
    }
    /// Keiyoushi's `parseGraphQLAs<T>()`: the answer's `data`, or its errors as a failure.
    async parseGraphQLAs() {
      const answer = JSON.parse(await this.raw.text());
      if (Array.isArray(answer.errors) && answer.errors.length && answer.data == null) {
        throw new KotlinError(
          answer.errors.map((error) => KT.str(error?.message ?? '')).join('\n'),
          'GraphQLException',
        );
      }
      return answer.data ?? null;
    }
    async use(lambda) {
      return lambda(this);
    }
    close() {}
  }

  /// The classes the source declares, by name (each extension has its own runtime).
  const CLASSES = {};
  /// A type as written (`List<ChapterDto>`, `Map<String,Foo>?`): its name and arguments.
  const parseType = (text) => {
    let at = 0;
    const read = () => {
      let name = '';
      while (at < text.length && !'<>,'.includes(text[at])) name += text[at++];
      const args = [];
      if (text[at] === '<') {
        at++;
        for (;;) {
          args.push(read());
          if (text[at] === ',') {
            at++;
            continue;
          }
          at++;
          break;
        }
      }
      return { name: name.trim().replace(/\?$/, '').split('.').pop(), args };
    };
    return read();
  };
  const LISTS = new Set([
    'List',
    'MutableList',
    'Array',
    'Set',
    'MutableSet',
    'Collection',
    'ArrayList',
    'Iterable',
  ]);
  const decodeAs = async (value, type) => {
    if (value === null || value === undefined) return null;
    if (LISTS.has(type.name)) {
      if (!Array.isArray(value)) return value;
      const out = [];
      for (const item of value)
        out.push(await decodeAs(item, type.args[0] ?? { name: 'Any', args: [] }));
      return out;
    }
    if (type.name === 'Map' || type.name === 'HashMap' || type.name === 'MutableMap') {
      if (!value || typeof value !== 'object') return value;
      const out = {};
      for (const [key, item] of Object.entries(value))
        out[key] = await decodeAs(item, type.args[1] ?? { name: 'Any', args: [] });
      return out;
    }
    const holder = CLASSES[type.name];
    if (holder && value && typeof value === 'object' && !Array.isArray(value))
      return holder.fromJson(value, type.args);
    return value;
  };
  /// A field's type with the class's type parameters put in (`T` of `ApiResponse<PageData>`).
  const substitute = (type, names, args) => {
    const at = names.indexOf(type.name);
    if (at >= 0 && args[at]) return args[at];
    return { name: type.name, args: type.args.map((arg) => substitute(arg, names, args)) };
  };
  /// A JSON answer as the type asked for: declared DTOs with their methods, the rest plain, names
  /// read through `@SerialName`.
  const decode = async (value, type) => (type ? decodeAs(value, parseType(type)) : value);
  const DECODERS = new Set([
    'parseAs',
    'decodeFromString',
    'decodeFromJsonElement',
    'parseGraphQLAs',
  ]);
  KT.unconverted = (name, reason) => ({
    isClass: true,
    name,
    make: async () => KT.raise(`Raygun couldn't convert ${name}: ${reason}`),
    fromJson: async () => KT.raise(`Raygun couldn't convert ${name}: ${reason}`),
  });

  /// The source's client: requests over the source's `Http` (its Referer, rate limit, cookies).
  class Client {
    constructor(source) {
      this.source = source;
      this.interceptors = [];
    }
    /// A request through the source's interceptors (`addInterceptor`), in the order added, then
    /// the network.
    async send(request, ensureSuccess) {
      const run = async (index, current) => {
        if (index >= this.interceptors.length) return this.transport(current);
        const interceptor = this.interceptors[index];
        const chain = {
          request: () => current,
          proceed: (next) => run(index + 1, next ?? current),
          call: () => ({ request: () => current, isCanceled: () => false }),
          connectTimeoutMillis: () => 30000,
          readTimeoutMillis: () => 30000,
          writeTimeoutMillis: () => 30000,
        };
        return typeof interceptor === 'function'
          ? interceptor(chain)
          : KT.m(interceptor, 'intercept', [chain]);
      };
      const response = await run(0, request);
      if (ensureSuccess && !response.isSuccessful)
        throw new HttpError(response.code, response.request.url.toString());
      return response;
    }
    async transport(request) {
      const fetch = request.kind;
      const kind = fetch && fetch.kind ? fetch.kind : request.method === 'GET' ? 'document' : 'xhr';
      const headers = request.headers.object();
      let body = null;
      if (request.body) {
        body = request.body.text;
        if (request.body.type) headers['Content-Type'] = request.body.type;
      }
      const raw = await this.source.http.request(request.url.toString(), {
        method: request.method,
        headers,
        body,
        kind: kind === 'none' ? null : kind,
        from: fetch && fetch.page ? fetch.page : null,
        ensureSuccess: false,
      });
      return new Response(raw, request);
    }
    newCall(request) {
      return {
        execute: () => this.send(request, false),
        await: () => this.send(request, false),
        awaitSuccess: () => this.send(request, true),
        enqueue: () => {},
      };
    }
    /// `client.get(url, [headers], fetch, [cacheControl], [ensureSuccess])` (Keiyoushi's).
    get(url, ...rest) {
      return this.request('GET', url, rest);
    }
    head(url, ...rest) {
      return this.request('HEAD', url, rest);
    }
    post(url, ...rest) {
      return this.request('POST', url, rest);
    }
    async request(method, url, rest) {
      const options = named(rest);
      // The source's own headers (`headersBuilder()`, its `configureHeaders`) unless given.
      let headers =
        options.headers ?? (this.defaultHeaders ? await this.defaultHeaders() : new Headers());
      let fetch = options.fetch ?? null;
      let body = options.body ?? null;
      let ensureSuccess = options.ensureSuccess ?? true;
      for (const arg of rest) {
        if (arg instanceof Headers) headers = arg;
        else if (arg && arg.isFetch) fetch = arg;
        else if (arg && arg.isBody) body = arg;
        else if (typeof arg === 'boolean') ensureSuccess = arg;
      }
      return this.send(new Request({ url, method, headers, body, kind: fetch }), ensureSuccess);
    }
    newBuilder() {
      return clientBuilder(this);
    }
  }
  const clientBuilder = (client) => {
    const builder = {
      rateLimit: (permits, period = 1, unit = null) => {
        const unitMs = unit && unit.unitMs ? unit.unitMs : 1000;
        const periodMs =
          typeof period === 'number' ? period * unitMs : period && period.ms ? period.ms : 1000;
        client.source.http.limit = { permits, periodMs };
        return builder;
      },
      rateLimitHost: (_host, permits, period = 1, unit = null) =>
        builder.rateLimit(permits, period, unit),
      connectTimeout: () => builder,
      readTimeout: () => builder,
      writeTimeout: () => builder,
      callTimeout: () => builder,
      followRedirects: () => builder,
      followSslRedirects: () => builder,
      retryOnConnectionFailure: () => builder,
      // Interceptors over requests and answers (headers, tokens, retries, cookies); one decoding
      // images can't convert, as the runtime has no pictures.
      addInterceptor: (interceptor) => {
        client.interceptors.push(interceptor);
        return builder;
      },
      addNetworkInterceptor: (interceptor) => {
        client.interceptors.push(interceptor);
        return builder;
      },
      interceptors: () => client.interceptors,
      // The extension's own cookie jar always keeps cookies.
      cookieJar: () => builder,
      /// Keiyoushi's `addCookie(…)`: cookies sent to the source's host (or one named first).
      addCookie: (...args) => {
        const domain = args.length >= 2 && typeof args[0] === 'function' ? args.shift() : null;
        const cookies = args[0];
        client.interceptors.push(async (chain) => {
          const request = chain.request();
          const host = domain ? KT.str(await domain()) : HttpUrl.parse(client.source.baseUrl).host;
          const target = request.url.host;
          if (target !== host && !target.endsWith(`.${host}`)) return chain.proceed(request);
          let list = typeof cookies === 'function' ? await cookies() : cookies;
          if (list && !Array.isArray(list)) list = [list];
          const pairs = (list ?? []).map((pair) => `${KT.str(pair.first)}=${KT.str(pair.second)}`);
          if (!pairs.length) return chain.proceed(request);
          const value = [request.headers.get('Cookie'), ...pairs].filter(Boolean).join('; ');
          return chain.proceed(request.newBuilder().header('Cookie', value).build());
        });
        return builder;
      },
      build: () => client,
    };
    return builder;
  };
  /// `response.newBuilder()…build()`: an answer an interceptor changes.
  const responseBuilder = (from) => {
    const state = {
      code: from.code,
      headers: from.headers,
      body: from.body,
      request: from.request,
      message: from.message ?? '',
    };
    const builder = {
      code: (code) => ((state.code = code), builder),
      message: (message) => ((state.message = message), builder),
      header: (name, value) => (
        (state.headers = state.headers.newBuilder().set(name, value).build()),
        builder
      ),
      addHeader: (name, value) => (
        (state.headers = state.headers.newBuilder().add(name, value).build()),
        builder
      ),
      removeHeader: (name) => (
        (state.headers = state.headers.newBuilder().removeAll(name).build()),
        builder
      ),
      headers: (headers) => ((state.headers = headers), builder),
      body: (body) => ((state.body = body), builder),
      request: (request) => ((state.request = request), builder),
      protocol: () => builder,
      build: () => Response.made(state),
    };
    return builder;
  };
  /// A body made in code (`"…".toResponseBody(mediaType)`).
  const textBody = (text, type = null) => {
    const value = KT.str(text);
    return {
      isResponseBody: true,
      string: async () => value,
      text: async () => value,
      bytes: async () => value,
      close: () => {},
      contentType: () => type,
      contentLength: () => value.length,
    };
  };
  KT.textBody = textBody;

  // ---- Mihon's models ----

  const manga = (fields = {}) => {
    const object = {
      __class: 'SManga',
      __classes: ['SManga'],
      url: '',
      title: '',
      artist: null,
      author: null,
      description: null,
      genre: null,
      status: 0,
      thumbnail_url: null,
      update_strategy: 0,
      initialized: false,
      memo: {},
      ...fields,
    };
    object.setUrlWithoutDomain = (url) => {
      object.url = K.withoutDomain(K.str(url));
    };
    object.getGenres = () =>
      object.genre
        ? object.genre
            .split(', ')
            .map((genre) => genre.trim())
            .filter(Boolean)
        : null;
    return object;
  };
  const chapter = (fields = {}) => {
    const object = {
      __class: 'SChapter',
      __classes: ['SChapter'],
      url: '',
      name: '',
      date_upload: 0,
      chapter_number: -1,
      scanlator: null,
      memo: {},
      ...fields,
    };
    object.setUrlWithoutDomain = (url) => {
      object.url = K.withoutDomain(K.str(url));
    };
    return object;
  };
  const exception = (kind) => (message) => new KotlinError(message ?? kind, kind);
  const durations = {};
  for (const [unit, ms] of [
    ['milliseconds', 1],
    ['seconds', 1000],
    ['minutes', 60000],
    ['hours', 3600000],
    ['days', 86400000],
  ])
    durations[unit] = ms;

  const GLOBALS = {
    listOf: (args) => [...args],
    mutableListOf: (args) => [...args],
    arrayListOf: (args) => [...args],
    arrayOf: (args) => [...args],
    listOfNotNull: (args) => args.filter((value) => value != null),
    emptyList: () => [],
    emptyArray: () => [],
    setOf: (args) => LIST.distinct(args),
    mutableSetOf: (args) => LIST.distinct(args),
    hashSetOf: (args) => LIST.distinct(args),
    linkedSetOf: (args) => LIST.distinct(args),
    setOfNotNull: (args) => LIST.distinct(args.filter((value) => value != null)),
    emptySet: () => [],
    mapOf: (args) => new Map(args.map((pair) => [pair.first, pair.second])),
    mutableMapOf: (args) => new Map(args.map((pair) => [pair.first, pair.second])),
    hashMapOf: (args) => new Map(args.map((pair) => [pair.first, pair.second])),
    linkedMapOf: (args) => new Map(args.map((pair) => [pair.first, pair.second])),
    emptyMap: () => new Map(),
    Pair: ([first, second]) => KT.pair(first, second),
    Triple: ([first, second, third]) => ({ first, second, third, isPair: true }),
    error: ([message]) => KT.raise(message),
    require: ([condition]) => (KT.truthy(condition) ? KT.unit : KT.raise('requirement failed')),
    requireNotNull: ([value]) => KT.nn(value),
    check: ([condition]) => (KT.truthy(condition) ? KT.unit : KT.raise('check failed')),
    checkNotNull: ([value]) => KT.nn(value),
    TODO: () => KT.raise('not implemented'),
    println: () => KT.unit,
    print: () => KT.unit,
    maxOf: (args) => Math.max(...args),
    minOf: (args) => Math.min(...args),
    Regex: ([pattern, option]) => K.regex(pattern, option),
    Exception: ([message]) => new KotlinError(message, 'Exception'),
    RuntimeException: ([message]) => new KotlinError(message, 'RuntimeException'),
    IllegalStateException: ([message]) => new KotlinError(message, 'IllegalStateException'),
    IllegalArgumentException: ([message]) => new KotlinError(message, 'IllegalArgumentException'),
    UnsupportedOperationException: ([message]) =>
      new KotlinError(message ?? 'This source can’t do that', 'UnsupportedOperationException'),
    NoSuchElementException: ([message]) => new KotlinError(message, 'NoSuchElementException'),
    IOException: ([message]) => new KotlinError(message, 'IOException'),
    Error: ([message]) => new KotlinError(message, 'Error'),
    HttpException: ([code]) => new KotlinError(`HTTP error ${code}`, 'HttpException'),
    GET: ([url, headers]) =>
      new Request({ url, headers: headers instanceof Headers ? headers : undefined }),
    POST: ([url, headers, body]) =>
      new Request({
        url,
        method: 'POST',
        headers: headers instanceof Headers ? headers : undefined,
        body: body ?? null,
      }),
    PUT: ([url, headers, body]) =>
      new Request({
        url,
        method: 'PUT',
        headers: headers instanceof Headers ? headers : undefined,
        body: body ?? null,
      }),
    DELETE: ([url, headers, body]) =>
      new Request({
        url,
        method: 'DELETE',
        headers: headers instanceof Headers ? headers : undefined,
        body: body ?? null,
      }),
    MangasPage: ([mangas, hasNextPage], named_) => ({
      __class: 'MangasPage',
      mangas: named_?.mangas ?? mangas,
      hasNextPage: named_?.hasNextPage ?? hasNextPage ?? false,
    }),
    Page: ([index, url, imageUrl], named_) => ({
      __class: 'Page',
      index: named_?.index ?? index ?? 0,
      url: named_?.url ?? url ?? '',
      imageUrl: named_?.imageUrl ?? imageUrl ?? null,
    }),
    SMangaUpdate: ([update, chapters], named_) => ({
      __class: 'SMangaUpdate',
      manga: named_?.manga ?? update,
      chapters: named_?.chapters ?? chapters ?? [],
    }),
    FilterList: (args) => (args.length === 1 && Array.isArray(args[0]) ? [...args[0]] : [...args]),
    SimpleDateFormat: ([pattern, locale]) => K.dateFormat(pattern, locale),
    StringBuilder: ([text = '']) => KT.stringBuilder().append(text),
    // A date is its Unix milliseconds, as a date format's `parse` gives.
    Date: ([ms]) => (ms === undefined ? Date.now() : msOf(ms)),
    JsonPrimitive: ([value]) => value,
    JsonObject: ([map]) => plain(map),
    JsonArray: ([list]) => plain(list),
    Locale: ([language, country]) => K.locale(language, country),
    abs: ([value]) => Math.abs(value),
    round: ([value]) => Math.round(value),
    floor: ([value]) => Math.floor(value),
    ceil: ([value]) => Math.ceil(value),
    // Settings screens aren't shown: preferences read their defaults.
    getPreferences: () => sharedPreferences,
    getPreferencesLazy: () => KT.deferred(async () => sharedPreferences),
    Selection: ([index, ascending], named_) => ({
      index: named_?.index ?? index ?? 0,
      ascending: named_?.ascending ?? ascending ?? false,
    }),
    // `Json { ignoreUnknownKeys = true }`: reading always ignores what it doesn't know.
    Json: () => jsonCodec,
    Mutex: () => KT.mutex(),
    Semaphore: ([permits]) => KT.semaphore(permits ?? 1),
    awaitAll: (args) => awaitEach(args),
    Char: ([code]) => String.fromCharCode(code),
    // Keiyoushi's GraphQL helpers (`keiyoushi.utils`).
    GraphQLErrorInterceptor: () => graphQLErrors,
    graphQLBody: ([query, operationName, variables, extensions], named_) => ({
      isBody: true,
      text: JSON.stringify(
        plain(
          Object.fromEntries(
            Object.entries({
              operationName: named_?.operationName ?? operationName,
              query: named_?.query ?? query,
              variables: named_?.variables ?? variables,
              extensions: named_?.extensions ?? extensions,
            }).filter(([, value]) => value !== undefined && value !== null),
          ),
        ),
      ),
      type: 'application/json; charset=utf-8',
    }),
    persistedQueryExtension: ([hash, version = 1]) => ({
      persistedQuery: { version, sha256Hash: hash },
    }),
    // `Interceptor { chain -> … }`: the function itself.
    Interceptor: (args, _named, lambda) => lambda ?? args[0],
    Comparator: (args, _named, lambda) => lambda ?? args[0],
    compareBy: (args, _named, lambda) =>
      comparator(
        (lambda ? [...args, lambda] : args).map((select) => ({ select, descending: false })),
      ),
    compareByDescending: (args, _named, lambda) =>
      comparator(
        (lambda ? [...args, lambda] : args).map((select) => ({ select, descending: true })),
      ),
    naturalOrder: () => comparator([{ select: null, descending: false }]),
    reverseOrder: () => comparator([{ select: null, descending: true }]),
    List: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init),
    MutableList: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init),
    Array: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init),
    arrayOfNulls: ([size]) => new Array(size).fill(null),
    HashMap: ([from]) => new Map(from instanceof Map ? from : Object.entries(from ?? {})),
    LinkedHashMap: ([from]) => new Map(from instanceof Map ? from : Object.entries(from ?? {})),
    ConcurrentHashMap: ([from]) => new Map(from instanceof Map ? from : Object.entries(from ?? {})),
    HashSet: ([from]) => LIST.distinct([...(from ?? [])]),
    LinkedHashSet: ([from]) => LIST.distinct([...(from ?? [])]),
    DateTimeFormatterBuilder: () => dateFormatterBuilder(),
    Intl: ([language, availableLanguages, baseLanguage], named_) =>
      intl(
        named_?.language ?? language,
        named_?.availableLanguages ?? availableLanguages,
        named_?.baseLanguage ?? baseLanguage ?? 'en',
      ),
    // `String(chars)`, `String(bytes)` (one byte a character).
    String: ([value]) =>
      Array.isArray(value)
        ? value
            .map((item) => (typeof item === 'number' ? String.fromCharCode(item) : KT.str(item)))
            .join('')
        : KT.str(value),
  };
  const instant = (ms) => moment(ms, 'Instant');
  const jsonCodec = {
    decodeFromString: (text) => JSON.parse(KT.str(text)),
    parseToJsonElement: (text) => JSON.parse(KT.str(text)),
    encodeToString: (value) => JSON.stringify(plain(value)),
    decodeFromJsonElement: (value) => value,
    encodeToJsonElement: (value) => plain(value),
  };
  /// `awaitAll()`: deferred values (`async { … }`), worked out together.
  const awaitEach = (items) =>
    Promise.all(items.map((item) => (item && item.isDeferred ? item.await() : item)));
  /// A zone (`TimeZone`, `ZoneId`, `ZoneOffset`): its offset from UTC in minutes.
  const zone = (offset) => ({
    offset,
    isZone: true,
    getID: () => (offset ? `GMT${offset > 0 ? '+' : '-'}${Math.abs(offset / 60)}` : 'UTC'),
  });
  const offsetOf = (value) => (value && typeof value.offset === 'number' ? value.offset : 0);
  const msOf = (value) =>
    typeof value === 'number'
      ? value
      : value && typeof value.timeInMillis === 'number'
        ? value.timeInMillis
        : Number(value);

  // java.util.Calendar: fields read and moved on the wall clock of its zone (the device's unless
  // one is given).
  const CALENDAR = {
    ERA: 0,
    YEAR: 1,
    MONTH: 2,
    WEEK_OF_YEAR: 3,
    WEEK_OF_MONTH: 4,
    DATE: 5,
    DAY_OF_MONTH: 5,
    DAY_OF_YEAR: 6,
    DAY_OF_WEEK: 7,
    AM_PM: 9,
    HOUR: 10,
    HOUR_OF_DAY: 11,
    MINUTE: 12,
    SECOND: 13,
    MILLISECOND: 14,
    SUNDAY: 1,
    MONDAY: 2,
    TUESDAY: 3,
    WEDNESDAY: 4,
    THURSDAY: 5,
    FRIDAY: 6,
    SATURDAY: 7,
    JANUARY: 0,
    FEBRUARY: 1,
    MARCH: 2,
    APRIL: 3,
    MAY: 4,
    JUNE: 5,
    JULY: 6,
    AUGUST: 7,
    SEPTEMBER: 8,
    OCTOBER: 9,
    NOVEMBER: 10,
    DECEMBER: 11,
    AM: 0,
    PM: 1,
  };
  const calendar = (offset = -new Date().getTimezoneOffset()) => {
    let ms = Date.now();
    const wall = () => new Date(ms + offset * 60000);
    const setWall = (date) => {
      ms = date.getTime() - offset * 60000;
    };
    const value = {
      __class: 'Calendar',
      __classes: ['Calendar'],
      add: (field, amount) => {
        const date = wall();
        switch (field) {
          case CALENDAR.YEAR:
            date.setUTCFullYear(date.getUTCFullYear() + amount);
            break;
          case CALENDAR.MONTH:
            date.setUTCMonth(date.getUTCMonth() + amount);
            break;
          case CALENDAR.WEEK_OF_YEAR:
          case CALENDAR.WEEK_OF_MONTH:
            date.setUTCDate(date.getUTCDate() + 7 * amount);
            break;
          case CALENDAR.DATE:
          case CALENDAR.DAY_OF_YEAR:
          case CALENDAR.DAY_OF_WEEK:
            date.setUTCDate(date.getUTCDate() + amount);
            break;
          case CALENDAR.HOUR:
          case CALENDAR.HOUR_OF_DAY:
            date.setUTCHours(date.getUTCHours() + amount);
            break;
          case CALENDAR.MINUTE:
            date.setUTCMinutes(date.getUTCMinutes() + amount);
            break;
          case CALENDAR.SECOND:
            date.setUTCSeconds(date.getUTCSeconds() + amount);
            break;
          case CALENDAR.MILLISECOND:
            date.setUTCMilliseconds(date.getUTCMilliseconds() + amount);
            break;
          default:
            break;
        }
        setWall(date);
      },
      set: (...args) => {
        const date = wall();
        if (args.length >= 3) {
          const [year, month, day, hour, minute, second] = args;
          date.setUTCFullYear(year, month, day);
          if (hour !== undefined) date.setUTCHours(hour, minute ?? 0, second ?? 0);
        } else {
          const [field, amount] = args;
          switch (field) {
            case CALENDAR.YEAR:
              date.setUTCFullYear(amount);
              break;
            case CALENDAR.MONTH:
              date.setUTCMonth(amount);
              break;
            case CALENDAR.DATE:
              date.setUTCDate(amount);
              break;
            case CALENDAR.DAY_OF_YEAR:
              date.setUTCMonth(0, amount);
              break;
            case CALENDAR.DAY_OF_WEEK:
              date.setUTCDate(date.getUTCDate() + amount - 1 - date.getUTCDay());
              break;
            case CALENDAR.HOUR_OF_DAY:
              date.setUTCHours(amount);
              break;
            case CALENDAR.HOUR:
              date.setUTCHours((date.getUTCHours() >= 12 ? 12 : 0) + amount);
              break;
            case CALENDAR.AM_PM:
              date.setUTCHours((date.getUTCHours() % 12) + (amount ? 12 : 0));
              break;
            case CALENDAR.MINUTE:
              date.setUTCMinutes(amount);
              break;
            case CALENDAR.SECOND:
              date.setUTCSeconds(amount);
              break;
            case CALENDAR.MILLISECOND:
              date.setUTCMilliseconds(amount);
              break;
            default:
              break;
          }
        }
        setWall(date);
      },
      get: (field) => {
        const date = wall();
        switch (field) {
          case CALENDAR.YEAR:
            return date.getUTCFullYear();
          case CALENDAR.MONTH:
            return date.getUTCMonth();
          case CALENDAR.DATE:
            return date.getUTCDate();
          case CALENDAR.DAY_OF_YEAR:
            return (
              Math.floor((date.getTime() - Date.UTC(date.getUTCFullYear(), 0, 1)) / 86400000) + 1
            );
          case CALENDAR.DAY_OF_WEEK:
            return date.getUTCDay() + 1;
          case CALENDAR.HOUR_OF_DAY:
            return date.getUTCHours();
          case CALENDAR.HOUR:
            return date.getUTCHours() % 12;
          case CALENDAR.AM_PM:
            return date.getUTCHours() >= 12 ? 1 : 0;
          case CALENDAR.MINUTE:
            return date.getUTCMinutes();
          case CALENDAR.SECOND:
            return date.getUTCSeconds();
          case CALENDAR.MILLISECOND:
            return date.getUTCMilliseconds();
          default:
            return 0;
        }
      },
      clone: () => {
        const copy = calendar(offset);
        copy.timeInMillis = ms;
        return copy;
      },
      before: (other) => ms < msOf(other),
      after: (other) => ms > msOf(other),
      compareTo: (other) => ms - msOf(other),
      getTimeInMillis: () => ms,
      setTimeInMillis: (next) => {
        ms = msOf(next);
      },
      getTime: () => ms,
      setTime: (next) => {
        ms = msOf(next);
      },
    };
    // Kotlin's `timeInMillis`, `time` and `timeZone` read and set these.
    Object.defineProperty(value, 'timeInMillis', {
      get: () => ms,
      set: (next) => {
        ms = msOf(next);
      },
      enumerable: false,
    });
    Object.defineProperty(value, 'time', {
      get: () => ms,
      set: (next) => {
        ms = msOf(next);
      },
      enumerable: false,
    });
    Object.defineProperty(value, 'timeZone', {
      get: () => zone(offset),
      set: (next) => {
        offset = offsetOf(next);
      },
      enumerable: false,
    });
    return value;
  };

  // java.time values as Unix milliseconds: a `LocalDate` at the UTC midnight of its day, a
  // `LocalDateTime` as its wall clock read in UTC, a `ZonedDateTime` as the instant it is.
  const DAY_MS = 86400000;
  const localOffset = () => -new Date().getTimezoneOffset();
  const moment = (ms, kind) => {
    const shift = (move) => {
      const date = new Date(ms);
      move(date);
      return moment(date.getTime(), kind);
    };
    const local = kind === 'LocalDate' || kind === 'LocalDateTime';
    const value = {
      __class: kind,
      __classes: [kind],
      ms,
      isInstant: true,
      minusDays: (n) => shift((date) => date.setUTCDate(date.getUTCDate() - n)),
      plusDays: (n) => shift((date) => date.setUTCDate(date.getUTCDate() + n)),
      minusWeeks: (n) => shift((date) => date.setUTCDate(date.getUTCDate() - 7 * n)),
      plusWeeks: (n) => shift((date) => date.setUTCDate(date.getUTCDate() + 7 * n)),
      minusMonths: (n) => shift((date) => date.setUTCMonth(date.getUTCMonth() - n)),
      plusMonths: (n) => shift((date) => date.setUTCMonth(date.getUTCMonth() + n)),
      minusYears: (n) => shift((date) => date.setUTCFullYear(date.getUTCFullYear() - n)),
      plusYears: (n) => shift((date) => date.setUTCFullYear(date.getUTCFullYear() + n)),
      minusHours: (n) => shift((date) => date.setUTCHours(date.getUTCHours() - n)),
      plusHours: (n) => shift((date) => date.setUTCHours(date.getUTCHours() + n)),
      minusMinutes: (n) => shift((date) => date.setUTCMinutes(date.getUTCMinutes() - n)),
      plusMinutes: (n) => shift((date) => date.setUTCMinutes(date.getUTCMinutes() + n)),
      minusSeconds: (n) => shift((date) => date.setUTCSeconds(date.getUTCSeconds() - n)),
      plusSeconds: (n) => shift((date) => date.setUTCSeconds(date.getUTCSeconds() + n)),
      // A local value placed in a zone (the device's when none is given).
      atStartOfDay: (where) =>
        moment(
          Math.floor(ms / DAY_MS) * DAY_MS -
            (where === undefined ? localOffset() : offsetOf(where)) * 60000,
          'ZonedDateTime',
        ),
      atZone: (where) => moment(ms - offsetOf(where) * 60000, 'ZonedDateTime'),
      toInstant: (where) =>
        instant(local && where !== undefined ? ms - offsetOf(where) * 60000 : ms),
      toLocalDate: () => moment(Math.floor(ms / DAY_MS) * DAY_MS, 'LocalDate'),
      toEpochMilli: () => ms,
      toEpochMilliseconds: () => ms,
      epochSeconds: Math.floor(ms / 1000),
      toEpochSecond: (where) =>
        Math.floor((local && where !== undefined ? ms - offsetOf(where) * 60000 : ms) / 1000),
      toEpochDay: () => Math.floor(ms / DAY_MS),
      truncatedTo: (unit) => {
        const size =
          { MILLIS: 1, SECONDS: 1000, MINUTES: 60000, HOURS: 3600000, DAYS: DAY_MS }[
            KT.str(unit)
          ] ?? 1;
        return moment(Math.floor(ms / size) * size, kind);
      },
      minus: (amount, unit) =>
        shift((date) =>
          date.setTime(
            date.getTime() -
              (amount && amount.isDuration ? amount.ms : amount) *
                (amount && amount.isDuration
                  ? 1
                  : ({
                      MILLIS: 1,
                      SECONDS: 1000,
                      MINUTES: 60000,
                      HOURS: 3600000,
                      DAYS: DAY_MS,
                      WEEKS: 7 * DAY_MS,
                    }[KT.str(unit)] ?? 1)),
          ),
        ),
      plus: (amount, unit) =>
        shift((date) =>
          date.setTime(
            date.getTime() +
              (amount && amount.isDuration ? amount.ms : amount) *
                (amount && amount.isDuration
                  ? 1
                  : ({
                      MILLIS: 1,
                      SECONDS: 1000,
                      MINUTES: 60000,
                      HOURS: 3600000,
                      DAYS: DAY_MS,
                      WEEKS: 7 * DAY_MS,
                    }[KT.str(unit)] ?? 1)),
          ),
        ),
      getYear: () => new Date(ms).getUTCFullYear(),
      getMonthValue: () => new Date(ms).getUTCMonth() + 1,
      getDayOfMonth: () => new Date(ms).getUTCDate(),
      format: (formatter) => formatter.format(value),
      isBefore: (other) => ms < msOf(other.ms ?? other),
      isAfter: (other) => ms > msOf(other.ms ?? other),
      toString: () =>
        kind === 'Instant'
          ? new Date(ms).toISOString()
          : new Date(ms).toISOString().slice(0, kind === 'LocalDate' ? 10 : 19),
    };
    return value;
  };
  /// A wall clock read from text with a date format (or ISO 8601 without one), in UTC.
  const wallClock = (text, format, kind) => {
    let ms;
    if (format && typeof format.parse === 'function') {
      const read = format.parse(text);
      ms = read === null ? null : read + format.offsetMinutes * 60000;
    } else {
      const iso = KT.str(text).trim();
      ms = Date.parse(
        kind === 'LocalDate'
          ? `${iso.slice(0, 10)}T00:00:00Z`
          : /[zZ]|[+-]\d{2}:?\d{2}$/.test(iso)
            ? iso
            : `${iso}Z`,
      );
    }
    if (ms === null || Number.isNaN(ms))
      throw new KotlinError(`Text '${KT.str(text)}' could not be parsed`, 'DateTimeParseException');
    return kind === 'LocalDate' ? Math.floor(ms / DAY_MS) * DAY_MS : ms;
  };
  const ENTITIES = {
    amp: '&',
    lt: '<',
    gt: '>',
    quot: '"',
    apos: "'",
    nbsp: ' ',
    hellip: '…',
    mdash: '—',
    ndash: '–',
    lsquo: '‘',
    rsquo: '’',
    ldquo: '“',
    rdquo: '”',
    laquo: '«',
    raquo: '»',
    copy: '©',
    reg: '®',
    trade: '™',
    bull: '•',
    middot: '·',
    deg: '°',
    times: '×',
  };
  /// Jsoup's `Parser.unescapeEntities`: character references in text become their characters.
  const unescapeEntities = (text) =>
    KT.str(text).replace(/&(#[xX][0-9a-fA-F]+|#\d+|[a-zA-Z]+);/g, (whole, code) => {
      if (code[0] !== '#') return ENTITIES[code] ?? ENTITIES[code.toLowerCase()] ?? whole;
      const point =
        code[1] === 'x' || code[1] === 'X'
          ? parseInt(code.slice(2), 16)
          : parseInt(code.slice(1), 10);
      return Number.isFinite(point) && point <= 0x10ffff ? String.fromCodePoint(point) : whole;
    });

  /// Keiyoushi's `GraphQLErrorInterceptor`: an answer naming GraphQL errors fails with them.
  const graphQLErrors = async (chain) => {
    const response = await chain.proceed(chain.request());
    if (response.isSuccessful) return response;
    let errors = null;
    try {
      errors = JSON.parse(await response.body.string()).errors;
    } catch {
      errors = null;
    }
    if (Array.isArray(errors) && errors.length) {
      throw new KotlinError(
        errors.map((error) => KT.str(error?.message ?? '')).join('\n'),
        'GraphQLException',
      );
    }
    return response;
  };

  /// kotlinx.coroutines' `Mutex`: one `withLock` at a time, the others wait in turn.
  KT.mutex = () => {
    let tail = Promise.resolve();
    let locked = false;
    let held = null;
    const acquire = async () => {
      const before = tail;
      let release;
      tail = new Promise((resolve) => {
        release = resolve;
      });
      await before;
      locked = true;
      return () => {
        locked = false;
        release();
      };
    };
    const mutex = {
      withLock: async (...args) => {
        const release = await acquire();
        try {
          return await args[args.length - 1]();
        } finally {
          release();
        }
      },
      lock: async () => {
        held = await acquire();
      },
      unlock: () => {
        const release = held;
        held = null;
        if (release) release();
      },
    };
    Object.defineProperty(mutex, 'isLocked', { get: () => locked, enumerable: false });
    return mutex;
  };
  /// kotlinx.coroutines' `Semaphore`: so many `withPermit` blocks at once.
  KT.semaphore = (permits) => {
    let free = permits;
    const waiting = [];
    const acquire = async () => {
      if (free > 0) {
        free -= 1;
        return;
      }
      await new Promise((resolve) => waiting.push(resolve));
    };
    const release = () => {
      const next = waiting.shift();
      if (next) next();
      else free += 1;
    };
    return {
      withPermit: async (...args) => {
        await acquire();
        try {
          return await args[args.length - 1]();
        } finally {
          release();
        }
      },
      acquire,
      release,
    };
  };
  const sharedPreferences = (() => {
    const store = new Map();
    const preferences = {
      getString: (key, fallback) => store.get(key) ?? fallback ?? null,
      getBoolean: (key, fallback) => store.get(key) ?? fallback ?? false,
      getInt: (key, fallback) => store.get(key) ?? fallback ?? 0,
      getLong: (key, fallback) => store.get(key) ?? fallback ?? 0,
      getStringSet: (key, fallback) => store.get(key) ?? fallback ?? [],
      contains: (key) => store.has(key),
      edit: () => {
        const editor = {
          putString: (key, value) => (store.set(key, value), editor),
          putBoolean: (key, value) => (store.set(key, value), editor),
          putInt: (key, value) => (store.set(key, value), editor),
          putLong: (key, value) => (store.set(key, value), editor),
          putStringSet: (key, value) => (store.set(key, value), editor),
          remove: (key) => (store.delete(key), editor),
          apply: () => {},
          commit: () => true,
        };
        return editor;
      },
    };
    return preferences;
  })();
  const OBJECTS = {
    SManga: {
      create: () => manga(),
      UNKNOWN: 0,
      ONGOING: 1,
      COMPLETED: 2,
      LICENSED: 3,
      PUBLISHING_FINISHED: 4,
      CANCELLED: 5,
      ON_HIATUS: 6,
    },
    SChapter: { create: () => chapter() },
    Jsoup: {
      parse: (html, base = '') => Jsoup.parse(html, KT.str(base)),
      parseBodyFragment: (html, base = '') =>
        Jsoup.parse(`<body>${KT.str(html)}</body>`, KT.str(base)),
      clean: (html) => Jsoup.parse(html).text(),
    },
    Fetch,
    Locale: {
      ENGLISH: K.locale('en'),
      US: K.locale('en', 'US'),
      ROOT: K.locale('root'),
      UK: K.locale('en', 'GB'),
      JAPANESE: K.locale('ja'),
      getDefault: () => K.locale('en'),
      forLanguageTag: (tag) => K.locale(tag),
      of: (language, country) => K.locale(language, country),
    },
    TimeZone: {
      getTimeZone: (name) => zone(K.zone(name)),
      getDefault: () => zone(-new Date().getTimezoneOffset()),
    },
    ZoneId: {
      of: (name) => zone(K.zone(name)),
      systemDefault: () => zone(-new Date().getTimezoneOffset()),
    },
    ZoneOffset: { UTC: zone(0), ofHours: (hours) => zone(hours * 60) },
    Calendar: {
      ...CALENDAR,
      getInstance: (where) => calendar(where && where.isZone ? where.offset : undefined),
    },
    TimeUnit: {
      MILLISECONDS: { unitMs: 1 },
      SECONDS: { unitMs: 1000 },
      MINUTES: { unitMs: 60000 },
      HOURS: { unitMs: 3600000 },
      DAYS: { unitMs: 86400000 },
    },
    CacheControl: {
      FORCE_NETWORK: { isCacheControl: true },
      FORCE_CACHE: { isCacheControl: true },
    },
    UUID: {
      randomUUID: () =>
        'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (char) => {
          const random = Math.floor(Math.random() * 16);
          return (char === 'x' ? random : (random % 4) + 8).toString(16);
        }),
    },
    Json: jsonCodec,
    MediaType: { parse: (text) => KT.str(text), get: (text) => KT.str(text) },
    ResponseBody: {
      create: (first, second) =>
        typeof first === 'string' ? KT.textBody(first, second) : KT.textBody(second, first),
    },
    String: {
      // `String.format(Locale.US, "%02d", n)` too: the locale changes nothing here.
      format: (...args) => {
        if (args[0] && typeof args[0] === 'object' && args[0].language !== undefined) args.shift();
        const [pattern, ...rest] = args;
        return K.format(pattern, ...rest);
      },
      valueOf: (value) => KT.str(value),
    },
    Character: {
      isDigit: (char) => /^\p{Nd}$/u.test(char),
      isLetter: (char) => /^\p{L}$/u.test(char),
      isLetterOrDigit: (char) => /^[\p{L}\p{Nd}]$/u.test(char),
      isWhitespace: (char) => /^\s$/.test(char),
    },
    Unit: undefined,
    // The Mihon release a source may name in its User-Agent.
    AppInfo: { getVersionName: () => '0.19.1', getVersionCode: () => 19 },
    // Android's log: nothing kept.
    Log: { e: () => 0, w: () => 0, i: () => 0, d: () => 0, v: () => 0 },
    // `Regex.escape(text)`: the text matched as it is.
    Regex: {
      escape: (text) => KT.str(text).replace(/[.*+?^${}()|[\]\\]/g, '\\$&'),
      fromLiteral: (text) => K.regex(KT.str(text).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
    },
    ChronoUnit: {
      MILLIS: 'MILLIS',
      SECONDS: 'SECONDS',
      MINUTES: 'MINUTES',
      HOURS: 'HOURS',
      DAYS: 'DAYS',
      WEEKS: 'WEEKS',
      MONTHS: 'MONTHS',
      YEARS: 'YEARS',
    },
    ChronoField: {},
    LocalDate: {
      now: () =>
        moment(Math.floor((Date.now() + localOffset() * 60000) / DAY_MS) * DAY_MS, 'LocalDate'),
      parse: (text, format) => moment(wallClock(text, format, 'LocalDate'), 'LocalDate'),
      of: (year, month, day) => moment(Date.UTC(year, month - 1, day), 'LocalDate'),
      ofEpochDay: (days) => moment(days * DAY_MS, 'LocalDate'),
    },
    LocalDateTime: {
      now: () => moment(Date.now() + localOffset() * 60000, 'LocalDateTime'),
      parse: (text, format) => moment(wallClock(text, format, 'LocalDateTime'), 'LocalDateTime'),
      of: (year, month, day, hour = 0, minute = 0, second = 0) =>
        moment(Date.UTC(year, month - 1, day, hour, minute, second), 'LocalDateTime'),
    },
    ZonedDateTime: {
      now: () => moment(Date.now(), 'ZonedDateTime'),
      // A zoned text names its offset; without a format it's read as ISO 8601.
      parse: (text, format) =>
        moment(
          format && typeof format.parse === 'function'
            ? wallClock(text, format, 'ZonedDateTime') - format.offsetMinutes * 60000
            : wallClock(text, null, 'ZonedDateTime'),
          'ZonedDateTime',
        ),
    },
    Parser: { unescapeEntities: (text) => unescapeEntities(text) },
    DateTimeFormatter: {
      ofPattern: (pattern, locale) => K.dateFormat(pattern, locale),
      ISO_LOCAL_DATE: K.dateFormat('yyyy-MM-dd'),
      ISO_LOCAL_DATE_TIME: K.dateFormat("yyyy-MM-dd'T'HH:mm:ss"),
    },
    // `System.getProperty("http.agent")`: the agent Android's own HTTP stack names.
    System: {
      currentTimeMillis: () => Date.now(),
      getProperty: (name) =>
        KT.str(name) === 'http.agent' ? 'Dalvik/2.1.0 (Linux; U; Android 14)' : null,
    },
    FormBody: { Builder: () => formBody() },
    Headers: {
      Builder: () => new HeadersBuilder(),
      headersOf: (...pairs) =>
        new Headers(
          pairs.reduce(
            (all, item, index) => (index % 2 ? all : [...all, [item, pairs[index + 1]]]),
            [],
          ),
        ),
    },
    Request: { Builder: () => new RequestBuilder() },
    RegexOption: { IGNORE_CASE: 'i', MULTILINE: 'm', DOT_MATCHES_ALL: 's' },
    // Set with the filters below.
    Filter: null,
    Math: {
      max: Math.max,
      min: Math.min,
      abs: Math.abs,
      floor: Math.floor,
      ceil: Math.ceil,
      round: Math.round,
      pow: Math.pow,
    },
    Integer: { parseInt: (text) => parseInt(text, 10), MAX_VALUE: 2147483647 },
    Long: { MAX_VALUE: Number.MAX_SAFE_INTEGER },
    Int: { MAX_VALUE: 2147483647 },
    URLEncoder: { encode: (text) => encodeURIComponent(KT.str(text)).replace(/%20/g, '+') },
    URLDecoder: { decode: (text) => decodeURIComponent(KT.str(text).replace(/\+/g, ' ')) },
    Charsets: { UTF_8: 'UTF-8' },
    Base64: {
      decode: (text) => K.base64Decode(text),
      DEFAULT: 0,
      NO_WRAP: 2,
      URL_SAFE: 8,
      encodeToString: (text) => btoa(KT.str(text)),
    },
    JsonNull: null,
    Instant: {
      parse: (text) => instant(Date.parse(KT.str(text))),
      fromEpochMilliseconds: (ms) => instant(ms),
      ofEpochMilli: (ms) => instant(ms),
      ofEpochSecond: (seconds) => instant(seconds * 1000),
      now: () => instant(Date.now()),
    },
    Clock: { System: { now: () => instant(Date.now()) } },
  };
  KT.global = (name) => {
    if (Object.prototype.hasOwnProperty.call(OBJECTS, name)) return OBJECTS[name];
    if (GLOBALS[name]) return async (...args) => GLOBALS[name](args);
    throw new KotlinError(`Raygun's runtime has no ${name}`);
  };
  KT.call = async (name, args, named_, lambda, type) => {
    const function_ = GLOBALS[name];
    if (function_) return function_(args, named_, lambda, type);
    const object = OBJECTS[name];
    if (object && typeof object === 'function') return object(...args);
    throw new KotlinError(`Raygun's runtime has no ${name}()`);
  };

  // Durations (`1.seconds`) as numbers' properties.
  for (const [unit, ms] of Object.entries(durations))
    NUMBER[unit] = (n) => ({ ms: n * ms, isDuration: true });

  // ---- Classes a source declares ----

  // ---- Mihon's filters, as Raygun's ----

  const FILTER_KINDS = new Set([
    'Select',
    'Sort',
    'CheckBox',
    'TriState',
    'Group',
    'Text',
    'Header',
    'Separator',
  ]);
  const FILTER_STATES = { CheckBox: false, TriState: 0, Text: '', Header: null, Separator: null };
  /// The kind of Mihon filter a class builds on (`Filter.Select<String>` → `Select`).
  const filterKindOf = (supers) => {
    for (const parent of supers ?? []) {
      const short = parent.split('<')[0].split('.').pop();
      if (FILTER_KINDS.has(short)) return short;
    }
    return null;
  };
  const filterKind = (filter) =>
    filter && Array.isArray(filter.__classes)
      ? (filter.__classes.find((name) => FILTER_KINDS.has(name)) ?? null)
      : null;
  /// `Filter.Header("…")` and its kin, made directly.
  const filterObject = (kind, fields) => ({
    __class: `Filter.${kind}`,
    __classes: [kind, 'Filter'],
    ...fields,
  });
  const FILTER_CONSTRUCTORS = {
    Header: (name) => filterObject('Header', { name, state: null }),
    Separator: (name = '') => filterObject('Separator', { name, state: null }),
    Text: (name, state = '') => filterObject('Text', { name, state }),
    CheckBox: (name, state = false) => filterObject('CheckBox', { name, state }),
    TriState: (name, state = 0) => filterObject('TriState', { name, state }),
    Select: (name, values, state = 0) =>
      filterObject('Select', { name, values: [...(values ?? [])], state }),
    Group: (name, state) => filterObject('Group', { name, state: [...(state ?? [])] }),
    Sort: (name, values, state = null) =>
      filterObject('Sort', { name, values: [...(values ?? [])], state }),
  };
  OBJECTS.Filter = FILTER_CONSTRUCTORS;
  /// A source's filters as Raygun shows them, keyed by place (`f0`, `f1`…): a select or a sort
  /// picks one, a checkbox toggles, a group of checkboxes or three-state boxes is tags. Text
  /// fields aren't shown.
  const describeFilters = (list) => {
    const out = [];
    list.forEach((filter, index) => {
      const kind = filterKind(filter);
      const key = `f${index}`;
      const label = KT.str(filter.name ?? '');
      if (kind === 'Select') {
        out.push({
          kind: 'select',
          key,
          label,
          options: (filter.values ?? []).map((value, at) => ({
            value: String(at),
            label: KT.str(value),
          })),
          default: String(filter.state ?? 0),
        });
      } else if (kind === 'Sort') {
        const options = (filter.values ?? []).flatMap((value, at) => [
          { value: `${at}:d`, label: `${KT.str(value)} ↓` },
          { value: `${at}:a`, label: `${KT.str(value)} ↑` },
        ]);
        const state = filter.state;
        out.push({
          kind: 'select',
          key,
          label,
          options,
          default: state ? `${state.index ?? 0}:${state.ascending ? 'a' : 'd'}` : undefined,
        });
      } else if (kind === 'CheckBox') {
        out.push({ kind: 'toggle', key, label, default: Boolean(filter.state) });
      } else if (kind === 'TriState') {
        out.push({ kind: 'tags', key, label, options: [{ value: '0', label }] });
      } else if (kind === 'Group') {
        const children = Array.isArray(filter.state) ? filter.state : [];
        out.push({
          kind: 'tags',
          key,
          label,
          options: children.map((child, at) => ({
            value: String(at),
            label: KT.str(child?.name ?? ''),
          })),
        });
      }
    });
    return out.filter((filter) => filter.kind !== 'select' || filter.options.length).slice(0, 20);
  };
  /// The source's filters set as Raygun's values ask.
  const applyFilters = (list, values) => {
    list.forEach((filter, index) => {
      const value = values[`f${index}`];
      if (value === undefined || value === null) return;
      const kind = filterKind(filter);
      if (kind === 'Select' && typeof value === 'string') {
        filter.state = parseInt(value, 10) || 0;
      } else if (kind === 'Sort' && typeof value === 'string') {
        const [at, direction] = value.split(':');
        filter.state = { index: parseInt(at, 10) || 0, ascending: direction === 'a' };
      } else if (kind === 'CheckBox') {
        filter.state = value === true;
      } else if (kind === 'TriState' && typeof value === 'object') {
        filter.state = value.include?.length ? 1 : value.exclude?.length ? 2 : 0;
      } else if (kind === 'Group' && typeof value === 'object') {
        const children = Array.isArray(filter.state) ? filter.state : [];
        children.forEach((child, at) => {
          const included = (value.include ?? []).includes(String(at));
          const excluded = (value.exclude ?? []).includes(String(at));
          if (filterKind(child) === 'TriState') child.state = included ? 1 : excluded ? 2 : 0;
          else child.state = included;
        });
      }
    });
    return list;
  };

  KT.defineClass = (definition) => {
    /// An instance with these fields (`undefined` ones take their defaults).
    const build = async (fields) => {
      const object = {
        __class: definition.name,
        __classes: [
          definition.name,
          ...definition.supers.map((name) => name.split('<')[0].split('.').pop()),
        ],
        __fields: definition.params,
      };
      for (const name of definition.params) {
        let value = fields[name];
        if (value === undefined && definition.defaults[name])
          value = await definition.defaults[name]();
        object[name] = value ?? null;
      }
      const parentName = definition.supers.length
        ? definition.supers[0].split('<')[0].split('.').pop()
        : null;
      const parentClass = parentName && parentName !== definition.name ? CLASSES[parentName] : null;
      if (definition.superArgs) {
        const parent = await definition.superArgs(...definition.params.map((name) => object[name]));
        object.__super = parent;
        // Filters keep what Mihon's do: a name and a state (and a select's or a sort's values).
        if (Array.isArray(parent) && !parentClass) {
          const kind = filterKindOf(definition.supers);
          if (object.name === undefined) object.name = parent[0];
          if (kind === 'Select' || kind === 'Sort') {
            if (object.values === undefined) object.values = parent[1] ?? [];
            if (object.state === undefined)
              object.state = parent[2] ?? (kind === 'Select' ? 0 : null);
          } else if (kind === 'Group') {
            if (object.state === undefined) object.state = parent[1] ?? [];
          } else if (kind) {
            if (object.state === undefined) object.state = parent[1] ?? FILTER_STATES[kind];
          } else {
            if (object.state === undefined)
              object.state = Array.isArray(parent[1]) ? 0 : (parent[1] ?? null);
            if (object.values === undefined && Array.isArray(parent[1])) object.values = parent[1];
          }
        }
      }
      for (const [name, member] of Object.entries(definition.members)) {
        if (member && member.isGetter)
          Object.defineProperty(object, name, {
            get: () => member.function_(object),
            enumerable: false,
          });
        else object[name] = (...rest) => member(object, ...rest);
      }
      // A parent the source declares too (`class SortFilter : SelectFilter("Sort", sort)`): made
      // from the arguments passed to it, it lends what this class doesn't have itself.
      if (parentClass) {
        const inherited = await parentClass.make(
          Array.isArray(object.__super) ? object.__super : [],
        );
        for (const name of Object.getOwnPropertyNames(inherited)) {
          if (name.startsWith('__') || Object.prototype.hasOwnProperty.call(object, name)) continue;
          const descriptor = Object.getOwnPropertyDescriptor(inherited, name);
          // Fields stay the parent's own, so its getters see what's set here (a filter's state).
          if ('value' in descriptor && typeof descriptor.value !== 'function') {
            Object.defineProperty(object, name, {
              get: () => inherited[name],
              set: (value) => {
                inherited[name] = value;
              },
              enumerable: true,
              configurable: true,
            });
          } else {
            Object.defineProperty(object, name, descriptor);
          }
        }
        for (const name of inherited.__classes)
          if (!object.__classes.includes(name)) object.__classes.push(name);
      }
      object.toString = () =>
        `${definition.name}(${definition.params.map((name) => `${name}=${KT.str(object[name])}`).join(', ')})`;
      object.copy = (...rest) =>
        make(
          definition.params.map((name, index) =>
            index < rest.length ? rest[index] : object[name],
          ),
          named(rest),
        );
      return object;
    };
    /// Made from constructor arguments, in order or by name.
    const make = async (args = [], named_ = null) => {
      const fields = {};
      definition.params.forEach((name, index) => {
        fields[name] =
          named_ && Object.prototype.hasOwnProperty.call(named_, name) ? named_[name] : args[index];
      });
      return build(fields);
    };
    /// Decoded from JSON: each field by its JSON name, as its type says.
    const fromJson = async (json, typeArgs = []) => {
      const fields = {};
      for (const name of definition.params) {
        const value = field(json, name);
        const type = substitute(
          parseType(definition.types?.[name] ?? 'Any'),
          definition.typeParams ?? [],
          typeArgs,
        );
        fields[name] = value === undefined ? undefined : await decodeAs(value, type);
      }
      return build(fields);
    };
    const holder = { isClass: true, name: definition.name, make, fromJson };
    CLASSES[definition.name] = holder;
    for (const entry of definition.entries)
      holder[entry] = {
        __class: definition.name,
        __classes: [definition.name],
        name: entry,
        ordinal: definition.entries.indexOf(entry),
        toString: () => entry,
      };
    holder.values = () => definition.entries.map((entry) => holder[entry]);
    holder.entries = definition.entries.map((entry) => holder[entry]);
    if (definition.kind === 'object') {
      for (const [name, member] of Object.entries(definition.members))
        holder[name] = (...rest) => member(holder, ...rest);
    }
    return holder;
  };
  KT.make = async (holder, args, named_) => holder.make(args, named_);
  /// An interceptor class the converter leaves out (a theme's image unscrambler, which Raygun's
  /// image fetcher stands in for): it passes requests on.
  KT.passThrough = (name) => ({
    isClass: true,
    name,
    make: async () => ({
      __class: name,
      __classes: [name, 'Interceptor'],
      intercept: async (chain) => chain.proceed(chain.request()),
    }),
    fromJson: async () => KT.raise(`${name} isn't data`),
  });
  /// A page's address marked for Raygun's image fetcher when its picture comes in shuffled
  /// tiles (`#scramble=[…]` from ComiciViewer, `#scramble` from GigaViewer).
  const unscrambled = (url, recipe) => {
    const at = url.indexOf('#');
    if (!recipe || at < 0) return url;
    const fragment = url.slice(at + 1);
    if (recipe === 'giga' && fragment === 'scramble')
      return `${url.slice(0, at)}#raygun-unscramble=giga`;
    if (recipe === 'comici' && fragment.startsWith('scramble=')) {
      return `${url.slice(0, at)}#raygun-unscramble=comici:${fragment.slice('scramble='.length).replace(/[[\]\s]/g, '')}`;
    }
    return url;
  };

  // ---- Receivers ----

  const hasMember = (receiver, name) => {
    if (receiver === null || receiver === undefined) return false;
    const kind = kindOf(receiver);
    if (kind === 'list' || kind === 'elements')
      return Boolean(LIST[name]) || (kind === 'elements' && typeof receiver[name] === 'function');
    if (kind === 'string') return Boolean(STRING[name]);
    if (kind === 'map') return Boolean(MAP[name]);
    if (typeof receiver[name] === 'function') return true;
    return receiver.isStringBuilder && name in receiver;
  };
  const hasField = (receiver, name) => {
    if (receiver === null || receiver === undefined) return false;
    // A list's, a string's or a map's own properties (`size` in `buildList { … }`).
    const kind = kindOf(receiver);
    if (kind === 'list' || kind === 'elements')
      return ['size', 'lastIndex', 'indices'].includes(name);
    if (kind === 'string') return ['length', 'lastIndex', 'indices'].includes(name);
    if (kind === 'map') return ['size', 'keys', 'values', 'entries'].includes(name);
    return (
      typeof receiver === 'object' &&
      (Object.prototype.hasOwnProperty.call(receiver, name) ||
        Boolean(receiver.isGetterHost && receiver.getters && receiver.getters[name]) ||
        classGetter(receiver, name))
    );
  };

  KT.rget = async (receivers, name) => {
    for (const receiver of receivers) {
      if (receiver.isSource) {
        if (receiver.has(name)) return receiver.get(name);
        continue;
      }
      if (hasField(receiver, name)) return KT.p(receiver, name);
    }
    const source = receivers[receivers.length - 1];
    if (source.constants && source.constants.has(name)) return source.constants.get(name);
    if (source.classes && source.classes[name]) return source.classes[name];
    // An object of Mihon's or Kotlin's (`SManga`, `Locale`).
    return KT.global(name);
  };
  KT.rset = async (receivers, name, value) => {
    for (const receiver of receivers) {
      if (receiver.isSource) {
        if (receiver.has(name)) return receiver.set(name, value);
        continue;
      }
      if (hasField(receiver, name)) return KT.setp(receiver, name, value);
    }
    throw new KotlinError(`Raygun's runtime doesn't know ${name}`);
  };
  KT.rcall = async (receivers, name, args, named_, lambda, type) => {
    for (const receiver of receivers) {
      if (receiver.isSource) {
        if (receiver.hasFunction(name))
          return receiver.call(name, lambda ? [...args, lambda] : args);
        if (receiver.hasExtension(name)) continue;
        continue;
      }
      if (hasMember(receiver, name) || (receiver.isJsonBuilder && name in receiver))
        return KT.m(receiver, name, args, named_, lambda, type);
      const source = receivers[receivers.length - 1];
      if (source.isSource && source.hasExtension(name))
        return source.ext(name, receiver, lambda ? [...args, lambda] : args);
    }
    return KT.call(name, args, named_, lambda, type);
  };

  // ---- A converted source ----

  const STATUS_NAMES = {
    0: 'unknown',
    1: 'ongoing',
    2: 'completed',
    3: 'unknown',
    4: 'publishingFinished',
    5: 'cancelled',
    6: 'hiatus',
  };

  /// What the source's own code answers, as Raygun's `manga.source` calls.
  /// A source made of layers, as Kotlin's classes inherit: a theme's classes beneath, the
  /// source's own on top. Calls and reads find the topmost layer that has the name; `super` in a
  /// layer finds the layers beneath it, then KeiSource's own.
  const makeSource = (t, builds) => {
    const layers = builds.map(() => ({ functions: new Map(), properties: new Map() }));
    const values = new Map();
    const extensions = new Map();
    const constants = new Map();
    const constantValues = new Map();
    const C = {
      prop: (name, getter) => constants.set(name, getter),
      has: (name) => constants.has(name),
      get: async (name) => {
        if (!constantValues.has(name)) constantValues.set(name, constants.get(name)());
        return constantValues.get(name);
      },
    };
    const D = {};
    const client = new Client(t);
    const defaultHeaders = async () => {
      const builder = new HeadersBuilder().set('Referer', `${t.baseUrl}/`);
      if (extensions.has('configureHeaders')) await extensions.get('configureHeaders')(builder);
      return builder;
    };
    let headers = null;
    const base = {
      baseUrl: async () => t.baseUrl,
      lang: async () => t.lang,
      name: async () => t.name,
      id: async () => t.id,
      versionId: async () => 1,
      supportsLatest: async () => true,
      client: async () => client,
      network: async () => ({ client, cloudflareClient: client }),
      headers: async () => (headers ??= (await defaultHeaders()).build()),
      json: async () => ({
        decodeFromString: (text) => JSON.parse(KT.str(text)),
        parseToJsonElement: (text) => JSON.parse(KT.str(text)),
        encodeToString: (value) => JSON.stringify(plain(value)),
        decodeFromJsonElement: (value) => value,
      }),
      preferences: async () => preferences,
    };
    const baseFunctions = {
      headersBuilder: async () => defaultHeaders(),
      getMangaUrl: async (item) => t.baseUrl + item.url,
      getChapterUrl: async (item) => t.baseUrl + item.url,
      getImageRequest: async (page) =>
        new Request({ url: page.imageUrl, headers: await self.get('headers') }),
      imageRequest: async (page) => self.call('getImageRequest', [page]),
      // Filters aren't shown yet: code asking for them gets none.
      getFilterList: async () => [],
    };
    const preferences = {
      store: new Map(),
      getString: (key, fallback) => preferences.store.get(key) ?? fallback ?? null,
      getBoolean: (key, fallback) => preferences.store.get(key) ?? fallback ?? false,
      getInt: (key, fallback) => preferences.store.get(key) ?? fallback ?? 0,
      getStringSet: (key, fallback) => preferences.store.get(key) ?? fallback ?? [],
      contains: (key) => preferences.store.has(key),
      edit: () => {
        const editor = {
          putString: (key, value) => (preferences.store.set(key, value), editor),
          putBoolean: (key, value) => (preferences.store.set(key, value), editor),
          putInt: (key, value) => (preferences.store.set(key, value), editor),
          remove: (key) => (preferences.store.delete(key), editor),
          apply: () => {},
          commit: () => true,
        };
        return editor;
      },
    };
    /// The function `name` taking `args`, from the layers beneath `below`, topmost first.
    const findFunction = (name, args, below) => {
      let fallback = null;
      for (let index = below - 1; index >= 0; index--) {
        const overloads = layers[index].functions.get(name);
        if (!overloads) continue;
        const fits = overloads.filter(
          ([required, total]) => args.length >= required && args.length <= total,
        );
        const chosen = fits.find(([, total]) => total === args.length) ?? fits[0];
        if (chosen) return chosen[2];
        fallback ??= overloads[0][2];
      }
      return fallback;
    };
    /// The layer beneath `below` that has the property `name`, topmost first.
    const findProperty = (name, below) => {
      for (let index = below - 1; index >= 0; index--) {
        if (layers[index].properties.has(name)) return index;
      }
      return -1;
    };
    const read = async (name, below) => {
      const index = findProperty(name, below);
      if (index >= 0) {
        const { getter, once } = layers[index].properties.get(name);
        if (!once) return getter();
        const key = `${index}:${name}`;
        if (!values.has(key)) values.set(key, getter());
        return values.get(key);
      }
      if (base[name]) return base[name]();
      throw new KotlinError(`the source has no ${name}`);
    };
    const invoke = async (name, args, below) => {
      const function_ = findFunction(name, args, below);
      if (function_) return function_(...args);
      if (baseFunctions[name]) return baseFunctions[name](...args);
      throw new KotlinError(`the source has no ${name}`);
    };
    const top = layers.length;
    /// The source as layer `index`'s code sees it: its definitions go there, `super` is beneath.
    const layerSelf = (index) => ({
      isSource: true,
      constants: C,
      classes: D,
      define: (name, overloads) => layers[index].functions.set(name, overloads),
      prop: (name, getter, once = true) => layers[index].properties.set(name, { getter, once }),
      extension: (name, function_) => extensions.set(name, function_),
      has: (name) =>
        findProperty(name, top) >= 0 || Object.prototype.hasOwnProperty.call(base, name),
      hasFunction: (name) =>
        layers.some((layer) => layer.functions.has(name)) ||
        Object.prototype.hasOwnProperty.call(baseFunctions, name),
      hasExtension: (name) => extensions.has(name),
      get: (name) => read(name, top),
      set: async (name, value) => {
        const at = findProperty(name, top);
        const layer = at >= 0 ? at : top - 1;
        layers[layer].properties.set(name, { getter: async () => value, once: true });
        values.set(`${layer}:${name}`, value);
      },
      call: (name, args) => invoke(name, args, top),
      ext: async (name, receiver, args) => {
        const function_ = extensions.get(name);
        if (!function_) return KT.m(receiver, name, args);
        return function_(receiver, ...args);
      },
      base: {
        get: async (name) =>
          findProperty(name, index) >= 0 || base[name]
            ? read(name, index)
            : KT.raise(`no ${name} to inherit`),
        call: async (name, args) =>
          findFunction(name, args, index) || baseFunctions[name]
            ? invoke(name, args, index)
            : KT.raise(`no ${name} to inherit`),
      },
    });
    const self = layerSelf(top - 1);
    builds.forEach((build, index) => build(index === top - 1 ? self : layerSelf(index), C, D));
    client.defaultHeaders = () => self.get('headers');
    // The client's rate limit and timeouts, as `configureClient` sets them.
    if (extensions.has('configureClient')) {
      self.ready = extensions
        .get('configureClient')(clientBuilder(client))
        .catch(() => {});
    }
    return self;
  };

  const listing = (page) => ({
    items: (page.mangas ?? [])
      .map((item) => ({
        id: item.url,
        title: K.str(item.title).trim(),
        coverUrl: item.thumbnail_url || undefined,
        coverHeaders: undefined,
      }))
      .filter((item) => item.id && item.title),
    hasNextPage: Boolean(page.hasNextPage),
  });

  mihon.source = (config) => {
    renames = { ...renames, ...(config.renames ?? {}) };
    messages = { ...messages, ...(config.messages ?? {}) };
    for (const source of config.sources) {
      const t = { id: source.id, name: source.name, lang: source.lang, baseUrl: source.baseUrl };
      t.http = new Http(t, {});
      const self = makeSource(t, config.layers ?? [config.build]);
      const ready = async () => {
        if (self.ready) await self.ready;
      };
      const filterList = async () => {
        try {
          const list = await self.call('getFilterList', []);
          return Array.isArray(list) ? list : [];
        } catch {
          return [];
        }
      };
      const covers = (result) => {
        for (const item of result.items) {
          if (item.coverUrl) {
            item.coverUrl = K.resolve(item.coverUrl, `${t.baseUrl}/`);
            item.coverHeaders = { Referer: `${t.baseUrl}/` };
          }
        }
        return result;
      };
      const update = async (id, details) => {
        await ready();
        const answer = await self.call('fetchMangaUpdate', [
          manga({ url: id }),
          [],
          details,
          !details,
        ]);
        return answer;
      };
      raygun.providers.register({
        kind: 'manga.source',
        id: source.id,
        name: source.name,
        popular: async ({ page = 1 }) => {
          await ready();
          return covers(listing(await self.call('getPopularManga', [page])));
        },
        latest: async ({ page = 1 }) => {
          await ready();
          if (!KT.truthy(await self.get('supportsLatest')))
            throw new Error('provider has no method latest');
          return covers(listing(await self.call('getLatestUpdates', [page])));
        },
        // The source's own filters (`getFilterList()`); none when they couldn't be converted.
        filters: async () => {
          await ready();
          return describeFilters(await filterList());
        },
        search: async ({ query = '', page = 1, filters = {} }) => {
          await ready();
          const list = applyFilters(await filterList(), filters ?? {});
          return covers(listing(await self.call('getSearchMangaList', [page, query, list])));
        },
        details: async ({ id }) => {
          const answer = await update(id, true);
          const item = answer.manga;
          return toDetails(t, id, {
            ...item,
            webUrl: await self.call('getMangaUrl', [manga({ url: id })]).catch(() => undefined),
          });
        },
        chapters: async ({ id }) => {
          const answer = await update(id, false);
          return {
            chapters: (answer.chapters ?? []).map((item) => ({
              id: item.url,
              name: K.str(item.name).trim() || undefined,
              number: item.chapter_number >= 0 ? item.chapter_number : undefined,
              dateUpload: item.date_upload > 0 ? item.date_upload : undefined,
              scanlator: item.scanlator || undefined,
            })),
          };
        },
        pages: async ({ chapterId }) => {
          await ready();
          const pages = await self.call('getPageList', [chapter({ url: chapterId })]);
          const out = [];
          for (const page of pages) {
            if (!page.imageUrl) continue;
            const request = await self.call('getImageRequest', [page]);
            out.push({
              url: unscrambled(request.url.toString(), config.imageRecipe),
              headers: request.headers.object(),
            });
          }
          return { pages: out };
        },
      });
    }
  };

  KT.statusName = (status) => STATUS_NAMES[status] ?? 'unknown';
  globalThis.KT = KT;
})();
