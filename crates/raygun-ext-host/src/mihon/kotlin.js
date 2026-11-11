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
  /// A `break` or `continue` (`kind`) out of a lambda or an expression, to the loop `loop`, whose
  /// body catches it.
  class Jump {
    constructor(kind, loop) {
      this.kind = kind;
      this.loop = loop;
    }
  }
  KT.Jump = Jump;
  KT.jump = (kind, loop) => {
    throw new Jump(kind, loop);
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
    if (error instanceof Return || error instanceof Jump) return false;
    const name = K.str(type).split('.').pop();
    if (
      ['Exception', 'Throwable', 'Error', 'RuntimeException', 'IOException', 'Any'].includes(name)
    )
      return true;
    // Text that isn't JSON fails as kotlinx's `SerializationException` does (an
    // `IllegalArgumentException`): `catch (e: SerializationException)` takes it.
    if (
      error instanceof SyntaxError &&
      ['SerializationException', 'JsonDecodingException', 'JsonException'].includes(name)
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
    // Instants and durations: `now - 1.days` is an instant, `end - start` a duration.
    if (
      left &&
      typeof left === 'object' &&
      (left.isInstant || left.isDuration || left.isTimeMark)
    ) {
      if (left.isInstant && right && right.isInstant) return duration(left.ms - right.ms);
      return left.minus(right);
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
    if (typeof left === 'bigint' || typeof right === 'bigint') {
      const [a, b] = [bigIntOf(left), bigIntOf(right)];
      return a < b ? -1 : a > b ? 1 : 0;
    }
    if (typeof left === 'string' && typeof right === 'string')
      return left < right ? -1 : left > right ? 1 : 0;
    // Durations and instants by their milliseconds.
    if (left && typeof left === 'object' && 'ms' in left) left = left.ms;
    if (right && typeof right === 'object' && 'ms' in right) right = right.ms;
    return left - right;
  };
  // Bits of whole numbers: JavaScript's operators work on 32 bits, Kotlin's Long on 64. Numbers
  // that fit an Int (and shifts below 32) get the 32-bit operation, as Kotlin's Int does; a
  // Long's (`(1664525L * state + 1013904223L) and 0xFFFFFFFFL`) the 64-bit one.
  const isInt32 = (n) => Number.isInteger(n) && n >= -2147483648 && n <= 2147483647;
  const big = (n) => (typeof n === 'bigint' ? n : BigInt(Math.trunc(Number(n) || 0)));
  const fromBig = (n) => Number(BigInt.asIntN(64, n));
  // Arithmetic the converter knows the kind of (`kotlin::emit::Num`): an `Int` kept to 32 bits,
  // a `Long`'s exact arithmetic over `BigInt`s and back.
  KT.i32 = (value) => (typeof value === 'number' ? value | 0 : value);
  KT.big = big;
  KT.fromBig = fromBig;
  KT.lmul = (left, right) => {
    const product = left * right;
    return Number.isSafeInteger(product) || !Number.isInteger(left) || !Number.isInteger(right)
      ? product
      : fromBig(big(left) * big(right));
  };
  KT.and = (left, right) =>
    typeof left === 'boolean'
      ? left && right
      : isInt32(left) && isInt32(right)
        ? left & right
        : fromBig(big(left) & big(right));
  KT.or = (left, right) =>
    typeof left === 'boolean'
      ? left || right
      : isInt32(left) && isInt32(right)
        ? left | right
        : fromBig(big(left) | big(right));
  KT.xor = (left, right) =>
    typeof left === 'boolean'
      ? left !== right
      : isInt32(left) && isInt32(right)
        ? left ^ right
        : fromBig(big(left) ^ big(right));
  KT.shl = (left, bits) =>
    isInt32(left) && bits < 32
      ? left << bits
      : fromBig(BigInt.asIntN(64, big(left) << BigInt(bits & 63)));
  KT.shr = (left, bits) =>
    isInt32(left) && bits < 32 ? left >> bits : fromBig(big(left) >> BigInt(bits & 63));
  KT.ushr = (left, bits) =>
    isInt32(left) && bits < 32
      ? (left >>> bits) | 0
      : fromBig(BigInt.asUintN(64, big(left)) >> BigInt(bits & 63));
  KT.inv = (value) => (isInt32(value) ? ~value : fromBig(~big(value)));

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
  /// `from until to`, `from..<to`: of numbers or `BigInteger`s.
  KT.until = (from, to) => KT.range(from, typeof to === 'bigint' ? to - 1n : to - 1);
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
  /// Kotlin's `use`: the block, then the value closed, whatever the block did.
  KT.use = async (value, block) => {
    try {
      return await block();
    } finally {
      if (
        value !== null &&
        value !== undefined &&
        typeof value === 'object' &&
        hasMember(value, 'close')
      )
        await KT.m(value, 'close', []);
    }
  };
  KT.also = async (value, then) => {
    await then();
    return value;
  };
  /// Kotlin's `Result`, as `runCatching` gives it.
  class Result {
    constructor(isSuccess, value, error = null) {
      this.isSuccess = isSuccess;
      this.value = isSuccess ? (value ?? null) : null;
      this.error = error;
      this.isResult = true;
    }
    get isFailure() {
      return !this.isSuccess;
    }
    getOrNull() {
      return this.value;
    }
    getOrThrow() {
      if (this.isSuccess) return this.value;
      throw this.error;
    }
    getOrDefault(fallback) {
      return this.isSuccess ? this.value : fallback;
    }
    async getOrElse(onFailure) {
      return this.isSuccess ? this.value : onFailure(this.error);
    }
    exceptionOrNull() {
      return this.isSuccess ? null : this.error;
    }
    async onSuccess(action) {
      if (this.isSuccess) await action(this.value);
      return this;
    }
    async onFailure(action) {
      if (!this.isSuccess) await action(this.error);
      return this;
    }
    async map(transform) {
      return this.isSuccess ? new Result(true, await transform(this.value)) : this;
    }
    async mapCatching(transform) {
      return this.isSuccess ? KT.runCatching(() => transform(this.value)) : this;
    }
    async recover(transform) {
      return this.isSuccess ? this : new Result(true, await transform(this.error));
    }
    async recoverCatching(transform) {
      return this.isSuccess ? this : KT.runCatching(() => transform(this.error));
    }
    async fold(onSuccess, onFailure) {
      return this.isSuccess ? onSuccess(this.value) : onFailure(this.error);
    }
    toString() {
      return this.isSuccess ? `Success(${KT.str(this.value)})` : `Failure(${this.error})`;
    }
  }
  KT.runCatching = async (work) => {
    try {
      return new Result(true, await work());
    } catch (error) {
      if (error instanceof Return || error instanceof Jump) throw error;
      return new Result(false, null, error);
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
  // Reified type parameters: a call's types ride with its arguments (`KT.typedArgs`), are set
  // aside as the function is entered, and taken by its first statement (`KT.takeTypes`).
  let pendingTypes = null;
  KT.typedArgs = (args, types) => {
    Object.defineProperty(args, '__types', { value: types, enumerable: false });
    return args;
  };
  /// Whether one of an extension's overloads takes `receiver` and `args`: its receiver's type
  /// and its parameters' (`[required, total, [types]]`).
  const extensionTakes = (entry, receiver, args) => {
    if (entry.type && !KT.is(receiver, entry.type)) return false;
    if (!entry.shape) return true;
    const [required, total, types] = entry.shape;
    if (args.length < required || args.length > total) return false;
    return types.every(
      (kind, at) => at >= args.length || kind === null || args[at] == null || KT.is(args[at], kind),
    );
  };
  /// Calls `function_` with `args`, its types set aside for it to take.
  const enter = (function_, args, receiver = undefined) => {
    const types = args && args.__types;
    if (types) pendingTypes = types;
    try {
      return receiver === undefined ? function_(...args) : function_(receiver, ...args);
    } finally {
      if (types) pendingTypes = null;
    }
  };
  /// Arguments with a trailing lambda added, keeping the types they carry.
  const withLambda = (args, lambda) =>
    lambda
      ? args.__types
        ? KT.typedArgs([...args, lambda], args.__types)
        : [...args, lambda]
      : args;
  KT.takeTypes = (count) => {
    const types = pendingTypes ?? [];
    pendingTypes = null;
    return Array.from({ length: count }, (_, index) => types[index] ?? 'Any');
  };
  /// A type's text with the reified parameters in it put in (`NextData<T>`).
  KT.typeWith = (text, types) =>
    KT.str(text).replace(/[A-Za-z_][A-Za-z0-9_]*/g, (word) =>
      Object.prototype.hasOwnProperty.call(types, word) ? KT.str(types[word]) : word,
    );
  KT.getter = (function_) => ({ isGetter: true, function_ });
  /// A class's `var` with a value: a field set when an instance is made.
  KT.field = (function_) => ({ isField: true, function_ });
  /// A class's `init { … }`: run when an instance is made, in order with its fields.
  KT.init = (function_) => ({ isInit: true, function_ });

  // ---- What a value is ----

  const kindOf = (value) => {
    if (typeof value === 'string') return 'string';
    if (typeof value === 'number') return 'number';
    if (typeof value === 'boolean') return 'boolean';
    // Java's `BigInteger` is JavaScript's own.
    if (typeof value === 'bigint') return 'bigint';
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
    Set: Array.isArray,
    MutableSet: Array.isArray,
    // OkHttp's and Mihon's builders (`HttpUrl.Builder`, `Request.Builder`, `Headers.Builder`).
    Builder: (value) => Boolean(value && typeof value.build === 'function'),
    HashSet: Array.isArray,
    LinkedHashSet: Array.isArray,
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
    Elements: (value) => value instanceof mihon.Elements,
    Document: (value) => value && typeof value.location === 'function',
    Response: (value) => value instanceof Response,
    Request: (value) => value instanceof Request,
    Headers: (value) => value instanceof Headers,
    HttpUrl: (value) => value instanceof HttpUrl,
    OkHttpClient: (value) => value instanceof Client,
    HttpSource: (value) => Boolean(value && value.isSource),
    KeiSource: (value) => Boolean(value && value.isSource),
    TextNode: (value) => Boolean(value && value.__class === 'TextNode'),
    Node: (value) =>
      Boolean(value && (typeof value.select === 'function' || value.__class === 'TextNode')),
    Any: (value) => value !== null && value !== undefined,
  };
  KT.is = (value, type) => {
    const name = K.str(type).split('<')[0].split('.').pop();
    if (BUILT_IN_TYPES[name]) return Boolean(BUILT_IN_TYPES[name](value));
    return Boolean(value && value.__classes && value.__classes.includes(name));
  };
  KT.asOrNull = (value, type) => (KT.is(value, type) ? value : null);

  // ---- Bytes ----
  // A `ByteArray` is a list of numbers 0–255; text where bytes are asked for is its UTF-8.

  const utf8Encode = (text) => {
    const out = [];
    for (const char of KT.str(text)) {
      const code = char.codePointAt(0);
      if (code < 0x80) out.push(code);
      else if (code < 0x800) out.push(0xc0 | (code >> 6), 0x80 | (code & 63));
      else if (code < 0x10000)
        out.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
      else
        out.push(
          0xf0 | (code >> 18),
          0x80 | ((code >> 12) & 63),
          0x80 | ((code >> 6) & 63),
          0x80 | (code & 63),
        );
    }
    return out;
  };
  /// UTF-8 bytes as text; a byte that doesn't decode becomes U+FFFD, as Java's decoder does.
  const utf8Decode = (bytes) => {
    let out = '';
    let at = 0;
    while (at < bytes.length) {
      const first = bytes[at] & 255;
      if (first < 0x80) {
        out += String.fromCharCode(first);
        at += 1;
        continue;
      }
      let need = 0;
      let code = 0;
      if (first >> 5 === 6) [need, code] = [1, first & 0x1f];
      else if (first >> 4 === 14) [need, code] = [2, first & 0x0f];
      else if (first >> 3 === 30) [need, code] = [3, first & 0x07];
      let valid = need > 0 && at + need < bytes.length;
      for (let index = 1; valid && index <= need; index++) {
        const next = bytes[at + index] & 255;
        if (next >> 6 !== 2) valid = false;
        else code = (code << 6) | (next & 63);
      }
      if (!valid || code > 0x10ffff) {
        out += '�';
        at += 1;
        continue;
      }
      out += String.fromCodePoint(code);
      at += need + 1;
    }
    return out;
  };
  const isLatin1 = (charset) => /8859-1|latin|us-ascii|ascii/i.test(KT.str(charset ?? ''));
  /// Bytes from a `ByteArray`, or from text in `charset` (UTF-8 unless it says Latin-1).
  const bytesOf = (value, charset = null) => {
    if (Array.isArray(value)) return value.map((byte) => byte & 255);
    if (value instanceof Uint8Array) return [...value];
    if (value === null || value === undefined) return [];
    if (isLatin1(charset)) return [...KT.str(value)].map((char) => char.charCodeAt(0) & 255);
    return utf8Encode(value);
  };
  const textOf = (bytes, charset = null) =>
    isLatin1(charset)
      ? bytes.map((byte) => String.fromCharCode(byte & 255)).join('')
      : utf8Decode(bytes);
  const binaryOf = (bytes) => {
    let out = '';
    for (let at = 0; at < bytes.length; at += 8192)
      out += String.fromCharCode(...bytes.slice(at, at + 8192).map((byte) => byte & 255));
    return out;
  };
  const base64Of = (bytes) => btoa(binaryOf(bytesOf(bytes)));
  const fromBase64 = (text) => {
    let clean = KT.str(text).replace(/\s+/g, '').replace(/-/g, '+').replace(/_/g, '/');
    while (clean.length % 4) clean += '=';
    const binary = atob(clean);
    const out = new Array(binary.length);
    for (let index = 0; index < binary.length; index++) out[index] = binary.charCodeAt(index);
    return out;
  };
  const hexOf = (bytes) =>
    bytesOf(bytes)
      .map((byte) => byte.toString(16).padStart(2, '0'))
      .join('');
  const fromHex = (text) => {
    const clean = KT.str(text).trim();
    if (clean.length % 2) throw new KotlinError('Hex string must have an even length');
    const out = [];
    for (let at = 0; at < clean.length; at += 2) {
      const byte = parseInt(clean.slice(at, at + 2), 16);
      if (Number.isNaN(byte)) throw new KotlinError(`invalid hex: ${clean.slice(at, at + 2)}`);
      out.push(byte);
    }
    return out;
  };
  /// Android's `Base64` flags (and Kotlin's `Base64.UrlSafe`): URL-safe letters, no padding,
  /// and lines of 76 unless `NO_WRAP`.
  const base64Encode = (value, flags = 0) => {
    let text = base64Of(value);
    if (flags & 8) text = text.replace(/\+/g, '-').replace(/\//g, '_');
    if (flags & 1) text = text.replace(/=+$/, '');
    if (!(flags & 2) && flags !== -1) text = `${text.replace(/(.{76})/g, '$1\n')}\n`;
    return text;
  };
  /// Java's `Base64.Decoder` and `Base64.Encoder` (flags as Android's: 1 no padding, 2 no wraps,
  /// 8 URL-safe).
  const javaBase64Decoder = {
    __class: 'Decoder',
    decode: (value) => fromBase64(Array.isArray(value) ? binaryOf(value) : value),
  };
  const javaBase64Encoder = (flags) => ({
    __class: 'Encoder',
    encodeToString: (value) => base64Encode(value, flags),
    encode: (value) => utf8Encode(base64Encode(value, flags)),
    withoutPadding: () => javaBase64Encoder(flags | 1),
  });
  const crypto = (args) => K.crypto(args);
  const digestOf = (algorithm, data) =>
    fromBase64(crypto({ op: 'digest', algorithm, data: base64Of(data) }));
  /// An AES cipher (`aesCbc(key, iv)`, `Cipher.getInstance("AES/CBC/PKCS5Padding")`): its
  /// `doFinal(bytes)` runs it in Rust.
  const aesCipher = (fields) => {
    const cipher = {
      __class: 'Cipher',
      __classes: ['Cipher'],
      isCipher: true,
      mode: 'cbc',
      padding: true,
      encrypt: false,
      key: null,
      iv: null,
      ...fields,
      init: (opmode, keySpec, params = null) => {
        cipher.encrypt = opmode === 1;
        cipher.key = bytesOf(keySpec?.key ?? keySpec);
        if (params) cipher.iv = bytesOf(params.iv ?? params);
      },
      // GCM's additional authenticated data.
      updateAAD: (bytes) => {
        cipher.aad = [...(cipher.aad ?? []), ...bytesOf(bytes)];
      },
      // A picture's bytes stay a `Uint8Array` on the way through (a list of numbers that size
      // would fill the script's memory).
      doFinalU8: (data = []) =>
        K.bin.fromBase64(
          crypto({
            op: 'aes',
            mode: cipher.mode,
            key: base64Of(cipher.key),
            iv: cipher.iv ? base64Of(cipher.iv) : null,
            padding: cipher.padding,
            encrypt: cipher.encrypt,
            data: data instanceof Uint8Array ? K.bin.toBase64(data) : base64Of(data),
            aad: cipher.aad && cipher.aad.length ? base64Of(cipher.aad) : null,
          }),
        ),
      doFinal: (data = []) => listOfBytes(cipher.doFinalU8(data)),
      getIV: () => cipher.iv,
      iv_: null,
    };
    return cipher;
  };
  /// A transformation (`AES/CBC/PKCS5Padding`) as a cipher's mode and padding.
  const transformation = (text) => {
    const [, mode = 'ECB', padding = 'PKCS5Padding'] = KT.str(text).split('/');
    return {
      mode: mode.toLowerCase(),
      padding: !/nopadding/i.test(padding) && !/zerobyte/i.test(padding),
    };
  };
  /// Java's `MessageDigest` and `Mac` by algorithm name, collecting what's `update`d.
  const digester = (algorithm, keyed = false) => {
    let pending = [];
    let key = null;
    const run = () => {
      const data = pending;
      pending = [];
      return keyed
        ? fromBase64(
            crypto({ op: 'hmac', algorithm, key: base64Of(key ?? []), data: base64Of(data) }),
          )
        : digestOf(algorithm, data);
    };
    return {
      init: (spec) => {
        key = bytesOf(spec?.key ?? spec);
      },
      update: (data) => {
        pending = pending.concat(bytesOf(data));
      },
      digest: (data = null) => {
        if (data !== null) pending = pending.concat(bytesOf(data));
        return run();
      },
      doFinal: (data = null) => {
        if (data !== null) pending = pending.concat(bytesOf(data));
        return run();
      },
      reset: () => {
        pending = [];
      },
    };
  };

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
  const toInt = (text, radix = 10) => {
    const trimmed = K.str(text).trim();
    const base = Number(radix) || 10;
    const digits = '0123456789abcdefghijklmnopqrstuvwxyz'.slice(0, base);
    const body = trimmed.replace(/^[+-]/, '').toLowerCase();
    if (!body || [...body].some((char) => !digits.includes(char))) return null;
    return parseInt(trimmed, base);
  };
  const toFloat = (text) => {
    const trimmed = K.str(text).trim();
    if (!/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(trimmed)) return null;
    return parseFloat(trimmed);
  };
  /// RC4 (Keiyoushi's `rc4(key, skip)`): the keystream XORed in, after `skip` of its bytes.
  const rc4 = (data, key, skip = 0) => {
    const state = Uint8Array.from({ length: 256 }, (_, index) => index);
    let j = 0;
    for (let i = 0; i < 256; i++) {
      j = (j + state[i] + key[i % key.length]) & 255;
      [state[i], state[j]] = [state[j], state[i]];
    }
    let a = 0;
    let b = 0;
    const next = () => {
      a = (a + 1) & 255;
      b = (b + state[a]) & 255;
      [state[a], state[b]] = [state[b], state[a]];
      return state[(state[a] + state[b]) & 255];
    };
    for (let count = 0; count < skip; count++) next();
    const out = new Uint8Array(data.length);
    for (let index = 0; index < data.length; index++) out[index] = data[index] ^ next();
    return out;
  };
  const STRING = {
    toString: (s) => s,
    prependIndent: (s, [indent = '    ']) =>
      s
        .split('\n')
        .map((line) => (line.trim() === '' && line.length < indent.length ? indent : indent + line))
        .join('\n'),
    replaceRange: (s, [from, to, replacement]) =>
      from && from.isRange
        ? s.slice(0, from.first) + KT.str(to) + s.slice(from.last + 1)
        : s.slice(0, from) + KT.str(replacement) + s.slice(to),
    replaceAfter: (s, [delimiter, replacement, missing = s]) => {
      const at = s.indexOf(KT.str(delimiter));
      return at < 0 ? missing : s.slice(0, at + KT.str(delimiter).length) + KT.str(replacement);
    },
    replaceAfterLast: (s, [delimiter, replacement, missing = s]) => {
      const at = s.lastIndexOf(KT.str(delimiter));
      return at < 0 ? missing : s.slice(0, at + KT.str(delimiter).length) + KT.str(replacement);
    },
    replaceBefore: (s, [delimiter, replacement, missing = s]) => {
      const at = s.indexOf(KT.str(delimiter));
      return at < 0 ? missing : KT.str(replacement) + s.slice(at);
    },
    replaceBeforeLast: (s, [delimiter, replacement, missing = s]) => {
      const at = s.lastIndexOf(KT.str(delimiter));
      return at < 0 ? missing : KT.str(replacement) + s.slice(at);
    },
    uppercaseChar: (s) => s.toUpperCase(),
    lowercaseChar: (s) => s.toLowerCase(),
    titlecaseChar: (s) => s.toUpperCase(),
    codePointAt: (s, [index]) => s.codePointAt(index),
    encodeUtf8: (s) => byteString(K.bin.utf8(s)),
    decodeHexToByteString: (s) => byteString(u8Of(fromHex(s))),
    toBigDecimal: (s) => Number(s),
    toBigInteger: (s, [radix = 10]) => bigIntOf(s, radix),
    toBigIntegerOrNull: (s, [radix = 10]) => {
      try {
        return bigIntOf(s, radix);
      } catch {
        return null;
      }
    },
    toULong: (s) => Number(s),
    subSequence: (s, [start, end = s.length]) => s.slice(start, end),
    toBigDecimalOrNull: (s) => toFloat(s),
    toUInt: (s, [radix]) => KT.nn(toInt(s, radix)) >>> 0,
    toUIntOrNull: (s, [radix]) => {
      const value = toInt(s, radix);
      return value === null || value < 0 || value > 4294967295 ? null : value;
    },
    lineSequence: (s) => s.split(/\r?\n/),
    isISOControl: (s) => /^[\u0000-\u001f\u007f-\u009f]$/.test(s),
    random: (s) => (s.length ? s[Math.floor(Math.random() * s.length)] : noSuch('empty text')),
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
    // `toInt(16)` too.
    toInt: (s, [radix]) => KT.nn(toInt(s, radix)),
    toIntOrNull: (s, [radix]) => toInt(s, radix),
    toLong: (s, [radix]) => KT.nn(toInt(s, radix)),
    toLongOrNull: (s, [radix]) => toInt(s, radix),
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
    parseAs: async (s, _args, lambda, _named, type) =>
      decode(JSON.parse(lambda ? KT.str(await lambda(s)) : s), type),
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
    // Bytes: UTF-8 (or Latin-1 when the charset says so).
    encodeToByteArray: (s) => utf8Encode(s),
    toByteArray: (s, [charset]) => bytesOf(s, charset),
    decodeToString: (s) => s,
    // Keiyoushi's `keiyoushi.utils` byte helpers on text.
    decodeHex: (s) => fromHex(s),
    md5: (s) => digestOf('md5', utf8Encode(s)),
    sha1: (s) => digestOf('sha1', utf8Encode(s)),
    sha256: (s) => digestOf('sha256', utf8Encode(s)),
    sha512: (s) => digestOf('sha512', utf8Encode(s)),
    hmacSha256: (s, [key]) =>
      fromBase64(
        crypto({
          op: 'hmac',
          algorithm: 'sha256',
          key: base64Of(key),
          data: base64Of(utf8Encode(s)),
        }),
      ),
    // Keiyoushi's `cryptoJsDecrypt(passphrase)`: CryptoJS's AES with a passphrase.
    cryptoJsDecrypt: (s, [passphrase]) => K.decrypt({ data: s, password: KT.str(passphrase) }),
    decodeBase64: (s) => fromBase64(s),
    // Descriptions are shown as plain text: there's nothing to escape.
    escapeMarkdown: (s) => s,
    // `findAnyOf(strings)`: the first of them found, as (index, string).
    findAnyOf: (s, [strings, startIndex = 0, ignoreCase = false], _lambda, named_) => {
      const fold = named_?.ignoreCase ?? ignoreCase;
      const text = fold ? s.toLowerCase() : s;
      let best = null;
      for (const candidate of KT.iter(strings)) {
        const needle = fold ? KT.str(candidate).toLowerCase() : KT.str(candidate);
        const at = text.indexOf(needle, named_?.startIndex ?? startIndex);
        if (at >= 0 && (best === null || at < best[0])) best = [at, KT.str(candidate)];
      }
      return best ? KT.pair(best[0], best[1]) : null;
    },
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
    // A string's characters with their places (`text.mapIndexed { i, c -> … }`).
    mapIndexed: async (s, _args, lambda) => {
      const out = [];
      const chars = [...s];
      for (let index = 0; index < chars.length; index++)
        out.push(await lambda(index, chars[index]));
      return out;
    },
    forEachIndexed: async (s, _args, lambda) => {
      const chars = [...s];
      for (let index = 0; index < chars.length; index++) await lambda(index, chars[index]);
      return KT.unit;
    },
    withIndex: (s) => [...s].map((value, index) => ({ index, value, isIndexed: true })),
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
    // `destructured`: its groups (the match left out), as parts (`val (a, b) = …`) and a list.
    destructured: {
      isMatchGroups: true,
      groupValues: [...match].map((group) => group ?? ''),
      toList: () => [...match].slice(1).map((group) => group ?? ''),
      component1: () => match[1] ?? '',
      component2: () => match[2] ?? '',
      component3: () => match[3] ?? '',
    },
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
  /// What a value is told apart by (`distinct()`, `toSet()`): an element by its node, data by its
  /// contents, anything else by itself.
  const identities = new WeakMap();
  let identityCount = 0;
  const keyOf = (value) => {
    if (value === null || typeof value !== 'object') return value;
    if (value.h !== undefined && typeof value.select === 'function') return `\u0000node:${value.h}`;
    try {
      return JSON.stringify(value);
    } catch {
      if (!identities.has(value)) identities.set(value, ++identityCount);
      return `\u0000object:${identities.get(value)}`;
    }
  };
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
  /// A key's order: its own comparator's (a collator, `compareBy(collator) { … }`), else natural.
  const keyOrder = (key, left, right) =>
    key.order && typeof key.order.compare === 'function'
      ? key.order.compare(left, right)
      : KT.cmp(left, right);
  const comparator = (keys) => {
    const made = {
      isComparator: true,
      __class: 'Comparator',
      __classes: ['Comparator'],
      keys,
      thenBy: (select) => comparator([...keys, { select, descending: false }]),
      thenByDescending: (select) => comparator([...keys, { select, descending: true }]),
      reversed: () => comparator(keys.map((key) => ({ ...key, descending: !key.descending }))),
      compare: async (left, right) => {
        for (const key of keys) {
          const a = key.select ? await key.select(left) : left;
          const b = key.select ? await key.select(right) : right;
          const result = keyOrder(key, a, b);
          if (result) return key.descending ? -result : result;
        }
        return 0;
      },
    };
    return made;
  };
  /// Items in the order a comparator, or a `Comparator { a, b -> … }`, gives.
  const sortWith = async (items, order) => {
    if (
      typeof order === 'function' ||
      (order && !order.keys && typeof order.compare === 'function')
    ) {
      const compare = typeof order === 'function' ? order : order.compare;
      const out = [];
      for (const item of items) {
        let at = out.length;
        while (at > 0 && (await compare(out[at - 1], item)) > 0) at--;
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
        const result = keyOrder(order.keys[index], a[0][index], b[0][index]);
        if (result) return order.keys[index].descending ? -result : result;
      }
      return 0;
    });
    return keyed.map(([, item]) => item);
  };
  /// `List(size) { index -> … }`.
  /// A list of `size` items: each `init(index)` when it's a function, else `init` itself.
  const listOfSize = async (size, init) => {
    const out = [];
    for (let index = 0; index < size; index++)
      out.push(typeof init === 'function' ? await init(index) : (init ?? null));
    return out;
  };
  /// java.time's `ChronoField`s a date format reads: each one's pattern letter and the part of a
  /// date it is (what `parseDefaulting` sets).
  const CHRONO_FIELDS = {
    YEAR: ['y', 'year'],
    YEAR_OF_ERA: ['y', 'year'],
    MONTH_OF_YEAR: ['M', 'month'],
    DAY_OF_MONTH: ['d', 'day'],
    HOUR_OF_DAY: ['H', 'hour'],
    CLOCK_HOUR_OF_AMPM: ['h', 'hour'],
    HOUR_OF_AMPM: ['h', 'hour'],
    MINUTE_OF_HOUR: ['m', 'minute'],
    SECOND_OF_MINUTE: ['s', 'second'],
    NANO_OF_SECOND: ['S', 'ms'],
    MILLI_OF_SECOND: ['S', 'ms'],
    AMPM_OF_DAY: ['a', 'ampm'],
    DAY_OF_WEEK: ['E', 'dayOfWeek'],
    DAY_OF_YEAR: ['D', 'dayOfYear'],
    EPOCH_DAY: ['', 'epochDay'],
  };
  /// `DateTimeFormatterBuilder()…toFormatter()`: its pattern pieces, as one date format, with
  /// optional sections, the values a text may leave out and month names of its own.
  const dateFormatterBuilder = () => {
    let pattern = '';
    const defaults = {};
    let monthNames = null;
    const letterOf = (field) => (CHRONO_FIELDS[KT.str(field)] ?? ['', ''])[0];
    const builder = {
      appendPattern: (text) => ((pattern += KT.str(text)), builder),
      appendLiteral: (text) => ((pattern += `'${KT.str(text).replace(/'/g, "''")}'`), builder),
      // `appendValue(ChronoField.YEAR, 4)`, `appendValue(DAY_OF_MONTH, 1, 2, SignStyle…)`.
      appendValue: (field, width = 1) => {
        const letter = letterOf(field);
        if (letter)
          pattern += letter.repeat(letter === 'y' ? (width === 2 ? 2 : 4) : Math.max(1, width));
        return builder;
      },
      // `appendFraction(NANO_OF_SECOND, 0, 9, true)`: a fraction of a second, with its point.
      appendFraction: (_field, min = 0, _max = 9, point = true) => {
        const piece = `${point ? '.' : ''}S`;
        pattern += Number(min) > 0 ? piece : `[${piece}]`;
        return builder;
      },
      // `appendText(MONTH_OF_YEAR, mapOf(1L to "январь", …))`: names, as the map gives them.
      appendText: (field, names = null) => {
        const letter = letterOf(field);
        if (letter === 'M' && names instanceof Map) {
          monthNames = [];
          for (const [number, name] of names) monthNames[Number(number) - 1] = KT.str(name);
        }
        if (letter) pattern += letter === 'M' ? 'MMMM' : letter.repeat(4);
        return builder;
      },
      parseCaseInsensitive: () => builder,
      parseCaseSensitive: () => builder,
      parseLenient: () => builder,
      parseStrict: () => builder,
      optionalStart: () => ((pattern += '['), builder),
      optionalEnd: () => ((pattern += ']'), builder),
      parseDefaulting: (field, value) => {
        const part = (CHRONO_FIELDS[KT.str(field)] ?? ['', ''])[1];
        if (part === 'month') defaults.month = Number(value) - 1;
        else if (['year', 'day', 'hour', 'minute', 'second'].includes(part))
          defaults[part] = Number(value);
        return builder;
      },
      toFormatter: (locale) => {
        const format = K.dateFormat(pattern, locale);
        format.defaults = { ...defaults };
        format.monthNames = monthNames;
        return format;
      },
    };
    return builder;
  };
  /// `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`'s patterns, as English writes them.
  const LOCALIZED_DATES = {
    FULL: 'MMMM d, yyyy',
    LONG: 'MMMM d, yyyy',
    MEDIUM: 'MMM d, yyyy',
    SHORT: 'M/d/yy',
  };
  /// java.time's ISO formats (`ISO_DATE_TIME`, `ISO_OFFSET_DATE_TIME`, `ISO_INSTANT`): ISO 8601
  /// with or without a fraction, an offset, or a zone's name in brackets.
  const isoFormat = (pattern, offset = 0) => {
    const format = K.dateFormat(pattern).withOffset(offset);
    format.read = (text) => {
      if (text === null || text === undefined) return null;
      const plain = KT.str(text)
        .trim()
        .replace(/\[[^\]]*\]$/, '');
      const ms = isoInstant(plain);
      if (ms === null) return null;
      // A time without an offset is the format's zone's wall clock.
      const zoned = /([Zz]|[+-]\d{2}(:?\d{2})?)$/.test(plain.slice(10));
      return zoned ? ms : ms - format.offsetMinutes * 60000;
    };
    format.withZone = (zone) => isoFormat(pattern, offsetOf(zone));
    return format;
  };
  const LIST = {
    size: (l) => l.length,
    // Jsoup's `Elements` edits, on each element.
    prepend: (l, [html]) => (l.forEach((element) => element.prepend(html)), l),
    append: (l, [html]) => (l.forEach((element) => element.append(html)), l),
    before: (l, [html]) => (l.forEach((element) => element.before(html)), l),
    after: (l, [html]) => (l.forEach((element) => element.after(html)), l),
    // Removals from either end, and what a list keeps or drops while a test holds.
    removeLast: (l) => (l.length ? l.pop() : noSuch('the list is empty')),
    removeFirst: (l) => (l.length ? l.shift() : noSuch('the list is empty')),
    removeLastOrNull: (l) => (l.length ? l.pop() : null),
    removeFirstOrNull: (l) => (l.length ? l.shift() : null),
    retainAll: async (l, args, lambda) => {
      const keep = lambda
        ? await mapAll(l, lambda)
        : l.map((item) => KT.iter(args[0]).some((other) => KT.eq(item, other)));
      let changed = false;
      for (let index = l.length - 1; index >= 0; index--)
        if (!KT.truthy(keep[index])) {
          l.splice(index, 1);
          changed = true;
        }
      return changed;
    },
    takeLastWhile: async (l, _args, lambda) => {
      let at = l.length;
      while (at > 0 && KT.truthy(await lambda(l[at - 1]))) at--;
      return l.slice(at);
    },
    dropLastWhile: async (l, _args, lambda) => {
      let at = l.length;
      while (at > 0 && KT.truthy(await lambda(l[at - 1]))) at--;
      return l.slice(0, at);
    },
    onEachIndexed: async (l, _args, lambda) => {
      for (let index = 0; index < l.length; index++) await lambda(index, l[index]);
      return l;
    },
    mapIndexedTo: async (l, [destination], lambda) => {
      for (let index = 0; index < l.length; index++)
        destination.push(await lambda(index, l[index]));
      return destination;
    },
    flatMapIndexed: async (l, _args, lambda) =>
      (await mapAll(l, lambda, true)).flatMap((value) => KT.iter(value)),
    flatMapTo: async (l, [destination], lambda) => {
      for (const item of l) destination.push(...KT.iter(await lambda(item)));
      return destination;
    },
    filterNotNullTo: (l, [destination]) => {
      for (const item of l) if (item !== null && item !== undefined) destination.push(item);
      return destination;
    },
    // `groupingBy { … }.eachCount()`.
    groupingBy: async (l, _args, lambda) => {
      const keys = await mapAll(l, lambda);
      return {
        __class: 'Grouping',
        __classes: ['Grouping'],
        eachCount: () => {
          const counts = new Map();
          for (const key of keys) counts.set(key, (counts.get(key) ?? 0) + 1);
          return counts;
        },
        keyOf: (item) => keys[l.indexOf(item)],
      };
    },
    iterator: (l) => {
      let at = 0;
      return {
        __class: 'Iterator',
        __classes: ['Iterator'],
        hasNext: () => at < l.length,
        next: () => (at < l.length ? l[at++] : noSuch('the list has no more items')),
        remove: () => {
          at -= 1;
          l.splice(at, 1);
        },
      };
    },
    // Keiyoushi's byte readers (`keiyoushi.utils.Binary`) and RC4, okio's `ByteString`.
    readUShortLittleEndian: (l, [offset]) => (l[offset] & 255) | ((l[offset + 1] & 255) << 8),
    readUShortBigEndian: (l, [offset]) => ((l[offset] & 255) << 8) | (l[offset + 1] & 255),
    readUIntLittleEndian: (l, [offset]) =>
      ((l[offset] & 255) | ((l[offset + 1] & 255) << 8) | ((l[offset + 2] & 255) << 16)) +
      (l[offset + 3] & 255) * 16777216,
    readLongLittleEndian: (l, [offset]) => {
      let value = 0;
      for (let index = 7; index >= 0; index--) value = value * 256 + (l[offset + index] & 255);
      return value;
    },
    rc4: (l, [key, skip = 0]) => listOfBytes(rc4(u8Of(l), u8Of(key), skip)),
    toByteString: (l) => byteString(u8Of(l)),
    // Keiyoushi's `ByteArray.inflate(nowrap)`.
    inflate: (l, [nowrap = false]) => listOfBytes(inflateBytes(l, nowrap ? 'raw' : 'zlib')),
    copyInto: (l, [destination, offset = 0, start = 0, end = l.length]) => {
      for (let index = start; index < end; index++) destination[offset + index - start] = l[index];
      return destination;
    },
    // A JSON array's `parseAs<List<T>>()`.
    parseAs: (l, _args, _lambda, _named, type) => decode(l, type),
    containsAll: (l, [other]) => KT.iter(other).every((item) => l.some((own) => KT.eq(own, item))),
    // A `ByteArray`'s own calls, and Keiyoushi's byte helpers on it.
    decodeToString: (l) => utf8Decode(l),
    // A body of bytes (sent as they are; `text` for a multipart form's part).
    toRequestBody: (l, [type = 'application/octet-stream']) => ({
      isBody: true,
      bytes: u8Of(l),
      text: textOf(l),
      type: KT.str(type),
    }),
    toResponseBody: (l, [type]) => bytesBody(async () => u8Of(l), type ?? null),
    inputStream: (l) => inputStream(u8Of(l)),
    byteInputStream: (l) => inputStream(u8Of(l)),
    // `bytes.toString(Charsets.UTF_8)` is the text they spell; a list's own is Kotlin's.
    toString: (l, [charset]) =>
      charset !== undefined && l.every((item) => typeof item === 'number')
        ? textOf(l, charset)
        : KT.str(l),
    copyOfRange: (l, [from, to]) => l.slice(from, to),
    copyOf: (l, [size]) =>
      size === undefined ? [...l] : Array.from({ length: size }, (_, index) => l[index] ?? 0),
    reversedArray: (l) => [...l].reverse(),
    sliceArray: (l, [range]) =>
      range && range.isRange ? KT.iter(range).map((index) => l[index]) : l.slice(),
    contentEquals: (l, [other]) =>
      Array.isArray(other) &&
      l.length === other.length &&
      l.every((item, at) => item === other[at]),
    toHexString: (l) => hexOf(l),
    md5: (l) => digestOf('md5', l),
    sha1: (l) => digestOf('sha1', l),
    sha256: (l) => digestOf('sha256', l),
    sha512: (l) => digestOf('sha512', l),
    hmacSha256: (l, [key]) =>
      fromBase64(
        crypto({ op: 'hmac', algorithm: 'sha256', key: base64Of(key), data: base64Of(l) }),
      ),
    aesDecrypt: (l, [cipher]) => aesCipher({ ...cipher, encrypt: false }).doFinal(l),
    aesEncrypt: (l, [cipher]) => aesCipher({ ...cipher, encrypt: true }).doFinal(l),
    xor: (l, [key]) => {
      const keys = Array.isArray(key) ? key : [key];
      return l.map((byte, at) => (byte ^ keys[at % keys.length]) & 255);
    },
    xorInPlace: (l, [key]) => {
      const keys = Array.isArray(key) ? key : [key];
      for (let at = 0; at < l.length; at++) l[at] = (l[at] ^ keys[at % keys.length]) & 255;
      return l;
    },
    cryptoJsDecrypt: (l, [passphrase]) =>
      utf8Encode(K.decrypt({ data: base64Of(l), password: KT.str(passphrase) })),
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
    // `groupBy({ key }, { value })` too.
    groupBy: async (l, args, lambda) => {
      const [keyOf, valueOf] =
        args.length >= 2 && typeof args[1] === 'function'
          ? [args[0], args[1]]
          : [lambda ?? args[0], args.length === 1 && lambda && lambda !== args[0] ? args[0] : null];
      const groups = new Map();
      for (const item of l) {
        const key = await keyOf(item);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(valueOf ? await valueOf(item) : item);
      }
      return groups;
    },
    associate: async (l, _args, lambda) =>
      new Map((await mapAll(l, lambda)).map((pair) => [pair.first, pair.second])),
    // `associateBy({ key }, { value })` too.
    associateBy: async (l, args, lambda) => {
      const [keyOf, valueOf] =
        args.length >= 2 && typeof args[1] === 'function'
          ? [args[0], args[1]]
          : [lambda ?? args[0], null];
      const map = new Map();
      for (const item of l) map.set(await keyOf(item), valueOf ? await valueOf(item) : item);
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
    foldIndexed: async (l, [initial], lambda) => {
      let total = initial;
      for (let index = 0; index < l.length; index++) total = await lambda(index, total, l[index]);
      return total;
    },
    // Jsoup's `Elements.hasText()`: whether any of them has text.
    hasText: (l) =>
      l.some((element) => element && typeof element.hasText === 'function' && element.hasText()),
    addLast: (l, [item]) => {
      l.push(item);
      return KT.unit;
    },
    addFirst: (l, [item]) => {
      l.unshift(item);
      return KT.unit;
    },
    // Keiyoushi's `ByteArray.readIntLittleEndian(offset)` and the like.
    readIntLittleEndian: (l, [at]) =>
      (l[at] & 255) |
      ((l[at + 1] & 255) << 8) |
      ((l[at + 2] & 255) << 16) |
      ((l[at + 3] & 255) << 24),
    readIntBigEndian: (l, [at]) =>
      ((l[at] & 255) << 24) |
      ((l[at + 1] & 255) << 16) |
      ((l[at + 2] & 255) << 8) |
      (l[at + 3] & 255),
    writeIntLittleEndian: (l, [at, value]) => {
      for (let index = 0; index < 4; index++) l[at + index] = ((value >> (8 * index)) << 24) >> 24;
      return KT.unit;
    },
    writeIntBigEndian: (l, [at, value]) => {
      for (let index = 0; index < 4; index++)
        l[at + index] = ((value >> (8 * (3 - index))) << 24) >> 24;
      return KT.unit;
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
      if (l.isSet && l.some((other) => KT.eq(other, args[args.length - 1]))) return false;
      if (args.length === 2) l.splice(args[0], 0, args[1]);
      else l.push(args[0]);
      return true;
    },
    addAll: (l, [items]) => {
      const list = [...KT.iter(items)];
      if (l.isSet) {
        let added = false;
        for (const item of list) added = LIST.add(l, [item]) || added;
        return added;
      }
      l.push(...list);
      return true;
    },
    randomOrNull: (l) => (l.length ? l[Math.floor(Math.random() * l.length)] : null),
    toHashSet: (l) => LIST.distinct(l),
    asList: (l) => l,
    contentToString: (l) => `[${l.map((item) => KT.str(item)).join(', ')}]`,
    minWith: async (l, [order]) => {
      if (!l.length) return noSuch('the list is empty');
      return (await sortWith(l, order))[0];
    },
    maxWith: async (l, [order]) => {
      if (!l.length) return noSuch('the list is empty');
      const sorted = await sortWith(l, order);
      return sorted[sorted.length - 1];
    },
    runningFold: async (l, [initial], lambda) => {
      const out = [initial];
      let acc = initial;
      for (const item of l) out.push((acc = await lambda(acc, item)));
      return out;
    },
    // `MutableList.reverse()`: in place.
    reverse: (l) => {
      l.reverse();
      return KT.unit;
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
    // Kotlin's `{a=1, b=2}`.
    toString: (m) =>
      `{${[...m].map(([key, value]) => `${KT.str(key)}=${KT.str(value)}`).join(', ')}}`,
    get: (m, [key]) => (m.has(key) ? m.get(key) : null),
    toSortedMap: (m) => new Map([...m.entries()].sort(([left], [right]) => KT.cmp(left, right))),
    getOrDefault: (m, [key, fallback]) => (m.has(key) ? m.get(key) : fallback),
    getOrElse: async (m, [key], lambda) => (m.has(key) ? m.get(key) : lambda()),
    getOrPut: async (m, [key], lambda) => {
      if (!m.has(key)) m.set(key, await lambda());
      return m.get(key);
    },
    containsKey: (m, [key]) => m.has(key),
    getValue: async (m, [key]) =>
      m.has(key) ? m.get(key) : m.defaultOf ? m.defaultOf(key) : noSuch(`no key ${KT.str(key)}`),
    // `withDefault { key -> … }`: what `getValue` answers for a key it lacks.
    withDefault: (m, _args, lambda) => {
      Object.defineProperty(m, 'defaultOf', {
        value: lambda,
        enumerable: false,
        configurable: true,
      });
      return m;
    },
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
    filterNot: async (m, _args, lambda) => {
      const out = new Map();
      for (const entry of KT.iter(m))
        if (!KT.truthy(await lambda(entry))) out.set(entry.key, entry.value);
      return out;
    },
    filterKeys: async (m, _args, lambda) => {
      const out = new Map();
      for (const [key, value] of m) if (KT.truthy(await lambda(key))) out.set(key, value);
      return out;
    },
    filterValues: async (m, _args, lambda) => {
      const out = new Map();
      for (const [key, value] of m) if (KT.truthy(await lambda(value))) out.set(key, value);
      return out;
    },
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
    // A locale's tag and names.
    toLanguageTag: (o) => (o.country ? `${o.language}-${o.country}` : KT.str(o.language)),
    getDisplayLanguage: (o) => LANGUAGE_NAMES[o.language] ?? KT.str(o.language),
    getDisplayName: (o) => LANGUAGE_NAMES[o.language] ?? KT.str(o.language),
    // A `JsonObject`'s text is its JSON.
    toString: (o) => JSON.stringify(o),
    // Keiyoushi's `JsonElement.parseAs<T>()`: the element decoded as the type.
    parseAs: (o, _args, _lambda, _named, type) => decode(o, type),
    // Keiyoushi's `Element.toMarkdown(keepNewlines)`: its text with paragraphs, plain.
    toMarkdown: (o, [keepNewlines = false], _lambda, named_) =>
      htmlText(o, named_?.keepNewlines ?? keepNewlines),
    get: (o, [key]) => (Object.prototype.hasOwnProperty.call(o, key) ? o[key] : null),
    getOrDefault: (o, [key, fallback]) => o[key] ?? fallback,
    toSortedMap: (o) => new Map(Object.entries(o).sort(([left], [right]) => KT.cmp(left, right))),
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
    getArray: (o, [key]) => (Array.isArray(o[key]) ? o[key] : noSuch(`${key} isn't a JSON array`)),
    getArrayOrNull: (o, [key]) => (Array.isArray(o[key]) ? o[key] : null),
    getObject: (o, [key]) =>
      o[key] && typeof o[key] === 'object' && !Array.isArray(o[key])
        ? o[key]
        : noSuch(`${key} isn't a JSON object`),
    getObjectOrNull: (o, [key]) =>
      o[key] && typeof o[key] === 'object' && !Array.isArray(o[key]) ? o[key] : null,
    toMap: (o) => new Map(Object.entries(o)),
    toMutableMap: (o) => new Map(Object.entries(o)),
  };
  /// What a map has, on a JSON object (`jsonObject.mapValues { … }`): read as a map.
  const objectAsMap = (name) =>
    MAP[name] ? (o, ...rest) => MAP[name](new Map(Object.entries(o)), ...rest) : null;

  /// Java's `BigInteger` as JavaScript's `BigInt`: text in a radix, bytes (two's complement, or a
  /// sign and a magnitude as `BigInteger(1, bytes)` reads them).
  const bigIntOf = (value, radix = 10) => {
    if (typeof value === 'bigint') return value;
    if (typeof value === 'number') return BigInt(Math.trunc(value));
    const text = KT.str(value).trim();
    const base = Number(radix);
    const negative = text.startsWith('-');
    const digits = negative || text.startsWith('+') ? text.slice(1) : text;
    if (!digits || !/^[0-9a-z]+$/i.test(digits))
      throw new KotlinError(`For input string: "${text}"`, 'NumberFormatException');
    let out = 0n;
    for (const digit of digits.toLowerCase()) {
      const at = parseInt(digit, 36);
      if (Number.isNaN(at) || at >= base)
        throw new KotlinError(`For input string: "${text}"`, 'NumberFormatException');
      out = out * BigInt(base) + BigInt(at);
    }
    return negative ? -out : out;
  };
  const bigIntOfBytes = (bytes, signed = true) => {
    const all = Array.from(u8Of(bytes));
    let out = 0n;
    for (const byte of all) out = (out << 8n) | BigInt(byte);
    return signed && all.length && all[0] & 0x80 ? out - (1n << BigInt(all.length * 8)) : out;
  };
  /// `toByteArray()`: the fewest bytes of two's complement that hold the value and its sign.
  const bigIntBytes = (value) => {
    let count = 1;
    while (value < -(1n << BigInt(count * 8 - 1)) || value >= 1n << BigInt(count * 8 - 1))
      count += 1;
    const hex = BigInt.asUintN(count * 8, value)
      .toString(16)
      .padStart(count * 2, '0');
    return hex.match(/../g).map((pair) => parseInt(pair, 16));
  };
  const modPow = (base, exponent, modulus) => {
    let result = 1n;
    let at = ((base % modulus) + modulus) % modulus;
    for (let rest = exponent; rest > 0n; rest >>= 1n) {
      if (rest & 1n) result = (result * at) % modulus;
      at = (at * at) % modulus;
    }
    return result;
  };
  const modInverse = (value, modulus) => {
    let [a, m] = [((value % modulus) + modulus) % modulus, modulus];
    let [x, last] = [0n, 1n];
    while (m !== 0n) {
      const quotient = a / m;
      [a, m] = [m, a - quotient * m];
      [last, x] = [x, last - quotient * x];
    }
    if (a !== 1n) throw new KotlinError('BigInteger not invertible.', 'ArithmeticException');
    return ((last % modulus) + modulus) % modulus;
  };
  const BIGINT = {
    plus: (b, [other]) => b + bigIntOf(other),
    add: (b, [other]) => b + bigIntOf(other),
    minus: (b, [other]) => b - bigIntOf(other),
    subtract: (b, [other]) => b - bigIntOf(other),
    times: (b, [other]) => b * bigIntOf(other),
    multiply: (b, [other]) => b * bigIntOf(other),
    div: (b, [other]) => b / bigIntOf(other),
    divide: (b, [other]) => b / bigIntOf(other),
    rem: (b, [other]) => b % bigIntOf(other),
    remainder: (b, [other]) => b % bigIntOf(other),
    mod: (b, [other]) => {
      const modulus = bigIntOf(other);
      return ((b % modulus) + modulus) % modulus;
    },
    modPow: (b, [exponent, modulus]) => modPow(b, bigIntOf(exponent), bigIntOf(modulus)),
    modInverse: (b, [modulus]) => modInverse(b, bigIntOf(modulus)),
    pow: (b, [exponent]) => b ** BigInt(exponent),
    gcd: (b, [other]) => {
      let [x, y] = [b < 0n ? -b : b, bigIntOf(other)];
      if (y < 0n) y = -y;
      while (y) [x, y] = [y, x % y];
      return x;
    },
    xor: (b, [other]) => b ^ bigIntOf(other),
    and: (b, [other]) => b & bigIntOf(other),
    or: (b, [other]) => b | bigIntOf(other),
    not: (b) => ~b,
    inv: (b) => ~b,
    andNot: (b, [other]) => b & ~bigIntOf(other),
    shl: (b, [bits]) => b << BigInt(bits),
    shiftLeft: (b, [bits]) => b << BigInt(bits),
    shr: (b, [bits]) => b >> BigInt(bits),
    shiftRight: (b, [bits]) => b >> BigInt(bits),
    testBit: (b, [bit]) => ((b >> BigInt(bit)) & 1n) === 1n,
    setBit: (b, [bit]) => b | (1n << BigInt(bit)),
    bitLength: (b) => (b < 0n ? ~b : b).toString(2).replace(/^0$/, '').length,
    bitCount: (b) => [...(b < 0n ? ~b : b).toString(2)].filter((digit) => digit === '1').length,
    negate: (b) => -b,
    unaryMinus: (b) => -b,
    abs: (b) => (b < 0n ? -b : b),
    signum: (b) => (b > 0n ? 1 : b < 0n ? -1 : 0),
    compareTo: (b, [other]) => {
      const value = bigIntOf(other);
      return b < value ? -1 : b > value ? 1 : 0;
    },
    equals: (b, [other]) => typeof other === 'bigint' && b === other,
    max: (b, [other]) => (b >= bigIntOf(other) ? b : bigIntOf(other)),
    min: (b, [other]) => (b <= bigIntOf(other) ? b : bigIntOf(other)),
    toInt: (b) => Number(BigInt.asIntN(32, b)),
    intValue: (b) => Number(BigInt.asIntN(32, b)),
    toLong: (b) => Number(BigInt.asIntN(64, b)),
    longValue: (b) => Number(BigInt.asIntN(64, b)),
    toDouble: (b) => Number(b),
    doubleValue: (b) => Number(b),
    toBigInteger: (b) => b,
    toByteArray: (b) => bigIntBytes(b),
    toString: (b, [radix = 10]) => b.toString(Number(radix)),
    hashCode: (b) => Number(BigInt.asIntN(32, b)),
  };
  const NUMBER = {
    // `BigDecimal`s are numbers here.
    stripTrailingZeros: (n) => n,
    setScale: (n, [scale = 0, mode = 'HALF_UP']) => {
      const factor = 10 ** Number(scale);
      const round =
        {
          DOWN: Math.trunc,
          FLOOR: Math.floor,
          CEILING: Math.ceil,
          UP: (x) => Math.sign(x) * Math.ceil(Math.abs(x)),
        }[KT.str(mode)] ?? Math.round;
      return round(n * factor) / factor;
    },
    toPlainString: (n) => String(n),
    mod: (n, [other]) => ((n % other) + other) % other,
    toBigDecimal: (n) => n,
    toBigInteger: (n) => BigInt(Math.trunc(n)),
    toULong: (n) => (n < 0 ? n + 18446744073709551616 : n),
    toUByte: (n) => n & 255,
    toUInt: (n) => Math.trunc(n) >>> 0,
    downTo: (n, [to]) => KT.range(n, to, -1),
    until: (n, [to]) => KT.range(n, to - 1),
    rangeTo: (n, [to]) => KT.range(n, to),
    signum: (n) => Math.sign(n),
    // A character's code read as one (`Char::isISOControl`).
    isISOControl: (n) => n <= 31 || (n >= 127 && n <= 159),
    toULong: (n) => (n < 0 ? n + 18446744073709551616 : Math.trunc(n)),
    toUShort: (n) => n & 65535,
    multipliedBy: (n, [other]) => n * Number(other),
    toString: (n, [radix]) => (radix ? n.toString(radix) : K.str(n)),
    // A Long's `toInt()` keeps its low 32 bits, as Kotlin's does (a Double's is cut short).
    toInt: (n) =>
      Number.isInteger(n) && !isInt32(n) && Math.abs(n) <= Number.MAX_SAFE_INTEGER
        ? Number(BigInt.asIntN(32, BigInt(n)))
        : Math.trunc(n),
    toLong: (n) => Math.trunc(n),
    toFloat: (n) => n,
    toDouble: (n) => n,
    // Bits as functions (`x.shl(3)`, `x.inv()`).
    and: (n, [other]) => KT.and(n, other),
    or: (n, [other]) => KT.or(n, other),
    xor: (n, [other]) => KT.xor(n, other),
    shl: (n, [bits]) => KT.shl(n, bits),
    shr: (n, [bits]) => KT.shr(n, bits),
    ushr: (n, [bits]) => KT.ushr(n, bits),
    inv: (n) => KT.inv(n),
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
    // A comparator's `then(other)` (not a property: an object with `then` would be awaited as a
    // promise): this order, then the other's for ties.
    then:
      (value, [other]) =>
      async (left, right) =>
        (await compareWith(value, left, right)) || compareWith(other, left, right),
    thenComparator:
      (value, [other], lambda) =>
      async (left, right) =>
        (await compareWith(value, left, right)) || (lambda ?? other)(left, right),
    thenDescending:
      (value, [other]) =>
      async (left, right) =>
        (await compareWith(value, left, right)) || -(await compareWith(other, left, right)),
    // kotlinx.datetime's and java.time's instants are the same here.
    toJavaInstant: (value) => value,
    toKotlinInstant: (value) => value,
    // A comparator's `compare(a, b)`: a `Comparator { a, b -> … }`, a collator, `compareBy`.
    compare: async (value, [left, right]) => compareWith(value, left, right),
    // Keiyoushi's protobuf writers, on an instance of a class the source declares.
    toRequestBodyProto: (value, [type = 'application/protobuf']) => ({
      isBody: true,
      bytes: protoEncode(value),
      text: '',
      type: KT.str(type),
    }),
    encodeProto: (value) => listOfBytes(protoEncode(value)),
    encodeProtoBase64: (value) => K.bin.toBase64(protoEncode(value)),
    toString: (value) => KT.str(value),
    equals: (value, [other]) => KT.eq(value, other),
    hashCode: (value) => {
      let hash = 0;
      for (const char of KT.str(value)) hash = (Math.imul(31, hash) + char.charCodeAt(0)) | 0;
      return hash;
    },
    use: async (value, _args, lambda) => lambda(value),
    // `builder.apply(params)`, `value.run(block)` with a function passed (a lambda with a
    // receiver takes it first).
    apply: async (value, args, lambda) => {
      await (lambda ?? args[0])(value);
      return value;
    },
    run: async (value, args, lambda) => (lambda ?? args[0])(value),
    closeQuietly: () => KT.unit,
    printStackTrace: () => KT.unit,
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
    // Keiyoushi's `obj` and `array` (`keiyoushi.utils`).
    obj: (value) =>
      value && typeof value === 'object' && !Array.isArray(value)
        ? value
        : KT.raise('not a JSON object'),
    array: (value) => (Array.isArray(value) ? value : KT.raise('not a JSON array')),
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
    // Keiyoushi's `stringOrNull`: a primitive's text; null for null and for objects and arrays.
    stringOrNull: (value) => (value === null || typeof value === 'object' ? null : KT.str(value)),
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
  /// A DTO property's other JSON names (`@JsonNames`), by property name.
  let aliases = {};
  /// Keiyoushi's `assets/i18n/messages_<lang>.properties`, by `<lang>` (`pt_br`).
  let messages = {};
  /// The extension's resources (`assets/…`), as base64, for `javaClass.getResource(…)`.
  let assets = {};
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
      // `intl.collator`: text ordered as the chosen language orders it.
      collator: collator({ language: KT.str(chosen).split(/[-_]/)[0] }),
      get,
      format: (key, ...args) => K.format(get(key), ...args),
      languageDisplayName: (code) => {
        const language = KT.str(code).split(/[-_]/)[0].toLowerCase();
        return LANGUAGE_NAMES[language] ?? KT.str(code);
      },
    };
  };
  /// A DTO's property by its Kotlin name, or the JSON name it's read by.
  const field = (object, name) => {
    if (Object.prototype.hasOwnProperty.call(object, name)) return object[name];
    const serial = renames[name];
    if (serial !== undefined && Object.prototype.hasOwnProperty.call(object, serial))
      return object[serial];
    for (const other of aliases[name] ?? [])
      if (Object.prototype.hasOwnProperty.call(object, other)) return object[other];
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
  /// What `JsonNull` (a `JsonPrimitive`) answers.
  const JSON_NULL_PROPS = {
    contentOrNull: null,
    content: 'null',
    isString: false,
    stringOrNull: null,
    intOrNull: null,
    longOrNull: null,
    doubleOrNull: null,
    floatOrNull: null,
    booleanOrNull: null,
    jsonPrimitive: null,
  };
  KT.p = async (value, name) => {
    if (value === null && Object.prototype.hasOwnProperty.call(JSON_NULL_PROPS, name))
      return JSON_NULL_PROPS[name];
    if (value === null || value === undefined)
      throw new KotlinError(`read ${name} of null`, 'NullPointerException');
    const kind = kindOf(value);
    if (kind === 'string' || kind === 'number' || kind === 'boolean') {
      if (name === 'length' && kind === 'string') return value.length;
      if (JSON_PROPS[name]) return JSON_PROPS[name](value);
      if (kind === 'number' && NUMBER[name]) return NUMBER[name](value, []);
      if (kind === 'string' && name === 'lastIndex') return value.length - 1;
      // A `Char`'s code (a one-letter string here).
      if (kind === 'string' && name === 'code') return value.length ? value.charCodeAt(0) : 0;
      if (kind === 'string' && name === 'category') return charCategory(value);
      if (kind === 'string' && name === 'indices') return KT.range(0, value.length - 1);
      throw new KotlinError(`a value has no ${name}`);
    }
    if (kind === 'bigint') {
      if (BIGINT[name]) return BIGINT[name](value, []);
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
      // The source itself (a context parameter's `source.headers`).
      if (value.isSource) return value.get(name);
      if (value.isGetterHost && value.getters && value.getters[name])
        return value.getters[name](value);
      if (value.isStringBuilder && (name === 'lastIndex' || name === 'indices'))
        return KT.p(value.text, name);
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
      const getterName = `get${name.charAt(0).toUpperCase()}${name.slice(1)}`;
      const getter = value[getterName];
      if (typeof getter === 'function') return getter.call(value);
      // A plain value's Java getter from the runtime's table (`locale.displayLanguage`).
      if (value.__class === undefined && OBJECT[getterName]) return OBJECT[getterName](value, []);
      if (value.__class !== undefined && !(name in value))
        throw new KotlinError(`${value.__class} has no ${name}`);
      return null;
    }
    throw new KotlinError(`a value has no ${name}`);
  };
  /// `Owner::name` on one of the runtime's objects: its own function when it has one, else a
  /// member of the first argument (`String::isNotBlank`).
  KT.ref = async (object, name, args) => {
    // `Int::toString` is the number's `toString`, not JavaScript's on the `Int` object.
    if (object && typeof object[name] === 'function' && object[name] !== Object.prototype[name])
      return KT.m(object, name, args);
    const [receiver, ...rest] = args;
    return KT.member(receiver, name, rest);
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
    try {
      value[name] = item;
    } catch (error) {
      // A `var` with a getter and a setter the converter kept the getter of: the value set
      // is kept as a field.
      if (!(error instanceof TypeError)) throw error;
      Object.defineProperty(value, name, {
        value: item,
        writable: true,
        enumerable: false,
        configurable: true,
      });
    }
  };

  /// A function passed in parentheses (`all(Char::isDigit)`, `fold(0, ::add)`): the lambda Kotlin
  /// would take trailing, when it's the only function and the last argument.
  const passedLambda = (args) => {
    const functions = args.filter((arg) => typeof arg === 'function');
    return functions.length === 1 && args[args.length - 1] === functions[0] ? functions[0] : null;
  };
  KT.m = async (receiver, name, args = [], named = null, written = null, type = null) => {
    // The runtime's own tables read the lambda however it was passed; a value's own methods get
    // the arguments as written.
    const lambda = written ?? passedLambda(args);
    if (receiver === null || receiver === undefined) {
      if (NULLABLE[name]) return NULLABLE[name](receiver, args, lambda, named, type);
      throw new KotlinError(`${name}() was called on null`, 'NullPointerException');
    }
    if (receiver.isLazySequence) return sequenceCall(receiver, name, args, lambda, named, type);
    if (hasNextJs(receiver, name)) return NEXT_JS[name](receiver, args, lambda, type);
    // Protobuf answers read as the type asked (`parseAsProto<T>()`), after a `transform` if any.
    if (type && PROTO_DECODERS.has(name)) {
      let bytes = await protoBytesOf(receiver, name);
      if (lambda) bytes = u8Of(await streamBytes(await lambda(okioBuffer(bytes))));
      return decodeProtoAs(bytes, type);
    }
    const kind = kindOf(receiver);
    if (receiver instanceof RegExp && REGEX[name]) return REGEX[name](receiver, args, lambda);
    // `response.parseAs<List<MangaDto>>()`, `json.decodeFromString<T>(text)`: as the type asked.
    if (type && DECODERS.has(name) && kind === 'object' && typeof receiver[name] === 'function') {
      const all = lambda && !args.includes(lambda) ? [...args, lambda] : args;
      return decode(await receiver[name](...all), type);
    }
    if (kind === 'elements' && typeof receiver[name] === 'function' && ELEMENTS_OWN.has(name))
      return receiver[name](...args);
    // A declared class built on a collection: its own functions first.
    if (
      (kind === 'list' || kind === 'map') &&
      receiver.__class !== undefined &&
      Object.prototype.hasOwnProperty.call(receiver, name) &&
      typeof receiver[name] === 'function'
    )
      return receiver[name](...args, ...(written ? [written] : []));
    const table = {
      string: STRING,
      list: LIST,
      elements: LIST,
      map: MAP,
      number: NUMBER,
      bigint: BIGINT,
    }[kind];
    if (table && table[name]) return table[name](receiver, args, lambda, named, type);
    // A map's entries as a list (`map.count { … }`, `map.maxByOrNull { … }`), a text's
    // characters (`text.foldIndexed(0) { … }`).
    if (kind === 'map' && LIST[name])
      return LIST[name](KT.iter(receiver), args, lambda, named, type);
    if (kind === 'string' && LIST[name] && !ANY[name])
      return LIST[name]([...receiver], args, lambda, named, type);
    // A StringBuilder is a CharSequence too (`if (isNotEmpty()) append(", ")`).
    if (receiver.isStringBuilder && typeof receiver[name] !== 'function' && STRING[name])
      return STRING[name](receiver.text, args, lambda, named, type);
    // A range is a list of its numbers (`(1..n).reversed()`, `(0 until size).map { … }`).
    if (receiver.isRange && name !== 'contains' && LIST[name])
      return LIST[name](KT.iter(receiver), args, lambda, named, type);
    // Keiyoushi's `response.aesDecrypt(cipher)` (and on a body): the answer, decrypted.
    if (
      (receiver instanceof Response || (receiver && receiver.isResponseBody)) &&
      (name === 'aesDecrypt' || name === 'aesDecryptWithPrependedIv')
    )
      return decryptAnswer(receiver, name, args, lambda, named);
    // okio's `source.cipherSource(cipher)` (Keiyoushi's `source.aesDecrypt(cipher)`): the bytes
    // left, decrypted.
    if (
      name === 'cipherSource' ||
      (name === 'aesDecrypt' && receiver && typeof receiver.u8Now === 'function')
    )
      return okioBuffer(
        args[0].doFinalU8
          ? args[0].doFinalU8(await streamBytes(receiver))
          : u8Of(await args[0].doFinal(await streamBytes(receiver))),
      );
    // OkHttp's `response.headers("Content-Type")`: that header's values.
    if (receiver instanceof Response && name === 'headers' && args.length)
      return receiver.headers.values(args[0]);
    if (kind === 'object') {
      // The source itself: its functions, then its extensions on nothing.
      if (receiver.isSource && receiver.hasFunction(name))
        return receiver.call(name, lambda && !args.includes(lambda) ? [...args, lambda] : args);
      if (receiver.isMatch && name === 'groupValues') return receiver.groupValues;
      const own = receiver[name];
      // Not JavaScript's own (a JSON object's `toString` is its JSON, below).
      if (typeof own === 'function' && own !== Object.prototype[name]) {
        const all = [...args];
        if (named) all.push({ [NAMED]: true, ...named });
        if (written) all.push(written);
        return own.call(receiver, ...all);
      }
      if (own && own.isMethod) return own.function_(receiver, args, lambda, named, type);
      if (receiver.__class === undefined && OBJECT[name])
        return OBJECT[name](receiver, args, lambda, named, type);
      if (receiver.__class === undefined && objectAsMap(name))
        return objectAsMap(name)(receiver, args, lambda, named, type);
    }
    if (kind === 'function' && name === 'invoke') return receiver(...args);
    if (ANY[name]) return ANY[name](receiver, args, lambda, named, type);
    throw new KotlinError(`Raygun's runtime has no ${name}() for this value`);
  };
  const ELEMENTS_OWN = new Set([
    'first',
    'last',
    'text',
    'textOrNull',
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
      appendCodePoint: (code) => {
        builder.text += String.fromCodePoint(Number(code));
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
      clear: () => {
        builder.text = '';
        return builder;
      },
      deleteCharAt: (at) => {
        builder.text = builder.text.slice(0, at) + builder.text.slice(at + 1);
        return builder;
      },
      delete: (start, end) => {
        builder.text = builder.text.slice(0, start) + builder.text.slice(end);
        return builder;
      },
      deleteRange: (start, end) => builder.delete(start, end),
      setCharAt: (at, char) => {
        builder.text = builder.text.slice(0, at) + KT.str(char) + builder.text.slice(at + 1);
      },
      reverse: () => {
        builder.text = [...builder.text].reverse().join('');
        return builder;
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
    return {
      isJsonBuilder: true,
      array,
      add: (value) => array.push(plain(value)),
      addAll: (values) => {
        for (const value of KT.iter(values)) array.push(plain(value));
        return true;
      },
      addJsonObject: async (lambda) => {
        const inner = KT.jsonBuilder();
        await lambda.call(null, inner);
        array.push(inner.object);
      },
      addJsonArray: async (lambda) => {
        const inner = KT.jsonArrayBuilder();
        await lambda.call(null, inner);
        array.push(inner.array);
      },
    };
  };
  KT.built = (builder) => (builder.isJsonBuilder ? (builder.object ?? builder.array) : builder);
  const plain = (value) => {
    if (value === undefined) return null;
    if (value instanceof Map)
      return Object.fromEntries([...value].map(([key, item]) => [key, plain(item)]));
    if (value instanceof Set) return [...value].map(plain);
    if (Array.isArray(value)) return value.map(plain);
    if (value && value.__fields) {
      // Each field by its own class's JSON name (another class's `@SerialName` for a field of
      // the same name doesn't apply).
      const own = CLASSES[value.__class]?.definition?.serialNames;
      return Object.fromEntries(
        value.__fields.map((name) => [
          (own ? own[name] : renames[name]) ?? name,
          plain(value[name]),
        ]),
      );
    }
    // JSON objects (`buildJsonObject`, a request's fields) with maps or DTOs inside.
    if (
      value !== null &&
      typeof value === 'object' &&
      Object.getPrototypeOf(value) === Object.prototype &&
      !value.__class
    ) {
      const out = {};
      for (const [key, item] of Object.entries(value))
        if (typeof item !== 'function') out[key] = plain(item);
      return out;
    }
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
    /// OkHttp's `topPrivateDomain()`: the registrable domain (two labels, three under a
    /// two-letter country's second level such as `co.jp`).
    topPrivateDomain() {
      const labels = this.parts.host.split('.');
      if (labels.length <= 2) return this.parts.host;
      const second = labels[labels.length - 2];
      const country = labels[labels.length - 1].length === 2;
      const take =
        country &&
        ['co', 'com', 'net', 'org', 'ne', 'or', 'ac', 'go', 'gov', 'edu'].includes(second)
          ? 3
          : 2;
      return labels.slice(-take).join('.');
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
      this.queryPairs = parts.query ? parts.query.split('&') : [];
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
    // Android's `Uri.Builder` names for the same.
    appendQueryParameter(name, value) {
      return this.addQueryParameter(name, value);
    }
    appendPath(segment) {
      return this.addPathSegment(segment);
    }
    appendEncodedPath(segments) {
      return this.addEncodedPathSegments(KT.str(segments).replace(/^\/+/, ''));
    }
    authority(value) {
      return this.host(value);
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
    removePathSegment(index) {
      this.segments.splice(index, 1);
      return this;
    }
    encodedQuery(query) {
      this.queryPairs = query ? KT.str(query).split('&') : [];
      return this;
    }
    encodedPath(path) {
      this.segments = KT.str(path).split('/').slice(1);
      return this;
    }
    addQueryParameter(name, value) {
      if (value === null || value === undefined)
        this.queryPairs.push(encodeURIComponent(KT.str(name)));
      else
        this.queryPairs.push(
          `${encodeURIComponent(KT.str(name))}=${encodeURIComponent(KT.str(value))}`,
        );
      return this;
    }
    addEncodedQueryParameter(name, value) {
      this.queryPairs.push(value == null ? KT.str(name) : `${KT.str(name)}=${KT.str(value)}`);
      return this;
    }
    removeAllQueryParameters(name) {
      const key = encodeURIComponent(KT.str(name));
      this.queryPairs = this.queryPairs.filter((pair) => pair.split('=')[0] !== key);
      return this;
    }
    setQueryParameter(name, value) {
      return this.removeAllQueryParameters(name).addQueryParameter(name, value);
    }
    setEncodedQueryParameter(name, value) {
      this.queryPairs = this.queryPairs.filter((pair) => pair.split('=')[0] !== KT.str(name));
      return this.addEncodedQueryParameter(name, value);
    }
    query(text) {
      this.queryPairs = text ? KT.str(text).split('&') : [];
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
      const query = this.queryPairs.length ? `?${this.queryPairs.join('&')}` : '';
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
    /// OkHttp's `headers.getDate("Date")`: the header's time, or null.
    getDate(name) {
      const value = this.get(name);
      const ms = value === null ? NaN : Date.parse(value);
      return Number.isNaN(ms) ? null : ms;
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
    /// What `tag(type, value)` put on it (`request.tag(Page::class.java)`).
    tag(type) {
      const tags = this.tags ?? {};
      const key = type === undefined ? 'Object' : KT.str(type).split('.').pop();
      return tags[key] ?? null;
    }
    newBuilder() {
      const builder = new RequestBuilder();
      builder.url_ = this.url;
      builder.method_ = this.method;
      builder.headers_ = this.headers.newBuilder();
      builder.body_ = this.body;
      builder.kind_ = this.kind;
      builder.tags_ = { ...(this.tags ?? {}) };
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
    /// OkHttp's `tag(type, value)` (or `tag(value)`): something carried with the request for
    /// its interceptors.
    tag(type, value) {
      this.tags_ = { ...(this.tags_ ?? {}) };
      if (value === undefined) this.tags_.Object = type ?? null;
      else this.tags_[KT.str(type).split('.').pop()] = value;
      return this;
    }
    fetch(fetch) {
      this.kind_ = fetch;
      return this;
    }
    build() {
      const request = new Request({
        url: this.url_,
        method: this.method_,
        headers: this.headers_.build(),
        body: this.body_,
        kind: this.kind_,
      });
      request.tags = this.tags_ ?? {};
      return request;
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
      // OkHttp's `FormBody`: its fields read back (`body.size`, `name(i)`, `value(i)`).
      build: () => ({
        __class: 'FormBody',
        __classes: ['FormBody', 'RequestBody'],
        isBody: true,
        text: K.form(pairs),
        type: 'application/x-www-form-urlencoded',
        size: pairs.length,
        name: (index) => pairs[index]?.[0] ?? noSuch(`no field ${index}`),
        value: (index) => pairs[index]?.[1] ?? noSuch(`no field ${index}`),
        encodedName: (index) => encodeURIComponent(pairs[index]?.[0] ?? ''),
        encodedValue: (index) => encodeURIComponent(pairs[index]?.[1] ?? ''),
        contentType: () => 'application/x-www-form-urlencoded',
        contentLength: () => K.bin.utf8(K.form(pairs)).length,
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
      // The answer's text as Raygun decoded it; its bytes as they came.
      this.body = bytesBody(
        () => raw.bytes(),
        raw.header('content-type'),
        () => raw.text(),
      );
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
    /// Plain JSON; `KT.m` decodes it as the type the call asks for. Keiyoushi's
    /// `parseAs<T> { text -> … }` changes the text first (a JSONP answer's call around it).
    async parseAs(transform) {
      const text = await this.raw.text();
      return JSON.parse(typeof transform === 'function' ? KT.str(await transform(text)) : text);
    }
    /// Keiyoushi's `parseGraphQLAs<T>()`: the answer's `data`; any errors fail it, even beside
    /// data (MangaHub's `{"data": {"chapter": null}, "errors": […]}` asks for a new key).
    async parseGraphQLAs() {
      const answer = JSON.parse(await this.raw.text());
      if (Array.isArray(answer.errors) && answer.errors.length) {
        throw new KotlinError(
          answer.errors.map((error) => KT.str(error?.message ?? '')).join('\n'),
          'GraphQLException',
        );
      }
      if (answer.data == null)
        throw new KotlinError(
          "GraphQL response is missing the 'data' field",
          'IllegalStateException',
        );
      return answer.data;
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
      const qualified = name.trim().replace(/\?$/, '');
      return { name: qualified.split('.').pop(), qualified, args };
    };
    return read();
  };
  /// A declared class by the name a type gives it (`ReaderDto.ChapterDto`, `ChapterDto`).
  const classOf = (type) => CLASSES[type.qualified] ?? CLASSES[type.name];
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
      const itemType = type.args[0] ?? { name: 'Any', args: [] };
      if (!Array.isArray(value)) {
        // One object where a list goes, as serializers that take either read it
        // (`SingleResultSerializer`), and a PHP-style object of numbered entries.
        if (value && typeof value === 'object' && classOf(itemType)) {
          const keys = Object.keys(value);
          const numbered = keys.length > 0 && keys.every((key) => /^\d+$/.test(key));
          const items = numbered ? keys.map((key) => value[key]) : [value];
          const out = [];
          for (const item of items) out.push(await decodeAs(item, itemType));
          return out;
        }
        return value;
      }
      const out = [];
      for (const item of value) out.push(await decodeAs(item, itemType));
      return out;
    }
    if (type.name === 'Map' || type.name === 'HashMap' || type.name === 'MutableMap') {
      if (!value || typeof value !== 'object') return value;
      const out = {};
      for (const [key, item] of Object.entries(value))
        out[key] = await decodeAs(item, type.args[1] ?? { name: 'Any', args: [] });
      return out;
    }
    const holder = classOf(type);
    if (holder && value && typeof value === 'object' && !Array.isArray(value))
      return polymorphic(holder, value).fromJson(value, type.args);
    // An enum's entry by its name as the answer spells it (`"shounen"` for `SHOUNEN`).
    if (holder && typeof value === 'string' && holder.definition?.kind === 'enum') {
      const simple = (text) => text.replace(/[-_ ]/g, '').toLowerCase();
      const entry =
        holder.definition.entries.find((name) => name === value) ??
        holder.definition.entries.find((name) => simple(name) === simple(value));
      return entry === undefined ? value : holder[entry];
    }
    // A list where one object goes (`JsonTransformingSerializer`s taking an array's first).
    if (holder && Array.isArray(value) && value.length && value[0] && typeof value[0] === 'object')
      return decodeAs(value[0], type);
    return value;
  };
  /// A sealed or abstract class's subclass that an answer names in its `type` (kotlinx's class
  /// discriminator), by its `@SerialName` or its name; else the class itself.
  /// The keys answers name their subclass by (kotlinx's `classDiscriminator`, `type` unless a
  /// source's `Json { classDiscriminator = … }` says).
  const discriminators = ['type'];
  const polymorphic = (holder, value) => {
    const key = discriminators.find((name) => typeof value[name] === 'string');
    const named = key === undefined ? undefined : value[key];
    if (typeof named !== 'string') return holder;
    for (const candidate of new Set(Object.values(CLASSES))) {
      if (
        candidate !== holder &&
        candidate.supers?.includes(holder.name) &&
        (candidate.serialName === named || candidate.name === named)
      )
        return candidate;
    }
    return holder;
  };
  /// A field's type with the class's type parameters put in (`T` of `ApiResponse<PageData>`).
  const substitute = (type, names, args) => {
    const at = names.indexOf(type.name);
    if (at >= 0 && args[at]) return args[at];
    return {
      name: type.name,
      qualified: type.qualified,
      args: type.args.map((arg) => substitute(arg, names, args)),
    };
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

  // ---- Next.js pages' data ----
  // Keiyoushi's `extractNextJs` and `extractNextJsRsc` (core `keiyoushi.utils`, from
  // keiyoushi/extensions-source, Apache-2.0): the data a Next.js page hydrates with, from the
  // App Router's flight stream (`self.__next_f.push`) or the Pages Router's `__NEXT_DATA__`, and
  // the first value in it the predicate (or the type's required fields) picks.

  const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
  const NEXT_F = /self\.__next_f\.push\(\s*(\[[\s\S]*])\s*\)\s*;?\s*$/;
  const HEX = /^[0-9a-fA-F]+$/;
  /// The flight stream a page's scripts push, joined: each push is a slice cut anywhere, `[1, text]`
  /// as text and `[3, base64]` as bytes.
  const flightStream = (document) => {
    let stream = '';
    for (const script of document.select('script:not([src])')) {
      const data = script.data();
      if (!data.includes('self.__next_f.push')) continue;
      try {
        const found = NEXT_F.exec(data);
        if (!found) continue;
        const push = JSON.parse(found[1]);
        const content = push[1];
        if (content === null || content === undefined || typeof content === 'object') continue;
        if (push[0] === 1) stream += String(content);
        else if (push[0] === 3) stream += K.base64Decode(String(content));
      } catch {
        // A push that isn't JSON is left out, as Keiyoushi's helper does.
      }
    }
    return stream;
  };
  const isSpace = (char) => char === ' ' || char === '\n' || char === '\r' || char === '\t';
  /// The JSON value starting at `start` and where it ends (`undefined` when it isn't one).
  const jsonAt = (body, start) => {
    if (start >= body.length) return [undefined, start];
    const parse = (end) => {
      try {
        return JSON.parse(body.slice(start, end));
      } catch {
        return undefined;
      }
    };
    let depth = 0;
    let inString = false;
    let escape = false;
    let at = start;
    while (at < body.length) {
      const char = body[at++];
      if (escape) {
        escape = false;
        continue;
      }
      if (char === '\\' && inString) {
        escape = true;
        continue;
      }
      if (char === '"') {
        inString = !inString;
        continue;
      }
      if (inString) continue;
      if (char === '{' || char === '[') depth++;
      else if (char === '}' || char === ']') {
        if (--depth === 0) return [parse(at), at];
      }
      if (depth === 0 && isSpace(char)) return [parse(at - 1), at];
    }
    return [undefined, at];
  };
  /// A flight stream's rows (`<hex id>:<json>`, or `<hex id>:T<hex byte length>,<text>`): every
  /// JSON value in it, in order, with the text chunks and the models kept by id.
  const flightRows = (body, chunks, models) => {
    const rows = [];
    let at = 0;
    while (at < body.length) {
      const colon = body.indexOf(':', at);
      if (colon === -1) break;
      const id = body.slice(at, colon);
      if (!HEX.test(id)) {
        at++;
        continue;
      }
      at = colon + 1;
      if (at >= body.length) break;
      if (body[at] === 'T') {
        at++;
        const comma = body.indexOf(',', at);
        if (comma === -1) break;
        const length = body.slice(at, comma);
        if (!HEX.test(length)) break;
        const bytes = parseInt(length, 16);
        at = comma + 1;
        const start = at;
        // The length counts UTF-8 bytes.
        let counted = 0;
        while (at < body.length && counted < bytes) {
          const code = body.charCodeAt(at);
          if (code < 0x80) counted += 1;
          else if (code < 0x800) counted += 2;
          else if (code >= 0xd800 && code <= 0xdbff) {
            counted += 4;
            at++;
          } else counted += 3;
          at++;
        }
        const text = body.slice(start, at);
        chunks.set(id, text);
        try {
          rows.push(JSON.parse(text));
        } catch {
          // A text chunk needn't be JSON.
        }
      } else {
        const [value, end] = jsonAt(body, at);
        if (value !== undefined) {
          rows.push(value);
          models.set(id, value);
        }
        at = end;
      }
    }
    return rows;
  };
  /// A value with React Flight's markers read: `$$` (an escaped `$`), `$undefined`, `$D` dates,
  /// `$n` big numbers, `$Infinity`…, `$Q` maps, `$W` sets, `$L` lazy and `$@` promised rows, and
  /// `$<id>` / `$<id>:<path>` references to other rows.
  const resolveFlight = (value, chunks, models, resolving) => {
    if (Array.isArray(value))
      return value.map((item) => resolveFlight(item, chunks, models, resolving));
    if (value !== null && typeof value === 'object') {
      const out = {};
      for (const [key, item] of Object.entries(value))
        out[key] = resolveFlight(item, chunks, models, resolving);
      return out;
    }
    if (typeof value !== 'string' || value.length < 2 || value[0] !== '$') return value;
    if (value === '$undefined') return null;
    if (['$Infinity', '$-Infinity', '$NaN', '$-0'].includes(value)) return value.slice(1);
    let found;
    switch (value[1]) {
      case '$':
        return value.slice(1);
      case 'D':
      case 'n':
        return value.slice(2);
      case 'Q':
        found = flightMap(value.slice(2), chunks, models, resolving);
        break;
      case 'W':
        found = flightSet(value.slice(2), chunks, models, resolving);
        break;
      case 'L':
      case '@':
        found = flightModel(value.slice(2), chunks, models, resolving);
        break;
      default:
        found = flightModel(value.slice(1), chunks, models, resolving);
    }
    return found === undefined ? value : found;
  };
  /// One step into a row: by key, by index, or a React element's (`["$", type, key, props]`) part.
  const flightStep = (value, segment) => {
    if (Array.isArray(value)) {
      if (value.length >= 4 && value[0] === '$') {
        const part = { type: 1, key: 2, props: 3 }[segment];
        if (part !== undefined) return value[part];
      }
      if (!/^-?\d+$/.test(segment)) return undefined;
      const index = Number(segment);
      return index >= 0 && index < value.length ? value[index] : undefined;
    }
    if (value !== null && typeof value === 'object')
      return has(value, segment) ? value[segment] : undefined;
    return undefined;
  };
  const flightModel = (reference, chunks, models, resolving) => {
    const segments = reference.split(':');
    const id = segments[0];
    if (segments.length === 1 && chunks.has(id)) return chunks.get(id);
    if (resolving.has(id) || !models.has(id)) return undefined;
    const guard = new Set(resolving).add(id);
    let value = models.get(id);
    for (const segment of segments.slice(1)) {
      if (typeof value === 'string' && value.startsWith('$'))
        value = resolveFlight(value, chunks, models, guard);
      value = flightStep(value, segment);
      if (value === undefined) return undefined;
    }
    return resolveFlight(value, chunks, models, guard);
  };
  const flightMap = (id, chunks, models, resolving) => {
    if (resolving.has(id)) return undefined;
    const entries = models.get(id);
    if (!Array.isArray(entries)) return undefined;
    const resolved = resolveFlight(entries, chunks, models, new Set(resolving).add(id));
    const out = {};
    for (const pair of resolved) {
      if (!Array.isArray(pair) || pair.length !== 2) continue;
      const [key, item] = pair;
      const name =
        key !== null && typeof key === 'object'
          ? JSON.stringify(key)
          : key === null
            ? 'null'
            : String(key);
      out[name] = item;
    }
    return out;
  };
  const flightSet = (id, chunks, models, resolving) => {
    if (resolving.has(id)) return undefined;
    const values = models.get(id);
    if (!Array.isArray(values)) return undefined;
    return resolveFlight(values, chunks, models, new Set(resolving).add(id));
  };
  /// The Pages Router's data: `props.pageProps`, then the whole.
  const pagesRouterData = (document) => {
    const script = document.selectFirst('script#__NEXT_DATA__');
    if (!script) return [];
    try {
      const root = JSON.parse(script.data());
      const pageProps = root?.props?.pageProps;
      return [pageProps, root].filter((value) => value !== null && value !== undefined);
    } catch {
      return [];
    }
  };
  /// A type's required JSON fields (each with its other names), as kotlinx.serialization's
  /// descriptor gives them: neither optional nor nullable.
  const requiredFields = (type) => {
    const fields = classOf(type)?.required ?? [];
    if (!fields.length)
      throw new KotlinError(
        `Cannot infer a predicate for ${type.name}: all fields are optional or nullable. Provide an explicit predicate instead.`,
        'IllegalArgumentException',
      );
    return fields;
  };
  const isJsonObject = (value) =>
    value !== null && typeof value === 'object' && !Array.isArray(value);
  /// The predicate a type gives: an object with its required fields, or for `List<T>` a list
  /// whose first item is one.
  const typePredicate = (typeText) => {
    const type = parseType(typeText);
    const list = type.name === 'List' || type.name === 'MutableList';
    const fields = requiredFields(list ? (type.args[0] ?? { name: 'Any', args: [] }) : type);
    const fits = (value) =>
      isJsonObject(value) && fields.every((names) => names.some((name) => has(value, name)));
    return async (value) =>
      list ? Array.isArray(value) && value.length > 0 && fits(value[0]) : fits(value);
  };
  /// The first object or list, depth first, that the predicate picks.
  const findNextJs = async (value, predicate) => {
    if (value === null || typeof value !== 'object') return undefined;
    if (KT.truthy(await predicate(value))) return value;
    for (const child of Array.isArray(value) ? value : Object.values(value)) {
      const found = await findNextJs(child, predicate);
      if (found !== undefined) return found;
    }
    return undefined;
  };
  const extractNextJs = async (rows, chunks, models, predicate, type) => {
    for (const row of rows) {
      const found = await findNextJs(resolveFlight(row, chunks, models, new Set()), predicate);
      if (found !== undefined) return decode(found, type);
    }
    return null;
  };
  const nextJsPredicate = (args, lambda, type) => {
    const given = lambda ?? args.find((arg) => typeof arg === 'function');
    if (given) return (value) => given(value);
    if (!type) throw new KotlinError('extractNextJs() needs a type or a predicate');
    return typePredicate(type);
  };
  const nextJsOfDocument = async (document, args, lambda, type) => {
    const predicate = nextJsPredicate(args, lambda, type);
    const chunks = new Map();
    const models = new Map();
    let rows = flightRows(flightStream(document), chunks, models);
    if (!rows.length) rows = pagesRouterData(document);
    return extractNextJs(rows, chunks, models, predicate, type);
  };
  const nextJsOfText = async (text, args, lambda, type) => {
    const predicate = nextJsPredicate(args, lambda, type);
    const chunks = new Map();
    const models = new Map();
    return extractNextJs(flightRows(KT.str(text), chunks, models), chunks, models, predicate, type);
  };
  const isDocument = (value) =>
    value !== null &&
    typeof value === 'object' &&
    typeof value.location === 'function' &&
    typeof value.select === 'function';
  /// `document.extractNextJs<T>()`, `response.extractNextJs<T>()` (an RSC answer or a page, by
  /// its type) and `text.extractNextJsRsc<T>()`.
  const NEXT_JS = {
    extractNextJs: async (receiver, args, lambda, type) => {
      if (isDocument(receiver)) return nextJsOfDocument(receiver, args, lambda, type);
      if (receiver instanceof Response) {
        const contentType = KT.str(receiver.header('Content-Type') ?? '');
        const text = await receiver.raw.text();
        if (contentType.includes('text/x-component')) return nextJsOfText(text, args, lambda, type);
        if (contentType.includes('text/html') || /^\s*</.test(text))
          return nextJsOfDocument(
            Jsoup.parse(text, receiver.request.url.toString()),
            args,
            lambda,
            type,
          );
        return nextJsOfText(text, args, lambda, type);
      }
      if (typeof receiver === 'string')
        return nextJsOfDocument(Jsoup.parse(receiver, ''), args, lambda, type);
      throw new KotlinError('extractNextJs() reads a document or an answer');
    },
    extractNextJsRsc: async (receiver, args, lambda, type) =>
      nextJsOfText(receiver, args, lambda, type),
  };
  const hasNextJs = (receiver, name) =>
    Boolean(NEXT_JS[name]) &&
    (name === 'extractNextJsRsc'
      ? typeof receiver === 'string'
      : isDocument(receiver) || receiver instanceof Response);

  KT.unconverted = (name, reason) => ({
    isClass: true,
    name,
    make: async () => KT.raise(`Raygun couldn't convert ${name}: ${reason}`),
    fromJson: async () => KT.raise(`Raygun couldn't convert ${name}: ${reason}`),
  });

  /// The source's client: requests over the source's `Http` (its Referer, rate limit, cookies).
  // ---- Cookies ----

  /// An OkHttp `Cookie`.
  const makeCookie = (fields) => ({
    __class: 'Cookie',
    __classes: ['Cookie'],
    name: '',
    value: '',
    domain: '',
    path: '/',
    expiresAt: 253402300799999,
    secure: false,
    httpOnly: false,
    hostOnly: true,
    persistent: false,
    ...fields,
    toString() {
      return `${this.name}=${this.value}`;
    },
  });
  /// `Cookie.parse(url, "name=value; Path=/; …")`.
  const parseCookie = (url, line) => {
    const parts = KT.str(line).split(';');
    const [name, ...rest] = parts[0].split('=');
    if (!name || !name.trim()) return null;
    const host = url && url.host !== undefined ? url.host : HttpUrl.parse(KT.str(url)).host;
    const cookie = makeCookie({ name: name.trim(), value: rest.join('=').trim(), domain: host });
    for (const attribute of parts.slice(1)) {
      const [key, ...value] = attribute.split('=');
      const text = value.join('=').trim();
      switch (key.trim().toLowerCase()) {
        case 'domain':
          cookie.domain = text.replace(/^\./, '');
          cookie.hostOnly = false;
          break;
        case 'path':
          cookie.path = text || '/';
          break;
        case 'expires': {
          const ms = Date.parse(text);
          if (!Number.isNaN(ms)) {
            cookie.expiresAt = ms;
            cookie.persistent = true;
          }
          break;
        }
        case 'max-age': {
          const seconds = Number(text);
          if (Number.isFinite(seconds)) {
            cookie.expiresAt = Date.now() + seconds * 1000;
            cookie.persistent = true;
          }
          break;
        }
        case 'secure':
          cookie.secure = true;
          break;
        case 'httponly':
          cookie.httpOnly = true;
          break;
        default:
      }
    }
    return cookie;
  };
  /// A cookie as the `Set-Cookie` line that keeps it.
  const cookieLine = (cookie) => {
    const parts = [`${KT.str(cookie.name)}=${KT.str(cookie.value)}`];
    if (cookie.domain && !cookie.hostOnly) parts.push(`Domain=${cookie.domain}`);
    parts.push(`Path=${cookie.path || '/'}`);
    if (cookie.persistent && Number.isFinite(cookie.expiresAt))
      parts.push(`Expires=${new Date(cookie.expiresAt).toUTCString()}`);
    if (cookie.secure) parts.push('Secure');
    if (cookie.httpOnly) parts.push('HttpOnly');
    return parts.join('; ');
  };
  const cookieBuilder = () => {
    const fields = {};
    const builder = {
      name: (name) => ((fields.name = KT.str(name)), builder),
      value: (value) => ((fields.value = KT.str(value)), builder),
      domain: (domain) => ((fields.domain = KT.str(domain)), (fields.hostOnly = false), builder),
      hostOnlyDomain: (domain) => (
        (fields.domain = KT.str(domain)),
        (fields.hostOnly = true),
        builder
      ),
      path: (path) => ((fields.path = KT.str(path)), builder),
      expiresAt: (ms) => ((fields.expiresAt = ms), (fields.persistent = true), builder),
      secure: () => ((fields.secure = true), builder),
      httpOnly: () => ((fields.httpOnly = true), builder),
      build: () => makeCookie(fields),
    };
    return builder;
  };
  const urlText = (url) => (url && url.parts ? url.toString() : KT.str(url));
  /// The source's `client.cookieJar`: the extension's own jar.
  const cookieJar = {
    __class: 'CookieJar',
    __classes: ['CookieJar'],
    loadForRequest: (url) => {
      const href = urlText(url);
      const host = HttpUrl.parse(href).host;
      return K.cookies(href).map(([name, value]) => makeCookie({ name, value, domain: host }));
    },
    saveFromResponse: (url, cookies) => {
      const list = Array.isArray(cookies) ? cookies : [cookies];
      K.setCookies(urlText(url), list.filter(Boolean).map(cookieLine));
    },
  };
  /// Android's `CookieManager`: the same jar, as `Cookie` headers.
  const cookieManager = {
    getCookie: (url) => {
      const pairs = K.cookies(urlText(url));
      return pairs.length ? pairs.map(([name, value]) => `${name}=${value}`).join('; ') : null;
    },
    setCookie: (url, line, callback = null) => {
      K.setCookies(urlText(url), [KT.str(line)]);
      if (typeof callback === 'function') callback(true);
    },
    hasCookies: () => true,
    flush: () => KT.unit,
    setAcceptCookie: () => KT.unit,
    setAcceptThirdPartyCookies: () => KT.unit,
    removeAllCookies: (callback = null) => {
      if (typeof callback === 'function') callback(false);
    },
    removeSessionCookies: (callback = null) => {
      if (typeof callback === 'function') callback(false);
    },
  };

  class Client {
    constructor(source) {
      this.owner = source;
      this.interceptors = [];
      // OkHttp's `followRedirects(false)`: redirects come back as they are.
      this.followsRedirects = true;
    }
    get cookieJar() {
      return cookieJar;
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
        body = request.body.bytes instanceof Uint8Array ? request.body.bytes : request.body.text;
        if (request.body.type) headers['Content-Type'] = request.body.type;
      }
      const raw = await this.owner.http.request(request.url.toString(), {
        method: request.method,
        headers,
        body,
        kind: kind === 'none' ? null : kind,
        from: fetch && fetch.page ? fetch.page : null,
        ensureSuccess: false,
        redirect: this.followsRedirects ? null : 'manual',
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
    put(url, ...rest) {
      return this.request('PUT', url, rest);
    }
    patch(url, ...rest) {
      return this.request('PATCH', url, rest);
    }
    delete(url, ...rest) {
      return this.request('DELETE', url, rest);
    }
    async request(method, url, rest) {
      // Every argument named (`client.post(url = …, body = …)`).
      if (url && url[NAMED]) {
        rest = [url, ...rest];
        url = url.url;
      }
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
    /// OkHttp's `newBuilder()`: a client of its own, with this one's interceptors and settings
    /// (changing it leaves this one as it is).
    newBuilder() {
      const copy = new Client(this.owner);
      copy.interceptors = [...this.interceptors];
      copy.followsRedirects = this.followsRedirects;
      copy.defaultHeaders = () => (this.defaultHeaders ? this.defaultHeaders() : new Headers());
      return clientBuilder(copy);
    }
  }
  const clientBuilder = (client) => {
    const builder = {
      rateLimit: (permits, period = 1, unit = null) => {
        const unitMs = unit && unit.unitMs ? unit.unitMs : 1000;
        const periodMs =
          typeof period === 'number' ? period * unitMs : period && period.ms ? period.ms : 1000;
        client.owner.http.limit = { permits, periodMs };
        return builder;
      },
      rateLimitHost: (_host, permits, period = 1, unit = null) =>
        builder.rateLimit(permits, period, unit),
      connectTimeout: () => builder,
      readTimeout: () => builder,
      writeTimeout: () => builder,
      callTimeout: () => builder,
      followRedirects: (follow = true) => {
        client.followsRedirects = follow !== false;
        return builder;
      },
      followSslRedirects: () => builder,
      retryOnConnectionFailure: () => builder,
      // Raygun's own client resolves names, picks protocols, checks certificates and caches.
      cache: () => builder,
      dispatcher: () => builder,
      dns: () => builder,
      protocols: () => builder,
      hostnameVerifier: () => builder,
      sslSocketFactory: () => builder,
      connectionPool: () => builder,
      networkInterceptors: () => client.interceptors,
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
          const host = domain ? KT.str(await domain()) : HttpUrl.parse(client.owner.baseUrl).host;
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
    const body = bytesBody(
      async () => K.bin.utf8(value),
      type,
      async () => value,
    );
    body.contentLength = () => K.bin.utf8(value).length;
    return body;
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
    ['nanoseconds', 1e-6],
    ['microseconds', 1e-3],
    ['milliseconds', 1],
    ['seconds', 1000],
    ['minutes', 60000],
    ['hours', 3600000],
    ['days', 86400000],
  ])
    durations[unit] = ms;
  /// A unit's milliseconds: Kotlin's `DurationUnit` or Java's `TimeUnit` (milliseconds if none).
  const unitMsOf = (unit) => (unit && typeof unit.unitMs === 'number' ? unit.unitMs : 1);
  /// Java's `TimeUnit` (and Kotlin's `DurationUnit`): a unit and its conversions
  /// (`TimeUnit.MILLISECONDS.toDays(ms)`, `TimeUnit.SECONDS.convert(n, TimeUnit.MINUTES)`).
  const timeUnit = (name, unitMs) => {
    const within = (n, size) => Math.trunc((Number(n) * unitMs) / size);
    return {
      name,
      unitMs,
      ordinal: [
        'NANOSECONDS',
        'MICROSECONDS',
        'MILLISECONDS',
        'SECONDS',
        'MINUTES',
        'HOURS',
        'DAYS',
      ].indexOf(name),
      toString: () => name,
      toNanos: (n) => within(n, 1e-6),
      toMicros: (n) => within(n, 1e-3),
      toMillis: (n) => within(n, 1),
      toSeconds: (n) => within(n, 1000),
      toMinutes: (n) => within(n, 60000),
      toHours: (n) => within(n, 3600000),
      toDays: (n) => within(n, 86400000),
      convert: (n, from) =>
        n && n.isDuration
          ? Math.trunc(n.ms / unitMs)
          : Math.trunc((Number(n) * unitMsOf(from)) / unitMs),
      sleep: (n) => pause(Number(n) * unitMs),
    };
  };
  /// Kotlin's `Duration` (java.time's too, as `Duration.ofSeconds(…)` makes it): a length of time
  /// in milliseconds, compared and added as one.
  class KDuration {
    constructor(ms) {
      this.ms = Number(ms);
      this.isDuration = true;
    }
    get inWholeNanoseconds() {
      return Math.trunc(this.ms * 1e6);
    }
    get inWholeMicroseconds() {
      return Math.trunc(this.ms * 1e3);
    }
    get inWholeMilliseconds() {
      return Math.trunc(this.ms);
    }
    get inWholeSeconds() {
      return Math.trunc(this.ms / 1000);
    }
    get inWholeMinutes() {
      return Math.trunc(this.ms / 60000);
    }
    get inWholeHours() {
      return Math.trunc(this.ms / 3600000);
    }
    get inWholeDays() {
      return Math.trunc(this.ms / 86400000);
    }
    get absoluteValue() {
      return new KDuration(Math.abs(this.ms));
    }
    isNegative() {
      return this.ms < 0;
    }
    isPositive() {
      return this.ms > 0;
    }
    isInfinite() {
      return !Number.isFinite(this.ms);
    }
    isFinite() {
      return Number.isFinite(this.ms);
    }
    isZero() {
      return this.ms === 0;
    }
    plus(other) {
      return new KDuration(this.ms + (other && other.isDuration ? other.ms : Number(other)));
    }
    minus(other) {
      return new KDuration(this.ms - (other && other.isDuration ? other.ms : Number(other)));
    }
    times(factor) {
      return new KDuration(this.ms * Number(factor));
    }
    div(divisor) {
      return divisor && divisor.isDuration
        ? this.ms / divisor.ms
        : new KDuration(this.ms / Number(divisor));
    }
    unaryMinus() {
      return new KDuration(-this.ms);
    }
    compareTo(other) {
      return Math.sign(this.ms - (other && other.isDuration ? other.ms : Number(other)));
    }
    toLong(unit) {
      return Math.trunc(this.ms / unitMsOf(unit));
    }
    toInt(unit) {
      return Math.trunc(this.ms / unitMsOf(unit));
    }
    toDouble(unit) {
      return this.ms / unitMsOf(unit);
    }
    // java.time's names.
    toMillis() {
      return Math.trunc(this.ms);
    }
    toNanos() {
      return Math.trunc(this.ms * 1e6);
    }
    toSeconds() {
      return Math.trunc(this.ms / 1000);
    }
    getSeconds() {
      return Math.trunc(this.ms / 1000);
    }
    toMinutes() {
      return Math.trunc(this.ms / 60000);
    }
    toHours() {
      return Math.trunc(this.ms / 3600000);
    }
    toDays() {
      return Math.trunc(this.ms / 86400000);
    }
    equals(other) {
      return Boolean(other && other.isDuration && other.ms === this.ms);
    }
    /// Kotlin's spelling: `1d 2h 3m 4.5s`, `250ms`, `0s`.
    toString() {
      if (!Number.isFinite(this.ms)) return this.ms > 0 ? 'Infinity' : '-Infinity';
      if (this.ms === 0) return '0s';
      const sign = this.ms < 0 ? '-' : '';
      let rest = Math.abs(this.ms);
      if (rest < 1000) return `${sign}${rest}ms`;
      const parts = [];
      for (const [unit, size] of [
        ['d', 86400000],
        ['h', 3600000],
        ['m', 60000],
      ]) {
        const count = Math.floor(rest / size);
        if (count) parts.push(`${count}${unit}`);
        rest -= count * size;
      }
      if (rest) parts.push(`${rest / 1000}s`);
      return sign + parts.join(' ');
    }
  }
  const duration = (ms) => new KDuration(ms);
  /// `java.util.Random`: the 48-bit linear congruential generator Java specifies (seeded from the
  /// clock when no seed is given).
  const javaRandom = (seed) => {
    const MULTIPLIER = 0x5deece66dn;
    const MASK = (1n << 48n) - 1n;
    let state =
      (BigInt(Math.trunc(Number(seed ?? Date.now() * 1000 + Math.floor(Math.random() * 1000)))) ^
        MULTIPLIER) &
      MASK;
    const next = (bits) => {
      state = (state * MULTIPLIER + 0xbn) & MASK;
      return Number(BigInt.asIntN(32, state >> BigInt(48 - bits)));
    };
    const random = {
      __class: 'Random',
      __classes: ['Random'],
      nextInt: (bound, until) => {
        if (until !== undefined) return bound + random.nextInt(until - bound);
        if (bound === undefined) return next(32);
        if ((bound & -bound) === bound) return Number((BigInt(bound) * BigInt(next(31))) >> 31n);
        let bits;
        let value;
        do {
          bits = next(31);
          value = bits % bound;
        } while (bits - value + (bound - 1) < 0 || bits - value + (bound - 1) > 2147483647);
        return value;
      },
      nextLong: () => Number(BigInt.asIntN(64, (BigInt(next(32)) << 32n) + BigInt(next(32)))),
      nextBoolean: () => next(1) !== 0,
      nextDouble: () => (next(26) * 2 ** 27 + next(27)) / 2 ** 53,
      nextFloat: () => next(24) / 2 ** 24,
      nextBytes: (target) => {
        for (let at = 0; at < target.length;) {
          let word = next(32);
          for (let left = Math.min(target.length - at, 4); left-- > 0; word >>= 8)
            target[at++] = (word << 24) >> 24;
        }
        return KT.unit;
      },
    };
    return random;
  };
  /// Kotlin's `TimeMark` (`TimeSource.Monotonic.markNow()`, moved by a duration): a moment on
  /// the clock in milliseconds.
  const timeMark = (at) => ({
    __class: 'TimeMark',
    __classes: ['TimeMark', 'ComparableTimeMark'],
    ms: at,
    isTimeMark: true,
    elapsedNow: () => duration(Date.now() - at),
    hasPassedNow: () => Date.now() >= at,
    hasNotPassedNow: () => Date.now() < at,
    plus: (other) => timeMark(at + (other && other.isDuration ? other.ms : Number(other))),
    minus: (other) =>
      other && other.isTimeMark
        ? duration(at - other.ms)
        : timeMark(at - (other && other.isDuration ? other.ms : Number(other))),
    compareTo: (other) => Math.sign(at - other.ms),
  });

  // ---- Streams: okio's `Buffer`, Java's byte streams and `ByteBuffer` ----
  // A `ByteArray` stays a list of numbers 0–255; streams and buffers hold `Uint8Array`s, so a
  // picture's bytes reach Rust without a list of every byte.

  /// The bytes of a value as a `Uint8Array`: a `ByteArray`, a buffer or stream, text's UTF-8.
  const u8Of = (value) => {
    if (value instanceof Uint8Array) return value;
    if (Array.isArray(value)) return Uint8Array.from(value, (byte) => byte & 255);
    if (value && typeof value.u8Now === 'function') return value.u8Now();
    if (value === null || value === undefined) return new Uint8Array(0);
    return K.bin.utf8(KT.str(value));
  };
  const listOfBytes = (bytes) => Array.from(bytes);
  /// A whole number from bytes, signed as Java reads it (big-endian unless `little`).
  const signedOf = (bytes, little) => {
    const ordered = little ? [...bytes].reverse() : [...bytes];
    let value = 0;
    for (const byte of ordered) value = value * 256 + byte;
    return ordered[0] >= 128 ? value - 2 ** (8 * ordered.length) : value;
  };
  /// The `size` bytes of a whole number (two's complement), big-endian unless `little`.
  const bytesOfNumber = (value, size, little) => {
    let rest = BigInt.asUintN(size * 8, BigInt(Math.trunc(Number(value))));
    const out = new Uint8Array(size);
    for (let index = size - 1; index >= 0; index--) {
      out[index] = Number(rest & 255n);
      rest >>= 8n;
    }
    return little ? out.reverse() : out;
  };
  const eof = (size, wanted) => new KotlinError(`size=${size} byteCount=${wanted}`, 'EOFException');

  /// Text read line by line (`bufferedReader()`, `reader()`).
  const textReader = (text) => {
    const lines = KT.str(text).split(/\r?\n/);
    if (lines.length && lines[lines.length - 1] === '') lines.pop();
    let at = 0;
    const reader = {
      __class: 'Reader',
      __classes: ['Reader', 'BufferedReader'],
      readText: () => KT.str(text),
      readLine: () => (at < lines.length ? lines[at++] : null),
      readLines: () => lines.slice(at),
      lineSequence: () => lines.slice(at),
      lines: () => lines.slice(at),
      close: () => KT.unit,
      use: async (lambda) => lambda(reader),
    };
    return reader;
  };

  /// okio's `Buffer` (and a body's `source()`): bytes written at the end, read from the front.
  const okioBuffer = (initial = null) => {
    let data = new Uint8Array(Math.max(64, initial ? initial.length : 0));
    let head = 0;
    let tail = 0;
    if (initial) {
      data.set(initial);
      tail = initial.length;
    }
    const room = (extra) => {
      if (tail + extra <= data.length) return;
      const kept = data.subarray(head, tail);
      const grown = new Uint8Array(Math.max(64, (kept.length + extra) * 2));
      grown.set(kept);
      data = grown;
      tail = kept.length;
      head = 0;
    };
    const append = (bytes) => {
      room(bytes.length);
      data.set(bytes, tail);
      tail += bytes.length;
    };
    const take = (count) => {
      const size = tail - head;
      const wanted = count === undefined || count === null ? size : Number(count);
      if (wanted > size) throw eof(size, wanted);
      const out = data.slice(head, head + wanted);
      head += wanted;
      return out;
    };
    const writer = (size, little) => (value) => (
      append(bytesOfNumber(value, size, little)),
      buffer
    );
    const reader = (size, little) => () => signedOf(take(size), little);
    const buffer = {
      __class: 'Buffer',
      __classes: ['Buffer', 'BufferedSource', 'BufferedSink', 'Source', 'Sink'],
      u8Now: () => data.slice(head, tail),
      /// Keiyoushi's `Source.fixedLength(n)` (`keiyoushi.zip`): the next `n` bytes.
      fixedLength: (count) => okioBuffer(take(Math.min(Number(count), tail - head))),
      /// Keiyoushi's `Source.inflate(nowrap)`: what's left, inflated.
      inflate: (nowrap = false) =>
        okioBuffer(inflateBytes(take(), nowrap === true ? 'raw' : 'zlib')),
      /// okio's `rangeEquals(offset, bytes)`: whether the bytes at `offset` are those.
      rangeEquals: (offset, other, otherOffset = 0, count = undefined) => {
        const wanted = u8Of(other);
        const length = count ?? wanted.length - otherOffset;
        if (offset < 0 || head + offset + length > tail) return false;
        for (let index = 0; index < length; index++)
          if (data[head + offset + index] !== wanted[otherOffset + index]) return false;
        return true;
      },
      get size() {
        return tail - head;
      },
      write: (value, offset = 0, count = undefined) => {
        if (typeof value === 'number') return (append([value & 255]), buffer);
        const bytes = u8Of(value);
        append(bytes.subarray(offset, count === undefined ? bytes.length : offset + count));
        return buffer;
      },
      writeAll: async (source) => {
        const bytes = source && source.u8Now ? source.u8Now() : await bodyU8(source);
        append(bytes);
        if (source && typeof source.clear === 'function') source.clear();
        return bytes.length;
      },
      writeUtf8: (text) => (append(K.bin.utf8(text)), buffer),
      writeString: (text) => (append(K.bin.utf8(text)), buffer),
      writeByte: (value) => (append([value & 255]), buffer),
      writeShort: writer(2, false),
      writeShortLe: writer(2, true),
      writeInt: writer(4, false),
      writeIntLe: writer(4, true),
      writeLong: writer(8, false),
      writeLongLe: writer(8, true),
      readByteArray: (count) => listOfBytes(take(count)),
      readByteString: (count) => byteString(take(count)),
      readUtf8: (count) => K.bin.text(take(count)),
      readString: (first, second) => K.bin.text(take(typeof first === 'number' ? first : second)),
      readUtf8Line: () => {
        const at = data.subarray(head, tail).indexOf(10);
        if (at < 0) return head < tail ? K.bin.text(take()) : null;
        const line = K.bin.text(take(at));
        take(1);
        return line.replace(/\r$/, '');
      },
      readByte: () => signedOf(take(1), false),
      readShort: reader(2, false),
      readShortLe: reader(2, true),
      readInt: reader(4, false),
      readIntLe: reader(4, true),
      readLong: reader(8, false),
      readLongLe: reader(8, true),
      readFully: (target) => {
        const bytes = take(target.length);
        for (let index = 0; index < bytes.length; index++) target[index] = bytes[index];
      },
      read: (target, offset = 0, count = undefined) => {
        if (!Array.isArray(target)) return buffer.write(take());
        const wanted = Math.min(count ?? target.length - offset, tail - head);
        if (wanted <= 0) return -1;
        const bytes = take(wanted);
        for (let index = 0; index < bytes.length; index++) target[offset + index] = bytes[index];
        return wanted;
      },
      skip: (count) => (take(count), KT.unit),
      exhausted: () => head >= tail,
      require: (count) => {
        if (tail - head < count) throw eof(tail - head, count);
        return KT.unit;
      },
      request: (count) => tail - head >= count,
      indexOf: (byte, from = 0) => {
        const at = data.subarray(head + from, tail).indexOf(byte & 255);
        return at < 0 ? -1 : at + from;
      },
      peek: () => okioBuffer(data.slice(head, tail)),
      copy: () => okioBuffer(data.slice(head, tail)),
      clone: () => okioBuffer(data.slice(head, tail)),
      clear: () => {
        head = 0;
        tail = 0;
        return KT.unit;
      },
      snapshot: () => byteString(data.slice(head, tail)),
      outputStream: () => outputStreamTo(buffer),
      inputStream: () => inputStream(take()),
      asResponseBody: (type = null) => bytesBody(async () => data.slice(head, tail), type),
      buffer: () => buffer,
      emit: () => buffer,
      flush: () => KT.unit,
      close: () => KT.unit,
      use: async (lambda) => lambda(buffer),
    };
    return buffer;
  };
  /// okio's `ByteString`: bytes kept whole.
  const byteString = (bytes) => ({
    __class: 'ByteString',
    __classes: ['ByteString'],
    u8Now: () => bytes,
    get size() {
      return bytes.length;
    },
    toByteArray: () => listOfBytes(bytes),
    utf8: () => K.bin.text(bytes),
    base64: () => K.bin.toBase64(bytes),
    base64Url: () => K.bin.toBase64(bytes).replace(/\+/g, '-').replace(/\//g, '_'),
    hex: () => hexOf(listOfBytes(bytes)),
    string: () => K.bin.text(bytes),
    get: (index) => (bytes[index] << 24) >> 24,
    // okio's digests and MACs: a `ByteString` of the digest (`key.encodeUtf8().sha256().hex()`).
    md5: () => byteString(u8Of(digestOf('md5', listOfBytes(bytes)))),
    sha1: () => byteString(u8Of(digestOf('sha1', listOfBytes(bytes)))),
    sha256: () => byteString(u8Of(digestOf('sha256', listOfBytes(bytes)))),
    sha512: () => byteString(u8Of(digestOf('sha512', listOfBytes(bytes)))),
    hmacSha256: (key) =>
      byteString(
        u8Of(
          fromBase64(
            crypto({
              op: 'hmac',
              algorithm: 'sha256',
              key: base64Of(key && key.u8Now ? listOfBytes(key.u8Now()) : key),
              data: base64Of(listOfBytes(bytes)),
            }),
          ),
        ),
      ),
  });

  /// Java's `InputStream` over bytes (`ByteArrayInputStream`, a body's `byteStream()`).
  const inputStream = (bytes) => {
    let at = 0;
    const stream = {
      __class: 'InputStream',
      __classes: ['InputStream', 'ByteArrayInputStream'],
      u8Now: () => bytes.subarray(at),
      read: (target, offset = 0, count = undefined) => {
        if (target === undefined) return at < bytes.length ? bytes[at++] : -1;
        const wanted = count === undefined ? target.length - offset : count;
        const length = Math.min(wanted, bytes.length - at);
        if (length <= 0) return wanted === 0 ? 0 : -1;
        for (let index = 0; index < length; index++) target[offset + index] = bytes[at + index];
        at += length;
        return length;
      },
      readBytes: () => {
        const rest = bytes.subarray(at);
        at = bytes.length;
        return listOfBytes(rest);
      },
      readAllBytes: () => stream.readBytes(),
      readNBytes: (count) => {
        const part = bytes.subarray(at, at + count);
        at += part.length;
        return listOfBytes(part);
      },
      available: () => bytes.length - at,
      skip: (count) => {
        const length = Math.max(0, Math.min(Number(count), bytes.length - at));
        at += length;
        return length;
      },
      copyTo: async (out) => {
        const rest = bytes.subarray(at);
        at = bytes.length;
        await KT.m(out, 'write', [rest]);
        return rest.length;
      },
      source: () => okioBuffer(bytes.subarray(at)),
      buffered: () => stream,
      reader: () => textReader(K.bin.text(bytes.subarray(at))),
      bufferedReader: () => textReader(K.bin.text(bytes.subarray(at))),
      close: () => KT.unit,
      use: async (lambda) => lambda(stream),
    };
    return stream;
  };

  /// Java's `OutputStream` into a buffer (`ByteArrayOutputStream`, okio's `outputStream()`).
  const outputStreamTo = (buffer) => {
    const stream = {
      __class: 'OutputStream',
      __classes: ['OutputStream', 'ByteArrayOutputStream'],
      u8Now: () => buffer.u8Now(),
      write: (value, offset = 0, count = undefined) => (
        buffer.write(value, offset, count),
        KT.unit
      ),
      toByteArray: () => listOfBytes(buffer.u8Now()),
      size: () => buffer.size,
      reset: () => buffer.clear(),
      writeTo: async (out) => KT.m(out, 'write', [buffer.u8Now()]),
      toString: () => K.bin.text(buffer.u8Now()),
      flush: () => KT.unit,
      close: () => KT.unit,
      use: async (lambda) => lambda(stream),
    };
    return stream;
  };

  /// Java's `ByteBuffer` over a `ByteArray` (shared, as `wrap` shares it).
  const byteBuffer = (array, offset = 0, length = undefined) => {
    let order = 'BIG_ENDIAN';
    let position = offset;
    let limit = length === undefined ? array.length : offset + length;
    const little = () => order === 'LITTLE_ENDIAN';
    const read = (index, size) => {
      if (index + size > limit) throw new KotlinError('BufferUnderflowException');
      const bytes = [];
      for (let at = 0; at < size; at++) bytes.push(array[index + at] & 255);
      return bytes;
    };
    const write = (index, bytes) => {
      for (let at = 0; at < bytes.length; at++) array[index + at] = bytes[at];
    };
    const getter = (size) => (index) => {
      const at = index === undefined ? position : offset + index;
      const value = signedOf(read(at, size), little());
      if (index === undefined) position += size;
      return value;
    };
    const putter = (size) => (first, second) => {
      const relative = second === undefined;
      const at = relative ? position : offset + first;
      write(at, bytesOfNumber(relative ? first : second, size, little()));
      if (relative) position += size;
      return buffer;
    };
    const view = (bytes) => new DataView(Uint8Array.from(bytes).buffer);
    const buffer = {
      __class: 'ByteBuffer',
      __classes: ['ByteBuffer'],
      order: (next) => {
        if (next === undefined) return order;
        order = KT.str(next);
        return buffer;
      },
      get: (first, second, third) => {
        if (Array.isArray(first)) {
          const from = second ?? 0;
          const count = third ?? first.length - from;
          const bytes = read(position, count);
          for (let at = 0; at < count; at++) first[from + at] = bytes[at];
          position += count;
          return buffer;
        }
        return getter(1)(first);
      },
      getShort: getter(2),
      getInt: getter(4),
      getLong: getter(8),
      getFloat: (index) => {
        const at = index === undefined ? position : offset + index;
        const value = view(read(at, 4)).getFloat32(0, little());
        if (index === undefined) position += 4;
        return value;
      },
      getDouble: (index) => {
        const at = index === undefined ? position : offset + index;
        const value = view(read(at, 8)).getFloat64(0, little());
        if (index === undefined) position += 8;
        return value;
      },
      put: (first, second) => {
        if (Array.isArray(first) || first instanceof Uint8Array) {
          const bytes = listOfBytes(u8Of(first));
          write(position, bytes);
          position += bytes.length;
          return buffer;
        }
        return putter(1)(first, second);
      },
      putShort: putter(2),
      putInt: putter(4),
      putLong: putter(8),
      array: () => array,
      arrayOffset: () => offset,
      position: (next) => {
        if (next === undefined) return position - offset;
        position = offset + next;
        return buffer;
      },
      limit: (next) => {
        if (next === undefined) return limit - offset;
        limit = offset + next;
        return buffer;
      },
      capacity: () => array.length - offset,
      remaining: () => limit - position,
      hasRemaining: () => position < limit,
      rewind: () => ((position = offset), buffer),
      flip: () => ((limit = position), (position = offset), buffer),
      clear: () => ((position = offset), (limit = array.length), buffer),
    };
    return buffer;
  };

  /// A body's bytes: as they came, a made body's, or its text's UTF-8.
  const bodyU8 = async (body) => {
    if (body === null || body === undefined) return new Uint8Array(0);
    if (typeof body.u8 === 'function') return body.u8();
    if (typeof body.u8Now === 'function') return body.u8Now();
    return K.bin.utf8(KT.str(await body.string()));
  };
  /// A response body of bytes: `load` gives them when first asked, and `text` the text when it's
  /// read another way (a page's, decoded by Raygun).
  const bytesBody = (load, type = null, text = null) => {
    let cached = null;
    const u8 = async () => (cached ??= await load());
    const string = text ?? (async () => K.bin.text(await u8()));
    const body = {
      __class: 'ResponseBody',
      __classes: ['ResponseBody'],
      isResponseBody: true,
      u8,
      string,
      text: string,
      bytes: async () => listOfBytes(await u8()),
      byteStream: async () => inputStream(await u8()),
      source: async () => okioBuffer(await u8()),
      byteString: async () => byteString(await u8()),
      charStream: async () => textReader(await string()),
      close: () => KT.unit,
      contentType: () => type,
      contentLength: async () => (await u8()).length,
      use: async (lambda) => lambda(body),
    };
    return body;
  };
  KT.bytesBody = bytesBody;

  /// An answer (or a body) decrypted by a cipher; `aesDecryptWithPrependedIv` takes the IV from the
  /// body's first bytes and makes the cipher with it.
  const decryptAnswer = async (receiver, name, args, lambda, given = null) => {
    const body = receiver instanceof Response ? receiver.body : receiver;
    let bytes = await bodyU8(body);
    let cipher;
    let type = null;
    if (name === 'aesDecrypt') {
      [cipher = given?.cipher, type = given?.contentType ?? null] = args;
    } else {
      // `(ivSize, contentType) { iv -> cipher }` or `(key, ivSize, contentType)` for AES-CBC,
      // by position or by name.
      const options = { ...named(args), ...(given ?? {}) };
      const plain = args.filter((arg) => !(arg && arg[NAMED]));
      const key =
        options.key ??
        (Array.isArray(plain[0]) || plain[0] instanceof Uint8Array ? plain.shift() : null);
      const ivSize = Number(options.ivSize ?? plain.shift());
      type = options.contentType ?? plain.find((arg) => typeof arg === 'string') ?? null;
      const make =
        options.cipher ?? lambda ?? plain.find((arg) => typeof arg === 'function') ?? null;
      const iv = bytes.subarray(0, ivSize);
      bytes = bytes.subarray(ivSize);
      cipher = make
        ? await make(listOfBytes(iv))
        : aesCipher({ key: bytesOf(await key), iv: [...iv], encrypt: false });
    }
    const plain = cipher.doFinalU8 ? cipher.doFinalU8(bytes) : u8Of(await cipher.doFinal(bytes));
    const next = bytesBody(
      async () => plain,
      type ?? (body.contentType ? body.contentType() : null),
    );
    if (!(receiver instanceof Response)) return next;
    const builder = await KT.m(receiver, 'newBuilder', []);
    await KT.m(builder, 'body', [next]);
    return KT.m(builder, 'build', []);
  };

  // ---- Protobuf: kotlinx.serialization's, as Keiyoushi's `parseAsProto` reads it ----
  // A class's fields carry their numbers (`@ProtoNumber`, the definition's `proto`); a message
  // is read into the field values JSON would give, then made into the class as JSON is.

  /// A message's fields by number: each occurrence `[wire type, value]` (varints as `BigInt`s,
  /// the rest as their bytes).
  const protoFields = (bytes) => {
    const fields = new Map();
    let at = 0;
    const varint = () => {
      let result = 0n;
      let shift = 0n;
      for (;;) {
        if (at >= bytes.length)
          throw new KotlinError('truncated protobuf', 'SerializationException');
        const byte = bytes[at++];
        result |= BigInt(byte & 127) << shift;
        if (byte < 128) return result;
        shift += 7n;
      }
    };
    while (at < bytes.length) {
      const key = Number(varint());
      const number = Math.floor(key / 8);
      const wire = key & 7;
      let value;
      if (wire === 0) value = varint();
      else if (wire === 1 || wire === 5) {
        const size = wire === 1 ? 8 : 4;
        value = bytes.subarray(at, at + size);
        at += size;
      } else if (wire === 2) {
        const length = Number(varint());
        value = bytes.subarray(at, at + length);
        at += length;
      } else throw new KotlinError(`protobuf wire type ${wire}`, 'SerializationException');
      if (!fields.has(number)) fields.set(number, []);
      fields.get(number).push([wire, value]);
    }
    return fields;
  };
  const PROTO_WHOLE = new Set(['Int', 'Long', 'Short', 'Byte', 'UInt', 'ULong', 'Char']);
  const littleView = (bytes) => new DataView(Uint8Array.from(bytes).buffer);
  /// A scalar of `type` from one occurrence (`kind`: `@ProtoType`'s `SIGNED` or `FIXED`).
  const protoScalar = (wire, value, type, kind) => {
    const name = type.name;
    if (name === 'String') return K.bin.text(value);
    if (name === 'ByteArray') return listOfBytes(value);
    if (name === 'Boolean') return wire === 0 ? value !== 0n : Boolean(value[0]);
    if (name === 'Double')
      return wire === 1 ? littleView(value).getFloat64(0, true) : Number(BigInt.asIntN(64, value));
    if (name === 'Float')
      return wire === 5 ? littleView(value).getFloat32(0, true) : Number(BigInt.asIntN(32, value));
    let whole;
    if (wire === 0) whole = value;
    else if (wire === 5) whole = BigInt(littleView(value).getUint32(0, true));
    else if (wire === 1) whole = littleView(value).getBigUint64(0, true);
    else return null;
    if (kind === 'SIGNED' && wire === 0) whole = (whole >> 1n) ^ -(whole & 1n);
    const bits = name === 'Long' || name === 'ULong' ? 64 : 32;
    return Number(name.startsWith('U') ? BigInt.asUintN(bits, whole) : BigInt.asIntN(bits, whole));
  };
  /// The values packed in one occurrence of a repeated number (`[1, 2, 3]` written together).
  const protoPacked = (bytes, type, kind) => {
    const out = [];
    if (type.name === 'Double' || (type.name === 'Long' && kind === 'FIXED')) {
      for (let at = 0; at + 8 <= bytes.length; at += 8)
        out.push(protoScalar(1, bytes.subarray(at, at + 8), type, kind));
      return out;
    }
    if (type.name === 'Float' || kind === 'FIXED') {
      for (let at = 0; at + 4 <= bytes.length; at += 4)
        out.push(protoScalar(5, bytes.subarray(at, at + 4), type, kind));
      return out;
    }
    let at = 0;
    while (at < bytes.length) {
      let result = 0n;
      let shift = 0n;
      for (;;) {
        const byte = bytes[at++];
        result |= BigInt(byte & 127) << shift;
        if (byte < 128 || at >= bytes.length) break;
        shift += 7n;
      }
      out.push(protoScalar(0, result, type, kind));
    }
    return out;
  };
  const protoOne = (wire, value, type, kind) => {
    const holder = classOf(type);
    if (holder && holder.definition) {
      // An enum by its number (its place, as kotlinx writes it).
      if (holder.definition.kind === 'enum')
        return holder.definition.entries[Number(value)] ?? holder.definition.entries[0] ?? null;
      return protoMessage(value, holder, type.args);
    }
    return protoScalar(wire, value, type, kind);
  };
  const protoValue = (occurrences, type, kind) => {
    if (LISTS.has(type.name)) {
      const item = type.args[0] ?? { name: 'Any', args: [] };
      const out = [];
      for (const [wire, value] of occurrences) {
        if (
          wire === 2 &&
          (PROTO_WHOLE.has(item.name) || ['Boolean', 'Double', 'Float'].includes(item.name))
        )
          out.push(...protoPacked(value, item, kind));
        else out.push(protoOne(wire, value, item, kind));
      }
      return out;
    }
    if (['Map', 'HashMap', 'MutableMap', 'LinkedHashMap'].includes(type.name)) {
      const [keyType = { name: 'String', args: [] }, valueType = { name: 'Any', args: [] }] =
        type.args;
      const out = {};
      for (const [, value] of occurrences) {
        const entry = protoFields(value);
        const key = entry.get(1)?.at(-1);
        const item = entry.get(2)?.at(-1);
        out[KT.str(key ? protoOne(key[0], key[1], keyType, null) : '')] = item
          ? protoOne(item[0], item[1], valueType, null)
          : null;
      }
      return out;
    }
    const [wire, value] = occurrences[occurrences.length - 1];
    return protoOne(wire, value, type, kind);
  };
  /// A message's fields as JSON would give them, by name.
  const protoMessage = (bytes, holder, typeArgs = []) => {
    const definition = holder.definition;
    const fields = protoFields(bytes);
    const out = {};
    for (const name of definition.params) {
      const spec = definition.proto?.[name];
      const number = spec ? spec[0] : definition.params.indexOf(name) + 1;
      const occurrences = fields.get(number);
      if (!occurrences) continue;
      const type = substitute(
        parseType(definition.types?.[name] ?? 'Any'),
        definition.typeParams ?? [],
        typeArgs,
      );
      out[name] = protoValue(occurrences, type, spec ? spec[1] : null);
    }
    return out;
  };
  /// Bytes read as the class `type` names (`parseAsProto<MangaPlusResponse>()`).
  const decodeProtoAs = async (bytes, type) => {
    const parsed = parseType(type);
    const holder = classOf(parsed);
    if (!holder || !holder.definition)
      throw new KotlinError(`Raygun doesn't know the class ${type}`, 'SerializationException');
    return decode(protoMessage(bytes, holder, parsed.args), type);
  };

  const protoVarint = (value, out) => {
    let rest = BigInt.asUintN(64, BigInt(value));
    do {
      let byte = Number(rest & 127n);
      rest >>= 7n;
      if (rest) byte |= 128;
      out.push(byte);
    } while (rest);
  };
  const protoKey = (number, wire, out) => protoVarint(number * 8 + wire, out);
  /// A value of `type` written under `number` (repeated ones one by one).
  const protoWrite = (number, value, type, kind, out) => {
    if (value === null || value === undefined) return;
    if (LISTS.has(type.name) && Array.isArray(value)) {
      for (const item of value)
        protoWrite(number, item, type.args[0] ?? { name: 'Any', args: [] }, kind, out);
      return;
    }
    const holder = classOf(type);
    if (holder && holder.definition && holder.definition.kind === 'enum') {
      protoKey(number, 0, out);
      protoVarint(
        typeof value === 'object'
          ? (value.ordinal ?? 0)
          : holder.definition.entries.indexOf(KT.str(value)),
        out,
      );
      return;
    }
    if (holder && holder.definition) {
      const bytes = protoEncode(value, holder);
      protoKey(number, 2, out);
      protoVarint(bytes.length, out);
      out.push(...bytes);
      return;
    }
    const name = type.name;
    if (name === 'String' || typeof value === 'string') {
      const bytes = K.bin.utf8(KT.str(value));
      protoKey(number, 2, out);
      protoVarint(bytes.length, out);
      out.push(...bytes);
    } else if (name === 'ByteArray' || Array.isArray(value)) {
      const bytes = u8Of(value);
      protoKey(number, 2, out);
      protoVarint(bytes.length, out);
      out.push(...bytes);
    } else if (name === 'Double') {
      const view = new DataView(new ArrayBuffer(8));
      view.setFloat64(0, Number(value), true);
      protoKey(number, 1, out);
      out.push(...new Uint8Array(view.buffer));
    } else if (name === 'Float') {
      const view = new DataView(new ArrayBuffer(4));
      view.setFloat32(0, Number(value), true);
      protoKey(number, 5, out);
      out.push(...new Uint8Array(view.buffer));
    } else if (typeof value === 'boolean') {
      protoKey(number, 0, out);
      protoVarint(value ? 1 : 0, out);
    } else if (kind === 'FIXED') {
      const size = name === 'Long' || name === 'ULong' ? 8 : 4;
      protoKey(number, size === 8 ? 1 : 5, out);
      out.push(...bytesOfNumber(value, size, true));
    } else {
      protoKey(number, 0, out);
      const whole = BigInt(Math.trunc(Number(value)));
      protoVarint(kind === 'SIGNED' ? (whole << 1n) ^ (whole >> 63n) : whole, out);
    }
  };
  /// An instance of a class the source declares, written as a protobuf message.
  const protoEncode = (instance, holder = null) => {
    const owner = holder ?? CLASSES[instance?.__class];
    if (!owner || !owner.definition)
      throw new KotlinError("Raygun can't write this value as protobuf", 'SerializationException');
    const definition = owner.definition;
    const out = [];
    for (const name of definition.params) {
      const spec = definition.proto?.[name];
      const number = spec ? spec[0] : definition.params.indexOf(name) + 1;
      protoWrite(
        number,
        instance[name],
        parseType(definition.types?.[name] ?? 'Any'),
        spec ? spec[1] : null,
        out,
      );
    }
    return Uint8Array.from(out);
  };
  /// The bytes a protobuf decoder reads from its receiver: an answer, a body, a buffer, a
  /// `ByteArray`, base64 text.
  const protoBytesOf = async (receiver, name) => {
    if (name === 'decodeProtoBase64') return K.bin.fromBase64(KT.str(receiver));
    if (receiver instanceof Response) return bodyU8(receiver.body);
    if (receiver && receiver.isResponseBody) return bodyU8(receiver);
    return u8Of(receiver);
  };
  const PROTO_DECODERS = new Set(['parseAsProto', 'decodeProto', 'decodeProtoBase64']);

  // ---- Pictures: Android's `Bitmap`, `BitmapFactory`, `Canvas`, held in Rust ----

  /// An `ARGB` colour int (Kotlin's are signed) as Rust takes it.
  const argbOf = (color) => Number(color ?? 0) >>> 0;
  const IDENTITY = [1, 0, 0, 1, 0, 0];
  /// `m` after `n`, both `[a, b, c, d, e, f]` (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`).
  const mul = (m, n) => [
    m[0] * n[0] + m[2] * n[1],
    m[1] * n[0] + m[3] * n[1],
    m[0] * n[2] + m[2] * n[3],
    m[1] * n[2] + m[3] * n[3],
    m[0] * n[4] + m[2] * n[5] + m[4],
    m[1] * n[4] + m[3] * n[5] + m[5],
  ];
  const translation = (dx, dy) => [1, 0, 0, 1, Number(dx) || 0, Number(dy) || 0];
  const about = (matrix, px, py) =>
    px === undefined ? matrix : mul(translation(px, py), mul(matrix, translation(-px, -py)));
  const scaling = (sx, sy, px, py) => about([sx, 0, 0, sy ?? sx, 0, 0], px, py);
  /// A clockwise turn in degrees, exact for quarter turns.
  const rotation = (degrees, px, py) => {
    const radians = (Number(degrees) * Math.PI) / 180;
    const exact = (value) =>
      Math.abs(value - Math.round(value)) < 1e-12 ? Math.round(value) : value;
    const cos = exact(Math.cos(radians));
    const sin = exact(Math.sin(radians));
    return about([cos, sin, -sin, cos, 0, 0], px, py);
  };

  class Bitmap {
    constructor(picture) {
      this.__class = 'Bitmap';
      this.__classes = ['Bitmap'];
      this.id = picture.id;
      this.width = picture.width;
      this.height = picture.height;
      this.isRecycled = false;
      this.isMutable = true;
      this.config = 'ARGB_8888';
      this.density = 160;
    }
    getWidth() {
      return this.width;
    }
    getHeight() {
      return this.height;
    }
    getConfig() {
      return this.config;
    }
    getByteCount() {
      return this.width * this.height * 4;
    }
    getRowBytes() {
      return this.width * 4;
    }
    hasAlpha() {
      return true;
    }
    setHasAlpha() {
      return KT.unit;
    }
    setDensity() {
      return KT.unit;
    }
    recycle() {
      if (!this.isRecycled) {
        K.bitmap.run('free', { id: this.id });
        this.isRecycled = true;
      }
      return KT.unit;
    }
    compress(format, quality, stream) {
      stream.write(K.bitmap.encode(this.id, KT.str(format), Number(quality)));
      return true;
    }
    copy() {
      return new Bitmap(K.bitmap.run('copy', { id: this.id }));
    }
    getPixel(x, y) {
      return K.bitmap.run('pixels', { id: this.id, x, y, width: 1, height: 1 })[0];
    }
    setPixel(x, y, color) {
      K.bitmap.run('setPixels', { id: this.id, x, y, width: 1, height: 1, colors: [color] });
      return KT.unit;
    }
    getPixels(pixels, offset, stride, x, y, width, height) {
      const colors = K.bitmap.run('pixels', { id: this.id, x, y, width, height });
      for (let row = 0; row < height; row++)
        for (let column = 0; column < width; column++)
          pixels[offset + row * stride + column] = colors[row * width + column];
      return KT.unit;
    }
    setPixels(pixels, offset, stride, x, y, width, height) {
      const colors = [];
      for (let row = 0; row < height; row++)
        for (let column = 0; column < width; column++)
          colors.push(pixels[offset + row * stride + column] ?? 0);
      K.bitmap.run('setPixels', { id: this.id, x, y, width, height, colors });
      return KT.unit;
    }
    eraseColor(color) {
      K.bitmap.run('fill', { id: this.id, color: argbOf(color), replace: true });
      return KT.unit;
    }
  }

  /// A picture read from its file's bytes; null when it can't be read, as Android answers.
  const decodeBitmap = (bytes, options = null) => {
    let picture;
    try {
      picture = K.bitmap.decode(u8Of(bytes));
    } catch (error) {
      raygun.log.warn(`a picture couldn't be read: ${error.message}`);
      return null;
    }
    let bitmap = new Bitmap(picture);
    if (options && typeof options === 'object') {
      options.outWidth = bitmap.width;
      options.outHeight = bitmap.height;
      if (KT.truthy(options.inJustDecodeBounds)) {
        bitmap.recycle();
        return null;
      }
      const sample = Number(options.inSampleSize ?? 1);
      if (sample > 1) {
        const scaled = K.bitmap.run('scaled', {
          id: bitmap.id,
          width: Math.max(1, Math.floor(bitmap.width / sample)),
          height: Math.max(1, Math.floor(bitmap.height / sample)),
          filter: true,
        });
        bitmap.recycle();
        bitmap = new Bitmap(scaled);
      }
    }
    return bitmap;
  };
  const streamBytes = async (stream) => {
    if (stream === null || stream === undefined) return null;
    if (typeof stream.u8Now === 'function') return stream.u8Now();
    return bodyU8(stream);
  };
  const bitmapFactory = {
    decodeStream: async (stream, _padding = null, options = null) => {
      const bytes = await streamBytes(stream);
      return bytes === null ? null : decodeBitmap(bytes, options ?? _padding);
    },
    decodeByteArray: (bytes, offset = 0, length = undefined, options = null) => {
      const all = u8Of(bytes);
      return decodeBitmap(
        all.subarray(offset, length === undefined ? all.length : offset + length),
        options,
      );
    },
    Options: () => ({
      __class: 'Options',
      inJustDecodeBounds: false,
      inMutable: false,
      inSampleSize: 1,
      inPreferredConfig: null,
      outWidth: -1,
      outHeight: -1,
      outMimeType: null,
    }),
  };
  /// Keiyoushi's `lib:imagedecoder` (`ImageDecoder.decode(bytes)`) and Android's own
  /// (`ImageDecoder.decodeBitmap(ImageDecoder.createSource(bytes)) { … }`): Raygun's decoder.
  const imageDecoder = {
    decode: (bytes) => {
      const bitmap = decodeBitmap(bytes);
      if (!bitmap) throw new KotlinError('No decoder could read the image', 'IOException');
      return bitmap;
    },
    createSource: (bytes) => ({ __class: 'Source', bytes }),
    decodeBitmap: async (source, listener = null) => {
      const bitmap = decodeBitmap(source && source.bytes !== undefined ? source.bytes : source);
      if (!bitmap) throw new KotlinError('No decoder could read the image', 'IOException');
      // The listener sets the decoder's options (`allocator`), which change nothing here.
      if (typeof listener === 'function') await listener({}, {}, source);
      return bitmap;
    },
    ALLOCATOR_DEFAULT: 0,
    ALLOCATOR_SOFTWARE: 1,
    ALLOCATOR_SHARED_MEMORY: 2,
    ALLOCATOR_HARDWARE: 3,
  };
  const bitmapStatics = {
    Config: {
      ARGB_8888: 'ARGB_8888',
      RGB_565: 'RGB_565',
      ALPHA_8: 'ALPHA_8',
      ARGB_4444: 'ARGB_4444',
      RGBA_F16: 'RGBA_F16',
      HARDWARE: 'HARDWARE',
    },
    CompressFormat: {
      JPEG: 'JPEG',
      PNG: 'PNG',
      WEBP: 'WEBP',
      WEBP_LOSSY: 'WEBP_LOSSY',
      WEBP_LOSSLESS: 'WEBP_LOSSLESS',
    },
    createBitmap: (...args) => {
      // A display's metrics first changes nothing.
      if (
        args[0] &&
        typeof args[0] === 'object' &&
        args[0].density !== undefined &&
        !(args[0] instanceof Bitmap)
      )
        args.shift();
      const [first] = args;
      if (typeof first === 'number')
        return new Bitmap(K.bitmap.run('create', { width: args[0], height: args[1], color: 0 }));
      if (Array.isArray(first)) {
        // (colors, [offset, stride,] width, height, config)
        const [colors, ...rest] = args;
        const [offset, stride, width, height] =
          typeof rest[2] === 'number' ? rest : [0, rest[0], rest[0], rest[1]];
        const bitmap = new Bitmap(K.bitmap.run('create', { width, height, color: 0 }));
        bitmap.setPixels(colors, offset, stride, 0, 0, width, height);
        return bitmap;
      }
      if (first instanceof Bitmap) {
        if (args.length === 1) return first.copy();
        const [source, x, y, width, height, matrix] = args;
        if (matrix && matrix.isMatrix && !matrix.isIdentity()) {
          // The part through the matrix, in a picture as large as its bounds.
          const corners = [
            [0, 0],
            [width, 0],
            [0, height],
            [width, height],
          ].map(([px, py]) => {
            const [a, b, c, d, e, f] = matrix.values;
            return [a * px + c * py + e, b * px + d * py + f];
          });
          const left = Math.min(...corners.map((corner) => corner[0]));
          const top = Math.min(...corners.map((corner) => corner[1]));
          const right = Math.max(...corners.map((corner) => corner[0]));
          const bottom = Math.max(...corners.map((corner) => corner[1]));
          const target = new Bitmap(
            K.bitmap.run('create', {
              width: Math.max(1, Math.round(right - left)),
              height: Math.max(1, Math.round(bottom - top)),
              color: 0,
            }),
          );
          K.bitmap.run('draw', {
            target: target.id,
            source: source.id,
            part: [x, y, x + width, y + height],
            matrix: mul(translation(-left, -top), mul(matrix.values, translation(-x, -y))),
          });
          return target;
        }
        return new Bitmap(K.bitmap.run('crop', { id: source.id, x, y, width, height }));
      }
      throw new KotlinError("Bitmap.createBitmap wasn't given a size or a picture");
    },
    createScaledBitmap: (source, width, height, filter = true) =>
      new Bitmap(
        K.bitmap.run('scaled', { id: source.id, width, height, filter: KT.truthy(filter) }),
      ),
  };

  /// Android's `Matrix`: an affine map, built by `pre…` (applied first) and `post…` (after).
  class Matrix {
    constructor(other = null) {
      this.__class = 'Matrix';
      this.__classes = ['Matrix'];
      this.isMatrix = true;
      this.values = other && other.values ? [...other.values] : [...IDENTITY];
    }
    isIdentity() {
      return this.values.every((value, index) => value === IDENTITY[index]);
    }
    reset() {
      this.values = [...IDENTITY];
      return KT.unit;
    }
    set(other) {
      this.values = other ? [...other.values] : [...IDENTITY];
      return KT.unit;
    }
    setTranslate(dx, dy) {
      this.values = translation(dx, dy);
      return KT.unit;
    }
    setScale(sx, sy, px, py) {
      this.values = scaling(sx, sy, px, py);
      return KT.unit;
    }
    setRotate(degrees, px, py) {
      this.values = rotation(degrees, px, py);
      return KT.unit;
    }
    preTranslate(dx, dy) {
      this.values = mul(this.values, translation(dx, dy));
      return true;
    }
    preScale(sx, sy, px, py) {
      this.values = mul(this.values, scaling(sx, sy, px, py));
      return true;
    }
    preRotate(degrees, px, py) {
      this.values = mul(this.values, rotation(degrees, px, py));
      return true;
    }
    preConcat(other) {
      this.values = mul(this.values, other.values);
      return true;
    }
    postTranslate(dx, dy) {
      this.values = mul(translation(dx, dy), this.values);
      return true;
    }
    postScale(sx, sy, px, py) {
      this.values = mul(scaling(sx, sy, px, py), this.values);
      return true;
    }
    postRotate(degrees, px, py) {
      this.values = mul(rotation(degrees, px, py), this.values);
      return true;
    }
    postConcat(other) {
      this.values = mul(other.values, this.values);
      return true;
    }
  }

  /// Android's `Rect` and `RectF`.
  class Rect {
    constructor(...args) {
      this.__class = 'Rect';
      this.__classes = ['Rect', 'RectF'];
      this.set(...args);
    }
    set(...args) {
      const [first] = args;
      if (first && typeof first === 'object')
        [this.left, this.top, this.right, this.bottom] = [
          first.left,
          first.top,
          first.right,
          first.bottom,
        ];
      else if (args.length >= 4) [this.left, this.top, this.right, this.bottom] = args.map(Number);
      else [this.left, this.top, this.right, this.bottom] = [0, 0, 0, 0];
      return KT.unit;
    }
    width() {
      return this.right - this.left;
    }
    height() {
      return this.bottom - this.top;
    }
    centerX() {
      return (this.left + this.right) / 2;
    }
    centerY() {
      return (this.top + this.bottom) / 2;
    }
    exactCenterX() {
      return (this.left + this.right) / 2;
    }
    exactCenterY() {
      return (this.top + this.bottom) / 2;
    }
    isEmpty() {
      return this.left >= this.right || this.top >= this.bottom;
    }
    setEmpty() {
      return this.set();
    }
    offset(dx, dy) {
      this.left += dx;
      this.right += dx;
      this.top += dy;
      this.bottom += dy;
      return KT.unit;
    }
    offsetTo(x, y) {
      return this.offset(x - this.left, y - this.top);
    }
    inset(dx, dy) {
      this.left += dx;
      this.right -= dx;
      this.top += dy;
      this.bottom -= dy;
      return KT.unit;
    }
    contains(x, y) {
      return x >= this.left && x < this.right && y >= this.top && y < this.bottom;
    }
    intersect(...args) {
      const other = new Rect(...args);
      if (
        other.left >= this.right ||
        this.left >= other.right ||
        other.top >= this.bottom ||
        this.top >= other.bottom
      )
        return false;
      this.set(
        Math.max(this.left, other.left),
        Math.max(this.top, other.top),
        Math.min(this.right, other.right),
        Math.min(this.bottom, other.bottom),
      );
      return true;
    }
    union(...args) {
      const other = new Rect(...args);
      this.set(
        Math.min(this.left, other.left),
        Math.min(this.top, other.top),
        Math.max(this.right, other.right),
        Math.max(this.bottom, other.bottom),
      );
      return KT.unit;
    }
  }
  const rectFrom = (args) =>
    args.length >= 4 ? new Rect(...args.slice(0, 4)) : args[0] ? new Rect(args[0]) : null;

  /// Android's `Paint` (and `TextPaint`): what a fill or text is drawn with.
  const makePaint = (from = null) => {
    const paint = {
      __class: 'Paint',
      __classes: ['Paint', 'TextPaint'],
      color: from && typeof from === 'object' ? from.color : -16777216,
      alpha: 255,
      style: 'FILL',
      textSize: from && typeof from === 'object' ? from.textSize : 12,
      typeface: from && typeof from === 'object' ? from.typeface : null,
      isAntiAlias: false,
      isFilterBitmap: false,
      isDither: false,
      setColor: (color) => ((paint.color = color), KT.unit),
      setAlpha: (alpha) => ((paint.alpha = alpha), KT.unit),
      setStyle: (style) => ((paint.style = style), KT.unit),
      setTextSize: (size) => ((paint.textSize = size), KT.unit),
      setTypeface: (typeface) => ((paint.typeface = typeface), typeface),
      setAntiAlias: (on) => ((paint.isAntiAlias = on), KT.unit),
      setFilterBitmap: (on) => ((paint.isFilterBitmap = on), KT.unit),
      setDither: (on) => ((paint.isDither = on), KT.unit),
      setXfermode: (mode) => ((paint.xfermode = mode), mode),
      setColorFilter: (filter) => ((paint.colorFilter = filter), filter),
    };
    return paint;
  };
  const paintStatics = {
    Style: { FILL: 'FILL', STROKE: 'STROKE', FILL_AND_STROKE: 'FILL_AND_STROKE' },
    Align: { LEFT: 'LEFT', CENTER: 'CENTER', RIGHT: 'RIGHT' },
    Join: { MITER: 'MITER', ROUND: 'ROUND', BEVEL: 'BEVEL' },
    Cap: { BUTT: 'BUTT', ROUND: 'ROUND', SQUARE: 'SQUARE' },
    ANTI_ALIAS_FLAG: 1,
    FILTER_BITMAP_FLAG: 2,
    DITHER_FLAG: 4,
  };
  const colorStatics = {
    BLACK: -16777216,
    DKGRAY: -12303292,
    GRAY: -7829368,
    LTGRAY: -3355444,
    WHITE: -1,
    RED: -65536,
    GREEN: -16711936,
    BLUE: -16776961,
    YELLOW: -256,
    CYAN: -16711681,
    MAGENTA: -65281,
    TRANSPARENT: 0,
    rgb: (red, green, blue) => 0xff000000 | (red << 16) | (green << 8) | blue | 0,
    argb: (alpha, red, green, blue) => (alpha << 24) | (red << 16) | (green << 8) | blue | 0,
    alpha: (color) => (color >>> 24) & 255,
    red: (color) => (color >> 16) & 255,
    green: (color) => (color >> 8) & 255,
    blue: (color) => color & 255,
    parseColor: (text) => {
      const value = KT.str(text).trim();
      const named = {
        black: -16777216,
        white: -1,
        red: -65536,
        green: -16711936,
        blue: -16776961,
        gray: -7829368,
        grey: -7829368,
        yellow: -256,
        cyan: -16711681,
        magenta: -65281,
        transparent: 0,
      }[value.toLowerCase()];
      if (named !== undefined) return named;
      const hex = value.startsWith('#') ? value.slice(1) : null;
      if (hex && /^[0-9a-fA-F]+$/.test(hex) && (hex.length === 6 || hex.length === 8))
        return parseInt(hex.length === 6 ? `ff${hex}` : hex, 16) | 0;
      throw new KotlinError(`Unknown color ${value}`, 'IllegalArgumentException');
    },
  };
  const typefaceStatics = {
    DEFAULT: { __class: 'Typeface', bold: false },
    DEFAULT_BOLD: { __class: 'Typeface', bold: true },
    SANS_SERIF: { __class: 'Typeface', bold: false },
    SERIF: { __class: 'Typeface', bold: false },
    MONOSPACE: { __class: 'Typeface', bold: false },
    NORMAL: 0,
    BOLD: 1,
    ITALIC: 2,
    BOLD_ITALIC: 3,
    create: (_family, style) => ({ __class: 'Typeface', bold: style === 1 || style === 3 }),
    defaultFromStyle: (style) => ({ __class: 'Typeface', bold: style === 1 || style === 3 }),
    // A font file a source fetched: text is drawn in the system's face here.
    createFromFile: () => ({ __class: 'Typeface', bold: false }),
    createFromAsset: () => ({ __class: 'Typeface', bold: false }),
  };

  /// Android's `Canvas` over a picture: draws through its matrix, inside its clip.
  class Canvas {
    constructor(bitmap = null) {
      this.__class = 'Canvas';
      this.__classes = ['Canvas'];
      this.bitmap = bitmap;
      this.matrix = [...IDENTITY];
      this.clip = null;
      this.saved = [];
    }
    get width() {
      return this.bitmap ? this.bitmap.width : 0;
    }
    get height() {
      return this.bitmap ? this.bitmap.height : 0;
    }
    getWidth() {
      return this.width;
    }
    getHeight() {
      return this.height;
    }
    target() {
      if (!this.bitmap) throw new KotlinError('the canvas has no picture to draw on');
      return this.bitmap.id;
    }
    setBitmap(bitmap) {
      this.bitmap = bitmap;
      return KT.unit;
    }
    save() {
      this.saved.push([this.matrix, this.clip]);
      return this.saved.length;
    }
    getSaveCount() {
      return this.saved.length + 1;
    }
    restore() {
      const [matrix, clip] = this.saved.pop() ?? [[...IDENTITY], null];
      this.matrix = matrix;
      this.clip = clip;
      return KT.unit;
    }
    restoreToCount(count) {
      while (this.saved.length >= Math.max(1, count)) this.restore();
      return KT.unit;
    }
    translate(dx, dy) {
      this.matrix = mul(this.matrix, translation(dx, dy));
      return KT.unit;
    }
    scale(sx, sy, px, py) {
      this.matrix = mul(this.matrix, scaling(sx, sy, px, py));
      return KT.unit;
    }
    rotate(degrees, px, py) {
      this.matrix = mul(this.matrix, rotation(degrees, px, py));
      return KT.unit;
    }
    concat(matrix) {
      if (matrix) this.matrix = mul(this.matrix, matrix.values);
      return KT.unit;
    }
    setMatrix(matrix) {
      this.matrix = matrix ? [...matrix.values] : [...IDENTITY];
      return KT.unit;
    }
    /// A clip of a rectangle, kept as its bounds on the picture.
    clipRect(...args) {
      const rect = rectFrom(args);
      if (!rect) return true;
      const [a, b, c, d, e, f] = this.matrix;
      const xs = [rect.left, rect.right].flatMap((x) =>
        [rect.top, rect.bottom].map((y) => a * x + c * y + e),
      );
      const ys = [rect.left, rect.right].flatMap((x) =>
        [rect.top, rect.bottom].map((y) => b * x + d * y + f),
      );
      const next = [
        Math.floor(Math.min(...xs)),
        Math.floor(Math.min(...ys)),
        Math.ceil(Math.max(...xs)),
        Math.ceil(Math.max(...ys)),
      ];
      this.clip = this.clip
        ? [
            Math.max(this.clip[0], next[0]),
            Math.max(this.clip[1], next[1]),
            Math.min(this.clip[2], next[2]),
            Math.min(this.clip[3], next[3]),
          ]
        : next;
      return this.clip[0] < this.clip[2] && this.clip[1] < this.clip[3];
    }
    /// `drawBitmap(bitmap, left, top, paint)`, `(bitmap, src, dst, paint)`, `(bitmap, matrix, paint)`.
    drawBitmap(bitmap, second, third) {
      let part = [0, 0, bitmap.width, bitmap.height];
      let local;
      if (typeof second === 'number') local = translation(second, third);
      else if (second && second.isMatrix) local = second.values;
      else {
        if (second) part = [second.left, second.top, second.right, second.bottom];
        const destination = third;
        const sx = (destination.right - destination.left) / (part[2] - part[0]);
        const sy = (destination.bottom - destination.top) / (part[3] - part[1]);
        local = [sx, 0, 0, sy, destination.left - part[0] * sx, destination.top - part[1] * sy];
      }
      if (!(part[2] > part[0]) || !(part[3] > part[1])) return KT.unit;
      K.bitmap.run('draw', {
        target: this.target(),
        source: bitmap.id,
        part: part.map((value) => Math.round(value)),
        matrix: mul(this.matrix, local),
        clip: this.clip,
      });
      return KT.unit;
    }
    drawColor(color, mode = null) {
      const replace = ['CLEAR', 'SRC'].includes(KT.str(mode ?? ''));
      K.bitmap.run('fill', {
        id: this.target(),
        color: KT.str(mode ?? '') === 'CLEAR' ? 0 : argbOf(color),
        replace,
      });
      return KT.unit;
    }
    drawARGB(alpha, red, green, blue) {
      return this.drawColor(colorStatics.argb(alpha, red, green, blue));
    }
    drawRGB(red, green, blue) {
      return this.drawColor(colorStatics.rgb(red, green, blue));
    }
    drawPaint(paint) {
      return this.drawColor(paint ? paint.color : colorStatics.BLACK);
    }
    drawRect(...args) {
      const paint = args.pop();
      const rect = rectFrom(args);
      if (!rect) return KT.unit;
      K.bitmap.run('fill', {
        id: this.target(),
        rect: [rect.left, rect.top, rect.right, rect.bottom],
        matrix: this.matrix,
        color: argbOf(paint ? paint.color : colorStatics.BLACK),
      });
      return KT.unit;
    }
  }

  /// Android's `StaticLayout`: text laid out in lines `width` wide with a paint's size and weight,
  /// measured and drawn by the runtime's own text layout (`draw(canvas)` at the canvas's origin,
  /// through its matrix). Lines are left-aligned whatever the alignment asks.
  const staticLayout = ({ text, paint, width, spacingMult = 1, spacingAdd = 0 }) => {
    const content = KT.str(text ?? '');
    const size = Number(paint?.textSize ?? 12);
    const bold = Boolean(paint?.typeface?.bold || paint?.isFakeBoldText);
    const lineWidth = Math.max(1, Number(width) || 1);
    const spacing = Number(spacingMult) || 1;
    const extra = Number(spacingAdd) || 0;
    const measured = content
      ? K.bitmap.run('measureText', { text: content, size, width: lineWidth, spacing, extra, bold })
      : { height: 0, lines: 0 };
    const layout = {
      __class: 'StaticLayout',
      __classes: ['StaticLayout', 'Layout'],
      width: lineWidth,
      height: Math.ceil(measured.height),
      lineCount: measured.lines,
      text: content,
      paint,
      getWidth: () => lineWidth,
      getHeight: () => layout.height,
      getLineCount: () => measured.lines,
      getLineWidth: () => lineWidth,
      getLineTop: (line) => (measured.lines ? (layout.height * line) / measured.lines : 0),
      getLineBottom: (line) => (measured.lines ? (layout.height * (line + 1)) / measured.lines : 0),
      draw: (canvas) => {
        if (content)
          K.bitmap.run('drawText', {
            id: canvas.target(),
            text: content,
            size,
            width: lineWidth,
            spacing,
            extra,
            bold,
            x: 0,
            y: 0,
            matrix: canvas.matrix,
            color: argbOf(paint ? paint.color : colorStatics.BLACK),
          });
        return KT.unit;
      },
    };
    return layout;
  };
  /// `StaticLayout.Builder.obtain(text, start, end, paint, width)…build()`.
  const staticLayoutBuilder = (text, start, end, paint, width) => {
    const parts = {
      text: KT.str(text ?? '').slice(start ?? 0, end ?? undefined),
      paint,
      width,
      spacingMult: 1,
      spacingAdd: 0,
    };
    const builder = {
      setAlignment: () => builder,
      setTextDirection: () => builder,
      setIncludePad: () => builder,
      setEllipsize: () => builder,
      setEllipsizedWidth: () => builder,
      setMaxLines: () => builder,
      setBreakStrategy: () => builder,
      setHyphenationFrequency: () => builder,
      setJustificationMode: () => builder,
      setUseLineSpacingFromFallbacks: () => builder,
      setLineSpacing: (add, mult) => {
        parts.spacingAdd = add;
        parts.spacingMult = mult;
        return builder;
      },
      build: () => staticLayout(parts),
    };
    return builder;
  };
  const layoutStatics = {
    Alignment: {
      ALIGN_NORMAL: 'ALIGN_NORMAL',
      ALIGN_CENTER: 'ALIGN_CENTER',
      ALIGN_OPPOSITE: 'ALIGN_OPPOSITE',
    },
    BREAK_STRATEGY_SIMPLE: 0,
    BREAK_STRATEGY_HIGH_QUALITY: 1,
    BREAK_STRATEGY_BALANCED: 2,
    HYPHENATION_FREQUENCY_NONE: 0,
    HYPHENATION_FREQUENCY_NORMAL: 1,
    HYPHENATION_FREQUENCY_FULL: 2,
    JUSTIFICATION_MODE_NONE: 0,
    JUSTIFICATION_MODE_INTER_WORD: 1,
  };

  /// Text a web comic's notes become, as `lib:textinterceptor` draws them: a white page 1000
  /// pixels wide, the heading in bold above the text.
  const TEXT_HOST = 'tachiyomi-lib-textinterceptor';
  /// Android's `Html.fromHtml(…).toString()`: lines kept, tags dropped, entities read.
  const fromHtml = (html) =>
    unescapeEntities(
      KT.str(html)
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/p\s*>/gi, '\n\n')
        .replace(/<[^>]*>/g, ''),
    ).trim();
  const textPage = async (request) => {
    const segments = request.url.pathSegments;
    const width = 1000;
    const padX = 50;
    const padY = 25;
    const parts = [
      { text: segments[0] ? fromHtml(segments[0]) : '', size: 36, bold: true },
      { text: segments[1] ? fromHtml(segments[1]) : '', size: 30, bold: false },
    ].map((part) => ({
      ...part,
      width: width - 2 * padX,
      spacing: 1.1,
      extra: 2,
      height: part.text
        ? K.bitmap.run('measureText', { ...part, width: width - 2 * padX, spacing: 1.1, extra: 2 })
            .height
        : 0,
    }));
    const picture = K.bitmap.run('create', {
      width,
      height: Math.max(1, Math.trunc(parts[0].height + parts[1].height + 2 * padY)),
      color: 0xffffffff,
    });
    try {
      let top = padY;
      for (const part of parts) {
        if (part.text)
          K.bitmap.run('drawText', {
            id: picture.id,
            text: part.text,
            size: part.size,
            width: part.width,
            spacing: part.spacing,
            extra: part.extra,
            bold: part.bold,
            x: padX,
            y: top,
            color: 0xff000000,
          });
        top += part.height;
      }
      const png = K.bitmap.encode(picture.id, 'PNG', 100);
      return Response.made({
        code: 200,
        message: 'OK',
        request,
        headers: new Headers([['Content-Type', 'image/png']]),
        body: bytesBody(async () => png, 'image/png'),
      });
    } finally {
      K.bitmap.run('free', { id: picture.id });
    }
  };
  const textInterceptor = () => ({
    __class: 'TextInterceptor',
    __classes: ['TextInterceptor', 'Interceptor'],
    intercept: async (chain) => {
      const request = chain.request();
      if (request.url.host !== TEXT_HOST) return chain.proceed(request);
      return textPage(request);
    },
  });
  const textInterceptorHelper = {
    HOST: TEXT_HOST,
    createUrl: (title, text) =>
      `http://${TEXT_HOST}/${encodeURIComponent(KT.str(title))}/${encodeURIComponent(KT.str(text))}`,
  };

  // ---- Sequences, formats and the rest of Kotlin's and Java's library ----

  /// Kotlin's sequences that may not end (`generateSequence(seed) { next(it) }`): items worked
  /// out as they're read; anything else reads the first `SEQUENCE_CAP` as a list.
  const SEQUENCE_CAP = 100000;
  const lazySequence = (generate) => ({
    isLazySequence: true,
    __class: 'Sequence',
    __classes: ['Sequence'],
    generate,
  });
  const collectSequence = async (sequence, limit = SEQUENCE_CAP) => {
    const out = [];
    if (limit <= 0) return out;
    for await (const item of sequence.generate()) {
      out.push(item);
      if (out.length >= limit) break;
    }
    return out;
  };
  const SEQUENCE = {
    iterator: (s) => {
      const source = s.generate();
      let peeked = null;
      const fill = async () => (peeked ??= await source.next());
      return {
        __class: 'Iterator',
        __classes: ['Iterator'],
        hasNext: async () => !(await fill()).done,
        next: async () => {
          const found = await fill();
          peeked = null;
          return found.done ? noSuch('the sequence has no more items') : found.value;
        },
      };
    },
    drop: (s, [count]) =>
      lazySequence(async function* () {
        let skipped = 0;
        for await (const item of s.generate()) if (skipped++ >= count) yield item;
      }),
    take: (s, [count]) =>
      lazySequence(async function* () {
        if (count <= 0) return;
        let taken = 0;
        for await (const item of s.generate()) {
          yield item;
          if (++taken >= count) return;
        }
      }),
    takeWhile: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) {
          if (!KT.truthy(await lambda(item))) return;
          yield item;
        }
      }),
    dropWhile: (s, _args, lambda) =>
      lazySequence(async function* () {
        let dropping = true;
        for await (const item of s.generate()) {
          if (dropping && KT.truthy(await lambda(item))) continue;
          dropping = false;
          yield item;
        }
      }),
    map: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) yield await lambda(item);
      }),
    mapIndexed: (s, _args, lambda) =>
      lazySequence(async function* () {
        let index = 0;
        for await (const item of s.generate()) yield await lambda(index++, item);
      }),
    mapNotNull: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) {
          const value = await lambda(item);
          if (value !== null && value !== undefined) yield value;
        }
      }),
    filter: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) if (KT.truthy(await lambda(item))) yield item;
      }),
    filterNot: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) if (!KT.truthy(await lambda(item))) yield item;
      }),
    filterNotNull: (s) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) if (item !== null && item !== undefined) yield item;
      }),
    filterIsInstance: (s, _args, _lambda, _named, type) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) if (KT.is(item, type)) yield item;
      }),
    onEach: (s, _args, lambda) =>
      lazySequence(async function* () {
        for await (const item of s.generate()) {
          await lambda(item);
          yield item;
        }
      }),
    constrainOnce: (s) => s,
    asSequence: (s) => s,
    firstOrNull: async (s, _args, lambda) => {
      for await (const item of s.generate())
        if (!lambda || KT.truthy(await lambda(item))) return item;
      return null;
    },
    first: async (s, args, lambda) => {
      for await (const item of s.generate())
        if (!lambda || KT.truthy(await lambda(item))) return item;
      return noSuch('the sequence is empty');
    },
    find: async (s, _args, lambda) => SEQUENCE.firstOrNull(s, [], lambda),
    any: async (s, _args, lambda) =>
      (await SEQUENCE.firstOrNull(s, [], lambda ?? (() => true))) !== null,
    none: async (s, _args, lambda) => !(await SEQUENCE.any(s, [], lambda)),
    all: async (s, _args, lambda) => {
      for await (const item of s.generate()) if (!KT.truthy(await lambda(item))) return false;
      return true;
    },
    elementAtOrNull: async (s, [index]) => {
      let at = 0;
      for await (const item of s.generate()) if (at++ === index) return item;
      return null;
    },
    elementAt: async (s, [index]) =>
      (await SEQUENCE.elementAtOrNull(s, [index])) ?? noSuch(`no item at ${index}`),
    forEach: async (s, _args, lambda) => {
      for await (const item of s.generate()) await lambda(item);
    },
  };
  const sequenceCall = async (s, name, args, lambda, named, type) => {
    if (SEQUENCE[name]) return SEQUENCE[name](s, args, lambda, named, type);
    const items = await collectSequence(s);
    if (LIST[name]) return LIST[name](items, args, lambda, named, type);
    if (ANY[name]) return ANY[name](s, args, lambda, named, type);
    throw new KotlinError(`Raygun's runtime has no ${name}() for a sequence`);
  };
  /// What a `for` loop goes through: a sequence's items (up to the cap), else as `KT.iter`.
  KT.each = async (value) =>
    value && value.isLazySequence ? collectSequence(value) : KT.iter(value);
  /// What a `for await` loop over a sequence that may not end reads, item by item.
  KT.items = (value) => (value && value.isLazySequence ? value.generate() : KT.iter(value));
  /// `sequence { yield(x); yieldAll(xs) }` (and `iterator { … }`): the block runs as the items
  /// are read, each `yield` waiting until the next one is asked for; ranges and sequences it
  /// yields all of are read one by one (`yieldAll(start..Int.MAX_VALUE)`).
  const builtItems = (block) => {
    let started = false;
    let finished = false;
    let reader = null; // the pending `next()`: [resolve, reject]
    let resume = null; // the block waiting at a `yield`
    const give = (result) => {
      const [resolve] = reader ?? [];
      reader = null;
      if (resolve) resolve(result);
    };
    const scope = {
      __class: 'SequenceScope',
      __classes: ['SequenceScope'],
      yield: (value) =>
        new Promise((resolve) => {
          resume = resolve;
          give({ value, done: false });
        }),
      yieldAll: async (values) => {
        if (values && values.isRange) {
          const { first, last, step } = values;
          for (let at = first; step > 0 ? at <= last : at >= last; at += step) {
            if (finished) return;
            await scope.yield(at);
          }
          return;
        }
        if (values && values.isLazySequence) {
          for await (const item of values.generate()) await scope.yield(item);
          return;
        }
        for (const item of KT.iter(values)) await scope.yield(item);
      },
    };
    return {
      [Symbol.asyncIterator]() {
        return this;
      },
      next: () =>
        new Promise((resolve, reject) => {
          if (finished) {
            resolve({ value: undefined, done: true });
            return;
          }
          reader = [resolve, reject];
          if (started) {
            const waiting = resume;
            resume = null;
            if (waiting) waiting();
            return;
          }
          started = true;
          Promise.resolve()
            .then(() => block(scope))
            .then(
              () => {
                finished = true;
                give({ value: undefined, done: true });
              },
              (error) => {
                finished = true;
                const [, fail] = reader ?? [];
                reader = null;
                if (fail) fail(error);
              },
            );
        }),
      // A reader that stops early (`take(3)`, a `break`) leaves the block waiting for good.
      return: async () => {
        finished = true;
        return { value: undefined, done: true };
      },
    };
  };
  KT.sequenceOf = (block) => lazySequence(() => builtItems(block));
  /// `PropertyDelegateProvider { thisRef, property -> lazy { … } }` behind `val x by provider`:
  /// the delegate it gives, made once, and its value.
  const delegateProvider = (provide) => {
    let delegate;
    let made = false;
    const value = async () => {
      if (!made) {
        delegate = await provide(null, { __class: 'KProperty', name: '' });
        made = true;
      }
      if (delegate && delegate.isDeferred) return delegate.await();
      if (delegate && typeof delegate.getValue === 'function') return delegate.getValue(null, {});
      return delegate;
    };
    return {
      __class: 'PropertyDelegateProvider',
      __classes: ['PropertyDelegateProvider'],
      isGetterHost: true,
      getters: { value },
      provideDelegate: async () => KT.deferred(value),
    };
  };
  /// `generateSequence(seed) { next }`, `generateSequence { next }`, `generateSequence(first, next)`:
  /// items until one is null.
  const generateSequence = (args, lambda) => {
    const [first, step] =
      args.length >= 2
        ? [args[0], args[1]]
        : lambda && args.length === 0
          ? [lambda, lambda]
          : [args[0], lambda ?? args[1]];
    return lazySequence(async function* () {
      let value = typeof first === 'function' && args.length !== 1 ? await first() : first;
      while (value !== null && value !== undefined) {
        yield value;
        value = args.length === 0 ? await step() : await step(value);
      }
    });
  };

  /// Java's `DecimalFormat`: `#`, `0`, grouping and decimals as the pattern says.
  const decimalFormat = (pattern) => {
    const text = KT.str(pattern ?? '#.##');
    const [whole, fraction = ''] = text.split(';')[0].split('.');
    const grouping = whole.includes(',') ? whole.length - whole.lastIndexOf(',') - 1 : 0;
    const minWhole = (whole.match(/0/g) ?? []).length;
    const minFraction = (fraction.match(/0/g) ?? []).length;
    const maxFraction = (fraction.match(/[0#]/g) ?? []).length;
    const format = {
      __class: 'DecimalFormat',
      __classes: ['DecimalFormat', 'NumberFormat'],
      groupingSize: grouping,
      format: (number) => {
        const value = Number(number);
        if (!Number.isFinite(value)) return String(value);
        const fixed = Math.abs(value).toFixed(maxFraction);
        let [integer, decimals = ''] = fixed.split('.');
        decimals = decimals.replace(/0+$/, '');
        while (decimals.length < minFraction) decimals += '0';
        integer = integer.replace(/^0+(?=\d)/, '');
        if (integer === '0' && minWhole === 0 && decimals) integer = '';
        while (integer.length < minWhole) integer = `0${integer}`;
        if (format.groupingSize > 0)
          integer = integer.replace(
            new RegExp(`\\B(?=(\\d{${format.groupingSize}})+(?!\\d))`, 'g'),
            ',',
          );
        const sign = value < 0 && (Number(integer) || Number(decimals)) ? '-' : '';
        return `${sign}${integer}${decimals ? `.${decimals}` : ''}`;
      },
      parse: (text) => Number(KT.str(text).replace(/,/g, '')),
      setRoundingMode: () => KT.unit,
      setMaximumFractionDigits: () => KT.unit,
      setMinimumFractionDigits: () => KT.unit,
      setGroupingUsed: (used) => {
        format.groupingSize = used ? grouping || 3 : 0;
      },
    };
    return format;
  };

  /// Kotlin's `CharCategory` of a character (`c.category == CharCategory.NON_SPACING_MARK`), by
  /// its Unicode general category; the categories are their names here.
  const CHAR_CATEGORIES = [
    ['Lu', 'UPPERCASE_LETTER'],
    ['Ll', 'LOWERCASE_LETTER'],
    ['Lt', 'TITLECASE_LETTER'],
    ['Lm', 'MODIFIER_LETTER'],
    ['Lo', 'OTHER_LETTER'],
    ['Mn', 'NON_SPACING_MARK'],
    ['Me', 'ENCLOSING_MARK'],
    ['Mc', 'COMBINING_SPACING_MARK'],
    ['Nd', 'DECIMAL_DIGIT_NUMBER'],
    ['Nl', 'LETTER_NUMBER'],
    ['No', 'OTHER_NUMBER'],
    ['Zs', 'SPACE_SEPARATOR'],
    ['Zl', 'LINE_SEPARATOR'],
    ['Zp', 'PARAGRAPH_SEPARATOR'],
    ['Cc', 'CONTROL'],
    ['Cf', 'FORMAT'],
    ['Co', 'PRIVATE_USE'],
    ['Cs', 'SURROGATE'],
    ['Pd', 'DASH_PUNCTUATION'],
    ['Ps', 'START_PUNCTUATION'],
    ['Pe', 'END_PUNCTUATION'],
    ['Pc', 'CONNECTOR_PUNCTUATION'],
    ['Po', 'OTHER_PUNCTUATION'],
    ['Sm', 'MATH_SYMBOL'],
    ['Sc', 'CURRENCY_SYMBOL'],
    ['Sk', 'MODIFIER_SYMBOL'],
    ['So', 'OTHER_SYMBOL'],
    ['Pi', 'INITIAL_QUOTE_PUNCTUATION'],
    ['Pf', 'FINAL_QUOTE_PUNCTUATION'],
  ];
  const charCategory = (char) => {
    const text = KT.str(char);
    for (const [code, name] of CHAR_CATEGORIES) {
      try {
        if (new RegExp(`^\\p{${code}}`, 'u').test(text)) return name;
      } catch {
        break;
      }
    }
    return 'UNASSIGNED';
  };
  /// Java's `Collator`: text compared as a language orders it.
  const collator = (locale) => {
    const tag = locale && locale.language ? KT.str(locale.language) : undefined;
    const made = {
      __class: 'Collator',
      __classes: ['Collator', 'RuleBasedCollator', 'Comparator'],
      strength: 2,
      decomposition: 0,
      compare: (left, right) => {
        const options =
          made.strength === 0
            ? { sensitivity: 'base' }
            : made.strength === 1
              ? { sensitivity: 'accent' }
              : {};
        try {
          return KT.str(left).localeCompare(KT.str(right), tag, options);
        } catch {
          return KT.cmp(KT.str(left), KT.str(right));
        }
      },
      setStrength: (value) => {
        made.strength = value;
      },
      setDecomposition: (value) => {
        made.decomposition = value;
      },
      freeze: () => made,
      cloneAsThawed: () => made,
      getCollationKey: (text) => ({
        sourceString: KT.str(text),
        compareTo: (other) => made.compare(text, other.sourceString),
      }),
    };
    return made;
  };
  /// `String.CASE_INSENSITIVE_ORDER`.
  const caseInsensitiveOrder = {
    __class: 'Comparator',
    __classes: ['Comparator'],
    compare: (left, right) => KT.cmp(KT.str(left).toLowerCase(), KT.str(right).toLowerCase()),
  };
  /// A comparator's verdict on two values: a `Comparator { a, b -> … }`, a collator, or one made
  /// by `compareBy`.
  const compareWith = async (order, left, right) => {
    if (typeof order === 'function') return order(left, right);
    if (order && order.keys) return order.compare(left, right);
    if (order && typeof order.compare === 'function') return order.compare(left, right);
    return KT.cmp(left, right);
  };

  /// Android's `LruCache` and the like: a map that keeps its newest `size` entries.
  const lruCache = (size) => {
    const store = new Map();
    const cache = {
      __class: 'LruCache',
      __classes: ['LruCache'],
      get: (key) => {
        if (!store.has(key)) return null;
        const value = store.get(key);
        store.delete(key);
        store.set(key, value);
        return value;
      },
      put: (key, value) => {
        const before = store.has(key) ? store.get(key) : null;
        store.delete(key);
        store.set(key, value);
        while (store.size > Math.max(1, Number(size) || 1)) store.delete(store.keys().next().value);
        return before;
      },
      remove: (key) => {
        const before = store.has(key) ? store.get(key) : null;
        store.delete(key);
        return before;
      },
      evictAll: () => store.clear(),
      size: () => store.size,
      snapshot: () => new Map(store),
    };
    return cache;
  };

  /// kotlinx.coroutines' `CompletableDeferred`: a value given later.
  const completableDeferred = (initial) => {
    let settle;
    let fail;
    let done = initial !== undefined;
    const promise =
      initial !== undefined
        ? Promise.resolve(initial)
        : new Promise((resolve, reject) => {
            settle = resolve;
            fail = reject;
          });
    promise.catch(() => {});
    return {
      __class: 'CompletableDeferred',
      __classes: ['CompletableDeferred', 'Deferred', 'Job'],
      isDeferred: true,
      await: () => promise,
      complete: (value) => {
        if (done) return false;
        done = true;
        settle(value);
        return true;
      },
      completeExceptionally: (error) => {
        if (done) return false;
        done = true;
        fail(KT.error(error));
        return true;
      },
      get isCompleted() {
        return done;
      },
      isActive: true,
      cancel: () => KT.unit,
      join: () =>
        promise.then(
          () => KT.unit,
          () => KT.unit,
        ),
    };
  };

  /// Java's `URI` and `URL`, and Android's `Uri`: an address's parts.
  const javaUri = (text, spec) => {
    let href = KT.str(text);
    if (spec !== undefined && spec !== null) href = K.resolve(KT.str(spec), href);
    let parts = null;
    try {
      parts = K.url(href);
    } catch {
      parts = null;
    }
    const pathOf = () => (parts ? parts.path : href.split(/[?#]/)[0]);
    const uri = {
      __class: 'URI',
      __classes: ['URI', 'URL', 'Uri'],
      scheme: parts?.scheme ?? null,
      protocol: parts?.scheme ?? null,
      host: parts?.host ?? null,
      authority: parts?.host ?? null,
      port: parts?.port ?? -1,
      path: pathOf(),
      rawPath: pathOf(),
      encodedPath: pathOf(),
      query: parts?.query == null ? null : decodeURIComponent(parts.query.replace(/\+/g, ' ')),
      rawQuery: parts?.query ?? null,
      encodedQuery: parts?.query ?? null,
      fragment: parts?.fragment ?? null,
      file: parts ? `${parts.path}${parts.query == null ? '' : `?${parts.query}`}` : href,
      get pathSegments() {
        return pathOf()
          .split('/')
          .filter((part) => part !== '')
          .map((part) => decodeURIComponent(part));
      },
      get lastPathSegment() {
        const segments = uri.pathSegments;
        return segments.length ? segments[segments.length - 1] : null;
      },
      getQueryParameter: (name) => (parts ? HttpUrl.parse(href).queryParameter(name) : null),
      getQueryParameterNames: () => (parts ? HttpUrl.parse(href).queryParameterNames : []),
      resolve: (other) => javaUri(K.resolve(KT.str(other), href)),
      toURL: () => uri,
      toURI: () => uri,
      toHttpUrl: () => HttpUrl.parse(href),
      toExternalForm: () => href,
      buildUpon: () => HttpUrl.parse(href).newBuilder(),
      toString: () => href,
      equals: (other) => KT.str(other) === href,
    };
    return uri;
  };
  /// Android's `Uri.encode`: everything but letters, digits and `_-!.~'()*` (and `allow`).
  const uriEncode = (text, allow = '') =>
    [...KT.str(text)]
      .map((char) =>
        /[A-Za-z0-9_\-!.~'()*]/.test(char) || KT.str(allow ?? '').includes(char)
          ? char
          : encodeURIComponent(char),
      )
      .join('');

  /// Jsoup's evaluators, as the selectors they match.
  const cssName = (name) => KT.str(name).replace(/([^A-Za-z0-9_-])/g, '\\$1');
  const evaluator = {
    Tag: (name) => cssName(name),
    Class: (name) => `.${cssName(name)}`,
    Id: (name) => `#${cssName(name)}`,
    Attribute: (name) => `[${cssName(name)}]`,
    AttributeWithValue: (name, value) =>
      `[${cssName(name)}="${KT.str(value).replace(/"/g, '\\"')}"]`,
    AttributeWithValueContaining: (name, value) =>
      `[${cssName(name)}*="${KT.str(value).replace(/"/g, '\\"')}"]`,
    AttributeStarting: (prefix) => `[^${cssName(prefix)}]`,
    AllElements: () => '*',
  };

  /// kotlinx.coroutines' dispatchers and jobs: Raygun runs a source's code on one thread.
  const coroutineContext = (name) => ({
    __class: name,
    __classes: [name, 'CoroutineContext'],
    plus: () => coroutineContext(name),
    limitedParallelism: () => coroutineContext(name),
    cancel: () => KT.unit,
    cancelChildren: () => KT.unit,
    isActive: true,
  });
  const dispatchers = {
    IO: coroutineContext('Dispatchers.IO'),
    Default: coroutineContext('Dispatchers.Default'),
    Main: coroutineContext('Dispatchers.Main'),
    Unconfined: coroutineContext('Dispatchers.Unconfined'),
  };
  /// Keiyoushi's web view helpers: Raygun has no web view, so a source driving one fails, naming
  /// the page it would load: as a browser check when it passes one (which the person may pass by
  /// hand: its cookies then go with the source's requests), else as a page that needs one.
  const webViewNeeded = (page, baseUrl, check) => {
    let host = '';
    for (const address of [page, baseUrl]) {
      if (host || address === null || address === undefined) continue;
      try {
        host = HttpUrl.parse(KT.str(address)).host ?? '';
      } catch {
        host = '';
      }
    }
    const site = host || 'the site';
    if (check)
      throw new KotlinError(
        `A browser check from ${site}: a bot check page the source passes in a web view`,
        'IOException',
      );
    throw new KotlinError(
      `The source reads ${site} in a web view, which Raygun doesn't have`,
      'IOException',
    );
  };
  /// Android's preference widgets, made by settings screens the converter leaves out: they keep
  /// what's set on them and show nothing.
  const preferenceWidget = (kind) => {
    const widget = {
      __class: kind,
      __classes: [kind, 'Preference'],
      key: null,
      title: null,
      summary: null,
      dialogTitle: null,
      dialogMessage: null,
      entries: [],
      entryValues: [],
      text: null,
      value: null,
      values: [],
      isChecked: false,
      isVisible: true,
      isEnabled: true,
      setDefaultValue: () => KT.unit,
      setOnPreferenceChangeListener: () => KT.unit,
      setOnPreferenceClickListener: () => KT.unit,
      setOnBindEditTextListener: () => KT.unit,
      addPreference: () => true,
    };
    return widget;
  };
  /// Keiyoushi's `SeedRandom` (seedrandom's ARC4 generator, `keiyoushi.utils`): the same
  /// numbers for the same seed.
  const seedRandom = (seed) => {
    const WIDTH = 256;
    const MASK = 255;
    const CHUNKS = 6;
    const startdenom = WIDTH ** CHUNKS;
    const significance = 2 ** 52;
    const overflow = significance * 2;
    const text = KT.str(seed);
    const key = new Array(WIDTH).fill(0);
    let smear = 0;
    for (let index = 0; index < text.length; index++) {
      smear ^= key[MASK & index] * 19;
      key[MASK & index] = MASK & (smear + text.charCodeAt(index));
    }
    const mixed = key.slice(0, Math.min(text.length, WIDTH));
    const effective = mixed.length ? mixed : [0];
    const state = Array.from({ length: WIDTH }, (_, index) => index);
    let i = 0;
    let j = 0;
    const g = (count) => {
      let r = 0;
      for (let left = count; left > 0; left--) {
        i = MASK & (i + 1);
        const t = state[i];
        j = MASK & (j + t);
        const sj = state[j];
        state[i] = sj;
        state[j] = t;
        r = r * WIDTH + state[MASK & (sj + t)];
      }
      return r;
    };
    let counter = 0;
    for (let k = 0; k < WIDTH; k++) {
      const t = state[k];
      counter = MASK & (counter + effective[k % effective.length] + t);
      state[k] = state[counter];
      state[counter] = t;
    }
    g(WIDTH);
    const nextDouble = () => {
      let n = g(CHUNKS);
      let d = startdenom;
      let x = 0;
      while (n < significance) {
        n = (n + x) * WIDTH;
        d *= WIDTH;
        x = g(1);
      }
      while (n >= overflow) {
        n /= 2;
        d /= 2;
        x = Math.floor(x / 2);
      }
      return (n + x) / d;
    };
    return {
      __class: 'SeedRandom',
      __classes: ['SeedRandom'],
      nextDouble,
      shuffle: (list) => {
        const keys = list.map((_, index) => index);
        const out = [];
        for (let count = 0; count < list.length; count++) {
          const at = Math.floor(nextDouble() * keys.length);
          out.push(list[keys.splice(at, 1)[0]]);
        }
        return out;
      },
    };
  };
  /// Bytes deflated in Rust (`runtime.rs`): `zlib`, `raw` or `gzip`.
  const deflateBytes = (bytes, format) =>
    u8Of(fromBase64(crypto({ op: 'deflate', data: K.bin.toBase64(u8Of(bytes)), format })));
  const compressingStream = (target, format) => {
    const parts = [];
    let closed = false;
    const stream = {
      __class: 'OutputStream',
      __classes: ['OutputStream', 'GZIPOutputStream', 'DeflaterOutputStream'],
      write: (value, offset = 0, count = undefined) => {
        const bytes = typeof value === 'number' ? [value & 255] : Array.from(u8Of(value));
        parts.push(...bytes.slice(offset, count === undefined ? undefined : offset + count));
        return KT.unit;
      },
      flush: () => KT.unit,
      finish: async () => stream.close(),
      close: async () => {
        if (closed) return KT.unit;
        closed = true;
        await KT.m(target, 'write', [deflateBytes(parts, format)]);
        return KT.unit;
      },
      use: async (lambda) => {
        try {
          return await lambda(stream);
        } finally {
          await stream.close();
        }
      },
    };
    return stream;
  };
  /// org.json's `JSONObject` and `JSONArray` over parsed JSON: objects and arrays inside come as
  /// these too, text, numbers and booleans as they are.
  const orgJsonValue = (value) =>
    Array.isArray(value)
      ? orgJsonArray(value)
      : value && typeof value === 'object'
        ? orgJsonObject(value)
        : (value ?? null);
  const jsonTextOf = (value) =>
    value === null || value === undefined
      ? null
      : typeof value === 'object'
        ? JSON.stringify(value)
        : String(value);
  const orgJsonObject = (data) => {
    const found = (key) => {
      const name = KT.str(key);
      if (!Object.prototype.hasOwnProperty.call(data, name))
        throw new KotlinError(`JSONObject["${name}"] not found.`, 'JSONException');
      return data[name];
    };
    const self = {
      __class: 'JSONObject',
      __classes: ['JSONObject'],
      data,
      opt: (key) => orgJsonValue(data[KT.str(key)]),
      get: (key) => orgJsonValue(found(key)),
      has: (key) => Object.prototype.hasOwnProperty.call(data, KT.str(key)),
      isNull: (key) => data[KT.str(key)] === null || data[KT.str(key)] === undefined,
      optString: (key, fallback = '') => jsonTextOf(data[KT.str(key)]) ?? fallback,
      getString: (key) => jsonTextOf(found(key)),
      optInt: (key, fallback = 0) => Math.trunc(Number(data[KT.str(key)] ?? fallback)),
      getInt: (key) => Math.trunc(Number(found(key))),
      optLong: (key, fallback = 0) => Math.trunc(Number(data[KT.str(key)] ?? fallback)),
      getLong: (key) => Math.trunc(Number(found(key))),
      optDouble: (key, fallback = NaN) => Number(data[KT.str(key)] ?? fallback),
      getDouble: (key) => Number(found(key)),
      optBoolean: (key, fallback = false) => data[KT.str(key)] ?? fallback,
      getBoolean: (key) => Boolean(found(key)),
      optJSONObject: (key) => {
        const value = data[KT.str(key)];
        return value && typeof value === 'object' && !Array.isArray(value)
          ? orgJsonObject(value)
          : null;
      },
      getJSONObject: (key) => orgJsonObject(found(key)),
      optJSONArray: (key) => {
        const value = data[KT.str(key)];
        return Array.isArray(value) ? orgJsonArray(value) : null;
      },
      getJSONArray: (key) => orgJsonArray(found(key)),
      keys: () => Object.keys(data),
      names: () => orgJsonArray(Object.keys(data)),
      length: () => Object.keys(data).length,
      put: (key, value) => {
        data[KT.str(key)] = plain(value && value.data !== undefined ? value.data : value);
        return self;
      },
      remove: (key) => {
        const value = data[KT.str(key)];
        delete data[KT.str(key)];
        return orgJsonValue(value);
      },
      toString: () => JSON.stringify(data),
    };
    return self;
  };
  const orgJsonArray = (data) => {
    const found = (index) => {
      if (index < 0 || index >= data.length)
        throw new KotlinError(`JSONArray[${index}] not found.`, 'JSONException');
      return data[index];
    };
    const self = {
      __class: 'JSONArray',
      __classes: ['JSONArray'],
      data,
      length: () => data.length,
      opt: (index) => orgJsonValue(data[index]),
      get: (index) => orgJsonValue(found(index)),
      isNull: (index) => data[index] === null || data[index] === undefined,
      optString: (index, fallback = '') => jsonTextOf(data[index]) ?? fallback,
      getString: (index) => jsonTextOf(found(index)),
      optInt: (index, fallback = 0) => Math.trunc(Number(data[index] ?? fallback)),
      getInt: (index) => Math.trunc(Number(found(index))),
      optLong: (index, fallback = 0) => Math.trunc(Number(data[index] ?? fallback)),
      getLong: (index) => Math.trunc(Number(found(index))),
      optJSONObject: (index) => {
        const value = data[index];
        return value && typeof value === 'object' && !Array.isArray(value)
          ? orgJsonObject(value)
          : null;
      },
      getJSONObject: (index) => orgJsonObject(found(index)),
      optJSONArray: (index) => (Array.isArray(data[index]) ? orgJsonArray(data[index]) : null),
      getJSONArray: (index) => orgJsonArray(found(index)),
      put: (value) => {
        data.push(plain(value && value.data !== undefined ? value.data : value));
        return self;
      },
      toString: () => JSON.stringify(data),
      [Symbol.iterator]: function* () {
        for (const item of data) yield orgJsonValue(item);
      },
    };
    return self;
  }; /// Bytes inflated in Rust (`runtime.rs`): Java's `Inflater` (`zlib`, `raw` without its
  /// header) and `GZIPInputStream` (`gzip`).
  const inflateBytes = (bytes, format) =>
    u8Of(fromBase64(crypto({ op: 'inflate', data: K.bin.toBase64(u8Of(bytes)), format })));
  /// Android's screen as a phone's, for sources sizing what they ask for by it.
  const displayMetrics = {
    widthPixels: 1080,
    heightPixels: 2340,
    density: 2.75,
    densityDpi: 440,
    scaledDensity: 2.75,
    xdpi: 440,
    ydpi: 440,
  };
  /// Keiyoushi's dialogs (`keiyoushi.utils.ui`): none shows, as if the person closed it.
  const dialogClosed = (fallback) => () => fallback;
  /// One of the extension's resources (`/assets/reader.js`), or null when it has none by that
  /// name: Java's `URL` to read it through.
  const resourceOf = (path) => {
    const key = KT.str(path).replace(/^\/+/, '');
    const found = assets[key];
    if (found === undefined) return null;
    const bytes = () => K.bin.fromBase64(found);
    return {
      __class: 'URL',
      __classes: ['URL'],
      readText: () => K.bin.text(bytes()),
      readBytes: () => listOfBytes(bytes()),
      openStream: () => inputStream(bytes()),
      path: `/${key}`,
      toString: () => `jar:file:/extension.apk!/${key}`,
    };
  };
  /// A class as `X::class.java` and `javaClass` give it: its name, and the extension's
  /// resources.
  /// A value's class name (`exception::class.simpleName`).
  KT.classNameOf = (value) =>
    value === null || value === undefined
      ? 'Nothing'
      : (value.__class ??
        value.kind ??
        { string: 'String', number: 'Int', boolean: 'Boolean', list: 'List', map: 'Map' }[
          kindOf(value)
        ] ??
        'Any');
  KT.javaClass = (name) => {
    if (name && name.__class === 'Class') return name;
    const named = KT.str(name);
    const made = {
      __class: 'Class',
      __classes: ['Class'],
      name: named,
      simpleName: named.split('.').pop(),
      toString: () => named,
      getResource: (path) => resourceOf(path),
      getResourceAsStream: (path) => resourceOf(path)?.openStream() ?? null,
    };
    made.classLoader = made;
    return made;
  };
  /// java.time's `DayOfWeek`: Monday is 1.
  const DAY_NAMES = ['MONDAY', 'TUESDAY', 'WEDNESDAY', 'THURSDAY', 'FRIDAY', 'SATURDAY', 'SUNDAY'];
  const dayOfWeek = (value) => {
    const name = DAY_NAMES[(((Number(value) - 1) % 7) + 7) % 7];
    const full = name.charAt(0) + name.slice(1).toLowerCase();
    return {
      __class: 'DayOfWeek',
      __classes: ['DayOfWeek', 'Enum'],
      name,
      value: Number(value),
      ordinal: Number(value) - 1,
      getValue: () => Number(value),
      // English names: Raygun shows sources' text as they give it.
      getDisplayName: (style) =>
        KT.str(style) === 'SHORT' || KT.str(style) === 'SHORT_STANDALONE'
          ? full.slice(0, 3)
          : KT.str(style) === 'NARROW'
            ? full.charAt(0)
            : full,
      plus: (days) => dayOfWeek(Number(value) + Number(days)),
      minus: (days) => dayOfWeek(Number(value) - Number(days)),
      toString: () => name,
    };
  };
  /// Languages' names in English, for `Locale("ja").displayLanguage`.
  const LANGUAGE_NAMES = {
    ar: 'Arabic',
    bg: 'Bulgarian',
    ca: 'Catalan',
    cs: 'Czech',
    da: 'Danish',
    de: 'German',
    el: 'Greek',
    en: 'English',
    es: 'Spanish',
    fa: 'Persian',
    fi: 'Finnish',
    fil: 'Filipino',
    fr: 'French',
    he: 'Hebrew',
    hi: 'Hindi',
    hu: 'Hungarian',
    id: 'Indonesian',
    it: 'Italian',
    ja: 'Japanese',
    ko: 'Korean',
    ms: 'Malay',
    my: 'Burmese',
    nl: 'Dutch',
    no: 'Norwegian',
    pl: 'Polish',
    pt: 'Portuguese',
    ro: 'Romanian',
    ru: 'Russian',
    sv: 'Swedish',
    th: 'Thai',
    tl: 'Tagalog',
    tr: 'Turkish',
    uk: 'Ukrainian',
    vi: 'Vietnamese',
    zh: 'Chinese',
  };
  /// Java's `File`, over a memory of the runtime's own: what a source writes it reads back while
  /// it runs (its caches), and nothing reaches the device's storage.
  const memoryFiles = new Map();
  const javaFile = (first, second = undefined) => {
    const parent =
      second === undefined ? null : first && first.isFile ? first.path : KT.str(first ?? '');
    const path = (parent === null ? KT.str(first) : `${parent}/${KT.str(second)}`).replace(
      /\/+/g,
      '/',
    );
    const inside = (key) => key.startsWith(`${path}/`);
    const missing = () => new KotlinError(`${path} (No such file or directory)`, 'IOException');
    const file = {
      __class: 'File',
      __classes: ['File'],
      isFile: true,
      path,
      absolutePath: path,
      canonicalPath: path,
      name: path.split('/').pop(),
      get parentFile() {
        return javaFile(path.split('/').slice(0, -1).join('/') || '/');
      },
      exists: () => memoryFiles.has(path) || [...memoryFiles.keys()].some(inside),
      isDirectory: () => !memoryFiles.has(path),
      mkdirs: () => true,
      mkdir: () => true,
      createNewFile: () => {
        if (memoryFiles.has(path)) return false;
        memoryFiles.set(path, new Uint8Array(0));
        return true;
      },
      delete: () => memoryFiles.delete(path),
      deleteRecursively: () => {
        for (const key of [...memoryFiles.keys()])
          if (key === path || inside(key)) memoryFiles.delete(key);
        return true;
      },
      length: () => memoryFiles.get(path)?.length ?? 0,
      lastModified: () => 0,
      setLastModified: () => true,
      readBytes: () => {
        const bytes = memoryFiles.get(path);
        if (!bytes) throw missing();
        return listOfBytes(bytes);
      },
      readText: () => {
        const bytes = memoryFiles.get(path);
        if (!bytes) throw missing();
        return K.bin.text(bytes);
      },
      writeBytes: (bytes) => {
        memoryFiles.set(path, Uint8Array.from(u8Of(bytes)));
      },
      writeText: (text) => {
        memoryFiles.set(path, K.bin.utf8(KT.str(text)));
      },
      appendText: (text) => {
        const before = memoryFiles.get(path) ?? new Uint8Array(0);
        const added = K.bin.utf8(KT.str(text));
        const joined = new Uint8Array(before.length + added.length);
        joined.set(before);
        joined.set(added, before.length);
        memoryFiles.set(path, joined);
      },
      inputStream: () => {
        const bytes = memoryFiles.get(path);
        if (!bytes) throw missing();
        return inputStream(bytes);
      },
      listFiles: () =>
        [
          ...new Set(
            [...memoryFiles.keys()]
              .filter(inside)
              .map((key) => key.slice(path.length + 1).split('/')[0]),
          ),
        ].map((name) => javaFile(path, name)),
      renameTo: (other) => {
        const bytes = memoryFiles.get(path);
        if (!bytes) return false;
        memoryFiles.delete(path);
        memoryFiles.set(other.path, bytes);
        return true;
      },
      toString: () => path,
    };
    return file;
  };
  /// A stand-in for Android's application: its name, and folders of Raygun's own.
  const application = {
    __class: 'Application',
    __classes: ['Application', 'Context'],
    packageName: 'app.raygun.player',
    getPackageName: () => 'app.raygun.player',
    cacheDir: javaFile('/cache'),
    externalCacheDir: javaFile('/cache'),
    filesDir: javaFile('/files'),
    getCacheDir: () => javaFile('/cache'),
    getFilesDir: () => javaFile('/files'),
  };
  /// `AtomicInteger`, `AtomicLong`, `AtomicBoolean`, `AtomicReference`: a value, set and read
  /// on one thread here.
  const atomic = (initial) => {
    let value = initial;
    return {
      __class: 'Atomic',
      __classes: ['Atomic'],
      get: () => value,
      set: (next) => {
        value = next;
      },
      getAndSet: (next) => {
        const before = value;
        value = next;
        return before;
      },
      compareAndSet: (expected, next) => {
        if (!KT.eq(value, expected)) return false;
        value = next;
        return true;
      },
      incrementAndGet: () => ++value,
      getAndIncrement: () => value++,
      decrementAndGet: () => --value,
      getAndDecrement: () => value--,
      addAndGet: (delta) => (value += delta),
      getAndAdd: (delta) => {
        const before = value;
        value += delta;
        return before;
      },
      toString: () => KT.str(value),
    };
  };
  /// `ReentrantLock`, `ReentrantReadWriteLock`: a mutex (both of a read-write lock's sides).
  const lockOf = () => {
    const mutex = KT.mutex();
    mutex.tryLock = () => {
      if (mutex.isLocked) return false;
      mutex.lock();
      return true;
    };
    mutex.isHeldByCurrentThread = () => Boolean(mutex.isLocked);
    return mutex;
  };
  /// A pause (`delay(ms)`, `Thread.sleep(ms)`), at most ten seconds.
  const pause = async (ms) => {
    const wait = Math.max(0, Math.min(10000, Number(ms && ms.isDuration ? ms.ms : ms) || 0));
    if (wait > 0) await raygun.time.sleep(wait);
    return KT.unit;
  };

  const GLOBALS = {
    // A parent's constructor arguments come as `listOf(…)` too, and may be named
    // (`: UriSortFilter(name = "Sort", vals = arrayOf(…))`): those ride along.
    listOf: (args, named_) => {
      const list = [...args];
      if (named_ && Object.keys(named_).length)
        Object.defineProperty(list, '__named', { value: named_, enumerable: false });
      return list;
    },
    // Android's graphics, okio's `Buffer` and Java's byte streams (above).
    Canvas: ([bitmap]) => new Canvas(bitmap ?? null),
    Rect: (args) => new Rect(...args),
    RectF: (args) => new Rect(...args),
    Matrix: ([other]) => new Matrix(other ?? null),
    Paint: ([from]) => makePaint(from ?? null),
    TextPaint: ([from]) => makePaint(from ?? null),
    Buffer: () => okioBuffer(),
    ByteArrayOutputStream: () => outputStreamTo(okioBuffer()),
    ByteArrayInputStream: ([bytes, offset = 0, length]) => {
      const all = u8Of(bytes);
      return inputStream(all.subarray(offset, length === undefined ? all.length : offset + length));
    },
    // Keiyoushi's `lib:textinterceptor`: pages of text drawn as pictures.
    TextInterceptor: () => textInterceptor(),
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
    IndexOutOfBoundsException: ([message]) => new KotlinError(message, 'IndexOutOfBoundsException'),
    IntRange: ([from, to]) => KT.range(from, to),
    IOException: ([message]) => new KotlinError(message, 'IOException'),
    Error: ([message]) => new KotlinError(message, 'Error'),
    GeneralSecurityException: ([message]) => new KotlinError(message, 'GeneralSecurityException'),
    SecurityException: ([message]) => new KotlinError(message, 'SecurityException'),
    UnknownHostException: ([message]) => new KotlinError(message, 'UnknownHostException'),
    ProtocolException: ([message]) => new KotlinError(message, 'ProtocolException'),
    SocketTimeoutException: ([message]) => new KotlinError(message, 'SocketTimeoutException'),
    InterruptedException: ([message]) => new KotlinError(message, 'InterruptedException'),
    NumberFormatException: ([message]) => new KotlinError(message, 'NumberFormatException'),
    TimeoutException: ([message]) => new KotlinError(message, 'TimeoutException'),
    CancellationException: ([message]) => new KotlinError(message, 'CancellationException'),
    // Sequences, formats, caches and coroutines (above).
    generateSequence: (args, _named, lambda) => generateSequence(args, lambda),
    // `java.util.Random(seed)`: Java's own generator, so a seeded one gives Java's numbers.
    Random: ([seed]) => javaRandom(seed),
    // `StaticLayout(text, paint, width, alignment, mult, add, pad)`, or with a range of the text
    // first (`StaticLayout(text, start, end, paint, width, …)`).
    StaticLayout: (args) =>
      typeof args[1] === 'number'
        ? staticLayout({
            text: KT.str(args[0] ?? '').slice(args[1], args[2]),
            paint: args[3],
            width: args[4],
            spacingMult: args[6] ?? 1,
            spacingAdd: args[7] ?? 0,
          })
        : staticLayout({
            text: args[0],
            paint: args[1],
            width: args[2],
            spacingMult: args[4] ?? 1,
            spacingAdd: args[5] ?? 0,
          }),
    DecimalFormat: ([pattern]) => decimalFormat(pattern),
    SecureRandom: () => random,
    Any: () => ({ __class: 'Any', __classes: ['Any'] }),
    LruCache: ([size]) => lruCache(size),
    CompletableDeferred: ([value]) => completableDeferred(value),
    URI: ([text]) => javaUri(text),
    URL: ([first, second]) => (second === undefined ? javaUri(first) : javaUri(second, first)),
    SupervisorJob: () => coroutineContext('SupervisorJob'),
    Job: () => coroutineContext('Job'),
    delay: ([ms]) => pause(ms),
    // Kotlin's `assert`: off, as the JVM runs without assertions.
    assert: () => KT.unit,
    // Keiyoushi's web view helpers (above); a page's local storage is empty without one.
    runWebView: ([page, baseUrl, check]) => webViewNeeded(page, baseUrl, check),
    getTurnstileToken: ([url], named_) => webViewNeeded(url ?? named_?.url, null, true),
    getLocalStorage: () => null,
    // Settings screens' widgets and OkHttp's disk cache: nothing to show or keep.
    SwitchPreferenceCompat: () => preferenceWidget('SwitchPreferenceCompat'),
    CheckBoxPreference: () => preferenceWidget('CheckBoxPreference'),
    ListPreference: () => preferenceWidget('ListPreference'),
    EditTextPreference: () => preferenceWidget('EditTextPreference'),
    MultiSelectListPreference: () => preferenceWidget('MultiSelectListPreference'),
    PreferenceCategory: () => preferenceWidget('PreferenceCategory'),
    Cache: () => ({ __class: 'Cache', __classes: ['Cache'], evictAll: () => KT.unit }),
    SeedRandom: ([seed]) => seedRandom(seed),
    SoftReference: ([value]) => ({ get: () => value ?? null, clear: () => KT.unit }),
    WeakReference: ([value]) => ({ get: () => value ?? null, clear: () => KT.unit }),
    randomUUID: () => OBJECTS.UUID.randomUUID(),
    // Filters imported by name (`Header("…")`).
    Header: ([name]) => FILTER_CONSTRUCTORS.Header(name),
    Separator: ([name]) => FILTER_CONSTRUCTORS.Separator(name),
    // Injekt's `injectLazy()`: the `Json` most sources ask for this way.
    injectLazy: () => KT.deferred(async () => jsonCodec),
    AtomicInteger: ([value = 0]) => atomic(value),
    AtomicLong: ([value = 0]) => atomic(value),
    AtomicBoolean: ([value = false]) => atomic(value),
    AtomicReference: ([value = null]) => atomic(value),
    ReentrantLock: () => lockOf(),
    ReentrantReadWriteLock: () => {
      const lock = lockOf();
      return { readLock: () => lock, writeLock: () => lock };
    },
    // `launch { … }` inside a coroutine scope: run alongside.
    launch: (args, _named, lambda) => coroutineScope().launch(...args, lambda),
    showInfo: dialogClosed(KT.unit),
    askConfirm: dialogClosed(false),
    askInput: dialogClosed(null),
    askPassword: dialogClosed(null),
    askSelect: dialogClosed(null),
    askSelectOption: dialogClosed(null),
    GZIPInputStream: async ([stream]) =>
      inputStream(inflateBytes(await streamBytes(stream), 'gzip')),
    InflaterInputStream: async ([stream]) =>
      inputStream(inflateBytes(await streamBytes(stream), 'zlib')),
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
    Json: (_args, named_) => {
      const key = named_?.classDiscriminator;
      if (key && !discriminators.includes(KT.str(key))) discriminators.unshift(KT.str(key));
      return jsonCodec;
    },
    sleep: ([ms]) => pause(ms),
    File: ([first, second]) => javaFile(first, second),
    // OkHttp's `Dispatcher`: Raygun's client has its own limits.
    Dispatcher: () => ({ maxRequests: 64, maxRequestsPerHost: 5 }),
    // `AutoCloseable { … }`: what `close()` runs.
    AutoCloseable: (_args, _named, lambda) => ({
      __class: 'AutoCloseable',
      __classes: ['AutoCloseable', 'Closeable'],
      close: async () => (lambda ? lambda() : KT.unit),
    }),
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
    // `compareBy(collator) { it.title }`: the key in the comparator's order.
    compareBy: (args, _named, lambda) =>
      args.length === 1 && lambda && args[0] && typeof args[0] === 'object'
        ? comparator([{ select: lambda, descending: false, order: args[0] }])
        : comparator(
            (lambda ? [...args, lambda] : args).map((select) => ({ select, descending: false })),
          ),
    compareByDescending: (args, _named, lambda) =>
      args.length === 1 && lambda && args[0] && typeof args[0] === 'object'
        ? comparator([{ select: lambda, descending: true, order: args[0] }])
        : comparator(
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
    // `String(chars)`, and `String(bytes, charset)` (UTF-8 unless it says otherwise).
    String: ([value, charset]) => {
      if (!Array.isArray(value)) return KT.str(value);
      if (value.length && value.every((item) => typeof item === 'number'))
        return textOf(value, charset);
      return value.map(KT.str).join('');
    },
    // Keiyoushi's AES ciphers (`keiyoushi.utils.aesCbc(key, iv)`…) and Java's key specs.
    aesCbc: ([key, iv, padding = true, encrypt = false], named_) =>
      aesCipher({
        mode: 'cbc',
        key: bytesOf(key),
        iv: bytesOf(iv),
        padding: named_?.padding ?? padding,
        encrypt: named_?.encrypt ?? encrypt,
      }),
    aesEcb: ([key, padding = true, encrypt = false], named_) =>
      aesCipher({
        mode: 'ecb',
        key: bytesOf(key),
        padding: named_?.padding ?? padding,
        encrypt: named_?.encrypt ?? encrypt,
      }),
    aesGcm: ([key, iv, tagLength = 128, encrypt = false], named_) =>
      aesCipher({
        mode: 'gcm',
        key: bytesOf(key),
        iv: bytesOf(iv),
        tagLength: named_?.tagLength ?? tagLength,
        encrypt: named_?.encrypt ?? (typeof tagLength === 'boolean' ? tagLength : encrypt),
      }),
    aesCtr: ([key, iv, encrypt = false], named_) =>
      aesCipher({
        mode: 'ctr',
        key: bytesOf(key),
        iv: bytesOf(iv),
        encrypt: named_?.encrypt ?? encrypt,
      }),
    newAesCipher: ([name, key, params = null, encrypt = false], named_) =>
      aesCipher({
        ...transformation(name),
        key: bytesOf(key),
        iv: params ? bytesOf(params.iv ?? params) : null,
        encrypt: named_?.encrypt ?? encrypt,
      }),
    IvParameterSpec: ([iv]) => ({ __class: 'IvParameterSpec', iv: bytesOf(iv) }),
    SecretKeySpec: ([key, algorithm]) => ({
      __class: 'SecretKeySpec',
      key: bytesOf(key),
      algorithm: KT.str(algorithm),
      encoded: bytesOf(key),
    }),
    GCMParameterSpec: ([tagLength, iv]) => ({
      __class: 'GCMParameterSpec',
      iv: bytesOf(iv),
      tagLength,
    }),
    PBEKeySpec: ([password, salt, iterations, keyLength]) => ({
      __class: 'PBEKeySpec',
      password: Array.isArray(password) ? password.map(KT.str).join('') : KT.str(password),
      salt: bytesOf(salt),
      iterations,
      keyLength,
    }),
    // Jsoup's `TextNode("…")`, for `replaceWith`/`appendChild`.
    TextNode: ([text]) => mihon.textNode(text),
    // Keiyoushi's `XorInterceptor`: pictures XORed with a key the request gives
    // (`XorInterceptor { it.url.fragment?.decodeHex() }`) or a fixed one (`XorInterceptor(KEY) {
    // predicate }`). Raygun's image fetcher undoes it: the pages' addresses carry the key.
    XorInterceptor: (args, _named, lambda) => {
      const fixed = Array.isArray(args[0]) || typeof args[0] === 'number' ? args[0] : null;
      const keyFor =
        fixed !== null
          ? async (request) =>
              !lambda || KT.truthy(await lambda(request))
                ? Array.isArray(fixed)
                  ? fixed
                  : [fixed & 255]
                : null
          : async (request) => (lambda ? lambda(request) : null);
      return {
        __class: 'XorInterceptor',
        __classes: ['XorInterceptor', 'Interceptor'],
        isXor: true,
        keyFor,
        intercept: async (chain) => chain.proceed(chain.request()),
      };
    },
    // Collections made by their constructors (`ArrayList()`, `ArrayList(other)`, `ArrayDeque()`).
    ArrayList: ([from]) => (from && typeof from === 'object' ? [...KT.iter(from)] : []),
    ArrayDeque: ([from]) => (from && typeof from === 'object' ? [...KT.iter(from)] : []),
    // Arrays of a size (`ByteArray(16)`, `IntArray(n) { it }`) and of values.
    ByteArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? 0),
    IntArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? 0),
    LongArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? 0),
    DoubleArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? 0),
    FloatArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? 0),
    BooleanArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? false),
    CharArray: ([size, init], _named, lambda) => listOfSize(size, lambda ?? init ?? '\u0000'),
    intArrayOf: (args) => [...args],
    byteArrayOf: (args) => [...args],
    longArrayOf: (args) => [...args],
    doubleArrayOf: (args) => [...args],
    floatArrayOf: (args) => [...args],
    charArrayOf: (args) => [...args],
    booleanArrayOf: (args) => [...args],
    sequenceOf: (args) => [...args],
    emptySequence: () => [],
    // `kotlin.math`'s `min`/`max` of two values.
    min: ([left, right]) => (KT.cmp(left, right) <= 0 ? left : right),
    max: ([left, right]) => (KT.cmp(left, right) >= 0 ? left : right),
    // There's one thread: `synchronized(lock) { … }` runs the block.
    synchronized: async (_args, _named, lambda) => lambda(),
    // A coroutine scope of its own (`CoroutineScope(Dispatchers.IO)`): its jobs run at once.
    CoroutineScope: () => coroutineScope(),
    // Keiyoushi's `ratingStars(rating, max)`: five stars with halves.
    ratingStars: ([rating, max = 10], named_) => ratingStars(rating, named_?.max ?? max),
    PropertyDelegateProvider: (args, _named, lambda) => delegateProvider(lambda ?? args[0]),
    // org.json's `JSONObject(text)` and `JSONArray(text)`.
    JSONObject: ([text]) =>
      orgJsonObject(
        text === undefined ? {} : typeof text === 'string' ? JSON.parse(text) : plain(text),
      ),
    JSONArray: ([text]) =>
      orgJsonArray(
        text === undefined ? [] : typeof text === 'string' ? JSON.parse(text) : plain(text),
      ),
    // `GZIPOutputStream(out)`, `DeflaterOutputStream(out)`: what's written, compressed into
    // `out` once the stream closes.
    GZIPOutputStream: ([target]) => compressingStream(target, 'gzip'),
    DeflaterOutputStream: ([target]) => compressingStream(target, 'zlib'),
    // `BigInteger("ff", 16)`, `BigInteger(1, magnitude)`, `BigInteger(twosComplement)`.
    BigInteger: ([first, second]) => {
      const bytesLike = (value) => Array.isArray(value) || value instanceof Uint8Array;
      if (typeof first === 'number' && bytesLike(second)) {
        const magnitude = bigIntOfBytes(second, false);
        return first < 0 ? -magnitude : magnitude;
      }
      if (bytesLike(first)) return bigIntOfBytes(first, true);
      return bigIntOf(first, second ?? 10);
    },
  };
  /// A coroutine scope: `launch` runs its block (not awaited), `async` a deferred value.
  const coroutineScope = () => ({
    __class: 'CoroutineScope',
    __classes: ['CoroutineScope'],
    launch: (...rest) => {
      const block = rest.find((arg) => typeof arg === 'function');
      const job = block ? Promise.resolve().then(() => block()) : Promise.resolve();
      job.catch(() => {});
      return { join: () => job, cancel: () => KT.unit, isActive: true };
    },
    async: (...rest) => {
      const block = rest.find((arg) => typeof arg === 'function');
      return KT.deferred(async () => (block ? block() : null));
    },
    cancel: () => KT.unit,
  });
  /// Keiyoushi's `ratingStars`: a rating out of `max` as five stars, with half stars.
  const ratingStars = (rating, max = 10) => {
    const halves = Math.min(10, Math.max(0, Math.ceil((rating * 10) / max - 0.5)));
    return (
      '★'.repeat(Math.trunc(halves / 2)) +
      '✬'.repeat(halves % 2) +
      '☆'.repeat(5 - Math.trunc((halves + 1) / 2))
    );
  };
  const instant = (ms) => moment(ms, 'Instant');
  /// An ISO 8601 instant (`1999-03-22T05:06:07.000Z`, `…+01:00`) in milliseconds, or null; one
  /// without a zone is read as UTC.
  const ISO_INSTANT =
    /^\d{4}-\d{2}-\d{2}[Tt ]\d{2}:\d{2}(:\d{2}(\.\d{1,9})?)?([Zz]|[+-]\d{2}(:?\d{2}(:?\d{2})?)?)?$/;
  const isoInstant = (text) => {
    if (text === null || text === undefined) return null;
    let value = KT.str(text).trim();
    const found = ISO_INSTANT.exec(value);
    if (!found) return null;
    value = value.replace(' ', 'T').replace(/\.(\d{3})\d+/, '.$1');
    if (!found[3]) value += 'Z';
    else if (/^[+-]\d{2}$/.test(found[3])) value += ':00';
    else if (/^[+-]\d{4}$/.test(found[3])) value = `${value.slice(0, -2)}:${value.slice(-2)}`;
    const ms = Date.parse(value);
    return Number.isNaN(ms) ? null : ms;
  };
  const jsonCodec = {
    decodeFromString: (text) => JSON.parse(KT.str(text)),
    parseToJsonElement: (text) => JSON.parse(KT.str(text)),
    // `encodeToString(String.serializer(), value)` too.
    encodeToString: (...args) => JSON.stringify(plain(args[args.length - 1])),
    decodeFromJsonElement: (...args) => args[args.length - 1],
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
    // `TimeZone.getOffset(date)`: milliseconds from UTC.
    getOffset: () => offset * 60000,
    getRawOffset: () => offset * 60000,
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
      getDayOfWeek: () => dayOfWeek(((new Date(ms).getUTCDay() + 6) % 7) + 1),
      getMonthValue: () => new Date(ms).getUTCMonth() + 1,
      getDayOfMonth: () => new Date(ms).getUTCDate(),
      getDayOfYear: () =>
        Math.floor((ms - Date.UTC(new Date(ms).getUTCFullYear(), 0, 1)) / DAY_MS) + 1,
      getHour: () => new Date(ms).getUTCHours(),
      getMinute: () => new Date(ms).getUTCMinutes(),
      getSecond: () => new Date(ms).getUTCSeconds(),
      lengthOfMonth: () => {
        const date = new Date(ms);
        return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0)).getUTCDate();
      },
      isLeapYear: () => {
        const year = new Date(ms).getUTCFullYear();
        return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
      },
      withDayOfMonth: (day) => shift((date) => date.setUTCDate(day)),
      withMonth: (month) => shift((date) => date.setUTCMonth(month - 1)),
      withYear: (year) => shift((date) => date.setUTCFullYear(year)),
      // A date at a time of day (`date.atTime(LocalTime.parse(…))`, `atTime(10, 30)`).
      atTime: (time, minute = 0, second = 0) =>
        moment(
          Math.floor(ms / DAY_MS) * DAY_MS +
            (time && typeof time === 'object'
              ? msOf(time.ms ?? 0) % DAY_MS
              : ((Number(time) * 60 + Number(minute)) * 60 + Number(second)) * 1000),
          'LocalDateTime',
        ),
      // A year-month's days (`YearMonth.atDay(1)`, `atEndOfMonth()`).
      atDay: (day) => moment(ms + (Number(day) - 1) * DAY_MS, 'LocalDate'),
      atEndOfMonth: () => moment(ms + (value.lengthOfMonth() - 1) * DAY_MS, 'LocalDate'),
      // `date.get(ChronoField.YEAR)`, `get(WeekFields.SUNDAY_START.dayOfWeek())`.
      get: (field) => fieldOf(ms, field),
      getLong: (field) => fieldOf(ms, field),
      format: (formatter) => formatter.format(value),
      isBefore: (other) => ms < msOf(other.ms ?? other),
      isAfter: (other) => ms > msOf(other.ms ?? other),
      isEqual: (other) => ms === msOf(other.ms ?? other),
      toString: () => {
        const iso = new Date(ms).toISOString();
        if (kind === 'Instant') return iso;
        if (kind === 'YearMonth') return iso.slice(0, 7);
        if (kind === 'LocalTime') return iso.slice(11, iso.slice(17, 19) === '00' ? 16 : 19);
        return iso.slice(0, kind === 'LocalDate' ? 10 : 19);
      },
    };
    return value;
  };
  /// A date's field (`ChronoField`'s names, a `WeekFields` field).
  const fieldOf = (ms, field) => {
    const date = new Date(ms);
    if (field && field.isWeekField) {
      const iso = ((date.getUTCDay() + 6) % 7) + 1;
      if (field.of === 'dayOfWeek') return ((iso - field.weekStart + 7) % 7) + 1;
      const start = Date.UTC(
        date.getUTCFullYear(),
        field.of === 'weekOfMonth' ? date.getUTCMonth() : 0,
        1,
      );
      const shiftDays = (new Date(start).getUTCDay() - (field.weekStart % 7) + 7) % 7;
      return Math.floor((Math.floor((ms - start) / DAY_MS) + shiftDays) / 7) + 1;
    }
    switch (KT.str(field)) {
      case 'YEAR':
      case 'YEAR_OF_ERA':
        return date.getUTCFullYear();
      case 'MONTH_OF_YEAR':
        return date.getUTCMonth() + 1;
      case 'DAY_OF_MONTH':
        return date.getUTCDate();
      case 'DAY_OF_WEEK':
        return ((date.getUTCDay() + 6) % 7) + 1;
      case 'DAY_OF_YEAR':
        return Math.floor((ms - Date.UTC(date.getUTCFullYear(), 0, 1)) / DAY_MS) + 1;
      case 'HOUR_OF_DAY':
        return date.getUTCHours();
      case 'MINUTE_OF_HOUR':
        return date.getUTCMinutes();
      case 'SECOND_OF_MINUTE':
        return date.getUTCSeconds();
      case 'EPOCH_DAY':
        return Math.floor(ms / DAY_MS);
      default:
        return 0;
    }
  };
  /// java.time's `WeekFields`: weeks starting on `weekStart` (1 Monday … 7 Sunday).
  const weekFields = (weekStart) => ({
    __class: 'WeekFields',
    dayOfWeek: () => ({ isWeekField: true, of: 'dayOfWeek', weekStart }),
    weekOfMonth: () => ({ isWeekField: true, of: 'weekOfMonth', weekStart }),
    weekOfYear: () => ({ isWeekField: true, of: 'weekOfYear', weekStart }),
    weekOfWeekBasedYear: () => ({ isWeekField: true, of: 'weekOfYear', weekStart }),
    getFirstDayOfWeek: () => dayOfWeek(weekStart),
  });
  /// A day's milliseconds in a zone (the device's when none is given), as java.time's `now(zone)`.
  const nowIn = (where) =>
    Date.now() + (where === undefined || where === null ? localOffset() : offsetOf(where)) * 60000;
  const msOfTemporal = (value) =>
    value && typeof value === 'object' && typeof value.ms === 'number' ? value.ms : Number(value);
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
      // `preferences.all`: every value kept, by key.
      getAll: () => new Map(store),
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
  // Keiyoushi's Markdown descriptions (`keiyoushi.utils.MarkdownDescription`, `toMarkdown`), as
  // the plain text Raygun shows: sections a blank line apart, a run of inline ones (rating,
  // facts, fields) on one line.
  /// An element's text with its paragraphs and line breaks.
  const htmlText = (element, keepNewlines = false) => {
    if (element === null || element === undefined) return '';
    let html = KT.str(typeof element.html === 'function' ? element.html() : element);
    if (keepNewlines) html = html.replace(/\r?\n/g, '<br>');
    const text = html
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/(p|div|h[1-6]|li|ul|ol|blockquote|section|article|tr|table)>/gi, '\n\n')
      .replace(/<li[^>]*>/gi, '- ')
      .replace(/<[^>]+>/g, '');
    return unescapeEntities(text)
      .replace(/[ \t ]+/g, ' ')
      .replace(/ *\n */g, '\n')
      .replace(/\n{3,}/g, '\n\n')
      .trim();
  };
  const markdownBuilder = () => {
    const sections = [];
    let lastInline = false;
    const add = (section) => {
      if (section === null || section === undefined || KT.str(section).trim() === '') return;
      sections.push(KT.str(section));
      lastInline = false;
    };
    const addInline = (text) => {
      if (text === null || text === undefined || KT.str(text).trim() === '') return;
      if (lastInline) sections[sections.length - 1] += ` · ${text}`;
      else sections.push(KT.str(text));
      lastInline = true;
    };
    const list = (label, items) => {
      const lines = items.map((item) => KT.str(item).trim()).filter(Boolean);
      return lines.length
        ? `${KT.str(label)}:\n${lines.map((line) => `- ${line}`).join('\n')}`
        : null;
    };
    const builder = {
      rating: (rating, ...rest) => {
        const named_ = rest.find((arg) => arg && arg[NAMED]) ?? {};
        const max = named_.max ?? (typeof rest[0] === 'number' ? rest[0] : 10);
        const votes = named_.votes ?? (typeof rest[1] === 'number' ? rest[1] : null);
        if (typeof rating === 'number' && Number.isFinite(rating)) {
          const number = String(Math.round(rating * 100) / 100);
          addInline(`${ratingStars(rating, max)} ${number}${votes !== null ? ` (${votes})` : ''}`);
        }
        return builder;
      },
      info: (...facts) => {
        addInline(
          facts
            .filter((fact) => typeof fact === 'string' && fact.trim() !== '')
            .map((fact) => fact.trim())
            .join(' · '),
        );
        return builder;
      },
      description: (value, ...rest) => {
        const keepNewlines = rest.find((arg) => typeof arg === 'boolean') ?? false;
        add(typeof value === 'string' ? value.trim() : htmlText(value, keepNewlines));
        return builder;
      },
      markdown: (text) => {
        add(typeof text === 'string' ? text.trim() : null);
        return builder;
      },
      field: (label, value) => {
        const text = Array.isArray(value)
          ? value
              .map((item) => KT.str(item).trim())
              .filter(Boolean)
              .join(', ')
          : typeof value === 'string'
            ? value.trim()
            : '';
        if (text) addInline(`${KT.str(label)}: ${text}`);
        return builder;
      },
      altTitles: (titles, ...rest) => {
        const label = rest.find((arg) => typeof arg === 'string') ?? 'Alternative Names';
        const names = Array.isArray(titles) ? titles : titles ? [titles] : [];
        add(list(label, [...new Set(names.map((title) => KT.str(title).trim()))]));
        return builder;
      },
      links: (links, ...rest) => {
        const label = rest.find((arg) => typeof arg === 'string') ?? 'Links';
        add(
          list(
            label,
            (links ?? [])
              .filter((link) => link && link.second)
              .map((link) => `${KT.str(link.first)}: ${KT.str(link.second)}`),
          ),
        );
        return builder;
      },
      // A line between sections, as plain text.
      separator: () => {
        add('---');
        return builder;
      },
      build: () => (sections.length ? sections.join('\n\n') : null),
    };
    return builder;
  };

  /// Kotlin's `Random` (`Random.nextInt(…)`, `Random.Default.nextBytes(16)`).
  const random = {
    nextInt: (from, until) => {
      if (until === undefined && from === undefined) return Math.floor(Math.random() * 2 ** 31);
      const [low, high] = until === undefined ? [0, from] : [from, until];
      return low + Math.floor(Math.random() * (high - low));
    },
    nextLong: (from, until) => random.nextInt(from ?? 0, until ?? Number.MAX_SAFE_INTEGER),
    nextDouble: () => Math.random(),
    nextFloat: () => Math.random(),
    nextBoolean: () => Math.random() < 0.5,
    nextBytes: (sizeOrArray) => {
      const size = Array.isArray(sizeOrArray) ? sizeOrArray.length : Number(sizeOrArray) || 0;
      const bytes = Array.from({ length: size }, () => Math.floor(Math.random() * 256));
      if (Array.isArray(sizeOrArray)) {
        bytes.forEach((byte, index) => (sizeOrArray[index] = byte));
        return sizeOrArray;
      }
      return bytes;
    },
  };
  random.Default = random;
  /// Android's `Build`: a phone's, as an extension asks for a User-Agent's parts.
  /// Java's `Integer` and `Long` helpers on 32 and 64 bits (unsigned readings, rotations, bit
  /// counts), as hashes and shuffles use them.
  const radixOf = (radix) => Number(radix ?? 10);
  const javaInteger = {
    MAX_VALUE: 2147483647,
    MIN_VALUE: -2147483648,
    SIZE: 32,
    BYTES: 4,
    parseInt: (text, radix = 10) => KT.nn(toInt(text, radixOf(radix))),
    valueOf: (value, radix = 10) =>
      typeof value === 'number' ? value : KT.nn(toInt(value, radixOf(radix))),
    parseUnsignedInt: (text, radix = 10) => parseInt(KT.str(text), radixOf(radix)) | 0,
    toString: (value, radix = 10) => (value | 0).toString(radixOf(radix)),
    toHexString: (value) => (value >>> 0).toString(16),
    toBinaryString: (value) => (value >>> 0).toString(2),
    toOctalString: (value) => (value >>> 0).toString(8),
    toUnsignedString: (value, radix = 10) => (value >>> 0).toString(radixOf(radix)),
    toUnsignedLong: (value) => value >>> 0,
    compare: (left, right) => (left < right ? -1 : left > right ? 1 : 0),
    compareUnsigned: (left, right) => {
      const [a, b] = [left >>> 0, right >>> 0];
      return a < b ? -1 : a > b ? 1 : 0;
    },
    divideUnsigned: (left, right) => Math.trunc((left >>> 0) / (right >>> 0)) | 0,
    remainderUnsigned: (left, right) => ((left >>> 0) % (right >>> 0)) | 0,
    rotateLeft: (value, distance) => (value << distance) | (value >>> -distance) | 0,
    rotateRight: (value, distance) => (value >>> distance) | (value << -distance) | 0,
    numberOfLeadingZeros: (value) => Math.clz32(value),
    numberOfTrailingZeros: (value) => (value === 0 ? 32 : 31 - Math.clz32(value & -value)),
    bitCount: (value) => {
      let bits = value >>> 0;
      let count = 0;
      for (; bits; bits >>>= 1) count += bits & 1;
      return count;
    },
    highestOneBit: (value) => (value === 0 ? 0 : (1 << (31 - Math.clz32(value))) | 0),
    lowestOneBit: (value) => value & -value,
    reverseBytes: (value) =>
      ((value & 255) << 24) | ((value & 0xff00) << 8) | ((value >>> 8) & 0xff00) | (value >>> 24),
    signum: (value) => Math.sign(value),
    sum: (left, right) => (left + right) | 0,
    max: (left, right) => Math.max(left, right),
    min: (left, right) => Math.min(left, right),
    hashCode: (value) => value | 0,
  };
  const javaLong = {
    MAX_VALUE: Number.MAX_SAFE_INTEGER,
    MIN_VALUE: -Number.MAX_SAFE_INTEGER,
    SIZE: 64,
    BYTES: 8,
    SIZE_BITS: 64,
    SIZE_BYTES: 8,
    parseLong: (text, radix = 10) => KT.nn(toInt(text, radixOf(radix))),
    valueOf: (value, radix = 10) =>
      typeof value === 'number' ? value : KT.nn(toInt(value, radixOf(radix))),
    toString: (value, radix = 10) => big(value).toString(radixOf(radix)),
    toHexString: (value) => BigInt.asUintN(64, big(value)).toString(16),
    toBinaryString: (value) => BigInt.asUintN(64, big(value)).toString(2),
    toUnsignedString: (value, radix = 10) =>
      BigInt.asUintN(64, big(value)).toString(radixOf(radix)),
    compare: (left, right) => (left < right ? -1 : left > right ? 1 : 0),
    compareUnsigned: (left, right) => {
      const [a, b] = [BigInt.asUintN(64, big(left)), BigInt.asUintN(64, big(right))];
      return a < b ? -1 : a > b ? 1 : 0;
    },
    divideUnsigned: (left, right) =>
      fromBig(BigInt.asUintN(64, big(left)) / BigInt.asUintN(64, big(right))),
    remainderUnsigned: (left, right) =>
      fromBig(BigInt.asUintN(64, big(left)) % BigInt.asUintN(64, big(right))),
    rotateLeft: (value, distance) => {
      const bits = BigInt.asUintN(64, big(value));
      const shift = BigInt(distance & 63);
      return fromBig(BigInt.asUintN(64, (bits << shift) | (bits >> ((64n - shift) & 63n))));
    },
    numberOfLeadingZeros: (value) => {
      const bits = BigInt.asUintN(64, big(value));
      return bits === 0n ? 64 : 64 - bits.toString(2).length;
    },
    numberOfTrailingZeros: (value) => {
      const bits = BigInt.asUintN(64, big(value));
      if (bits === 0n) return 64;
      let count = 0;
      for (let rest = bits; (rest & 1n) === 0n; rest >>= 1n) count += 1;
      return count;
    },
    bitCount: (value) =>
      [...BigInt.asUintN(64, big(value)).toString(2)].filter((d) => d === '1').length,
    signum: (value) => Math.sign(value),
    max: (left, right) => Math.max(left, right),
    min: (left, right) => Math.min(left, right),
    hashCode: (value) => Number(BigInt.asIntN(32, big(value) ^ (big(value) >> 32n))),
  };
  const build = {
    VERSION: { RELEASE: '14', SDK_INT: 34, CODENAME: 'REL', INCREMENTAL: '1' },
    // Android's API levels by name (`Build.VERSION.SDK_INT >= Build.VERSION_CODES.S`).
    VERSION_CODES: {
      LOLLIPOP: 21,
      M: 23,
      N: 24,
      O: 26,
      O_MR1: 27,
      P: 28,
      Q: 29,
      R: 30,
      S: 31,
      S_V2: 32,
      TIRAMISU: 33,
      UPSIDE_DOWN_CAKE: 34,
      VANILLA_ICE_CREAM: 35,
    },
    MODEL: 'Pixel 4',
    MANUFACTURER: 'Google',
    BRAND: 'google',
    DEVICE: 'flame',
    PRODUCT: 'flame',
    ID: 'UQ1A.240205.002',
  };
  /// Java's `Normalizer.normalize(text, Normalizer.Form.NFD)`.
  const normalizer = {
    Form: { NFC: 'NFC', NFD: 'NFD', NFKC: 'NFKC', NFKD: 'NFKD' },
    normalize: (text, form) => KT.str(text).normalize(KT.str(form ?? 'NFC')),
  };

  /// OkHttp's `MultipartBody.Builder()…addFormDataPart(…)…build()`: a form of text parts.
  const multipartBuilder = (given = null) => {
    const boundary = given ? KT.str(given) : `raygun${Math.random().toString(16).slice(2)}`;
    let type = 'multipart/form-data';
    const parts = [];
    const builder = {
      setType: (value) => ((type = KT.str(value)), builder),
      addFormDataPart: (name, ...rest) => {
        const [filename, body] = rest.length >= 2 ? rest : [null, rest[0]];
        const text = body && body.isBody ? body.text : KT.str(body);
        const disposition = `form-data; name="${KT.str(name)}"${filename ? `; filename="${KT.str(filename)}"` : ''}`;
        const contentType =
          body && body.isBody && body.type ? `\r\nContent-Type: ${body.type}` : '';
        parts.push(`Content-Disposition: ${disposition}${contentType}\r\n\r\n${text}`);
        return builder;
      },
      addPart: (body) => {
        parts.push(`\r\n${body && body.isBody ? body.text : KT.str(body)}`);
        return builder;
      },
      build: () => ({
        isBody: true,
        text: `${parts.map((part) => `--${boundary}\r\n${part}\r\n`).join('')}--${boundary}--\r\n`,
        type: `${type}; boundary=${boundary}`,
      }),
    };
    return builder;
  };

  const OBJECTS = {
    // Java's `Collator`, kotlinx.coroutines' dispatchers, Jsoup's evaluators, OkHttp's `HttpUrl`,
    // Android's `Uri`, numbers' limits and `Thread.sleep` (above).
    Collator: {
      getInstance: (locale) => collator(locale),
      PRIMARY: 0,
      SECONDARY: 1,
      TERTIARY: 2,
      IDENTICAL: 3,
      NO_DECOMPOSITION: 0,
      CANONICAL_DECOMPOSITION: 1,
      FULL_DECOMPOSITION: 2,
    },
    Dispatchers: dispatchers,
    Evaluator: evaluator,
    HttpUrl: {
      Builder: () => new HttpUrlBuilder(null),
      parse: (text) => HttpUrl.parse(text),
      get: (text) => HttpUrl.parse(text),
      Companion: {
        parse: (text) => HttpUrl.parse(text),
        get: (text) => HttpUrl.parse(text),
      },
    },
    Uri: {
      parse: (text) => javaUri(text),
      encode: (text, allow) => uriEncode(text, allow),
      decode: (text) => decodeURIComponent(KT.str(text)),
      EMPTY: javaUri(''),
    },
    Float: {
      MAX_VALUE: 3.4028234663852886e38,
      MIN_VALUE: 1.401298464324817e-45,
      POSITIVE_INFINITY: Infinity,
      NEGATIVE_INFINITY: -Infinity,
      NaN,
    },
    Double: {
      MAX_VALUE: Number.MAX_VALUE,
      MIN_VALUE: Number.MIN_VALUE,
      POSITIVE_INFINITY: Infinity,
      NEGATIVE_INFINITY: -Infinity,
      NaN,
    },
    Thread: { sleep: (ms) => pause(ms) },
    // `javaClass.getResource(…)` in the source's own code, and Keiyoushi's `jsonInstance`.
    javaClass: KT.javaClass('Source'),
    jsonInstance: jsonCodec,
    // OkHttp's `Response.Builder()` and `Protocol`, for answers an interceptor makes.
    Response: {
      Builder: () =>
        responseBuilder({
          code: 200,
          headers: new Headers([]),
          body: null,
          request: null,
          message: 'OK',
        }),
    },
    Protocol: { HTTP_1_0: 'http/1.0', HTTP_1_1: 'http/1.1', HTTP_2: 'h2', QUIC: 'quic' },
    Resources: { getSystem: () => ({ displayMetrics }) },
    DayOfWeek: {
      ...Object.fromEntries(DAY_NAMES.map((name, index) => [name, dayOfWeek(index + 1)])),
      entries: DAY_NAMES.map((_, index) => dayOfWeek(index + 1)),
      values: () => DAY_NAMES.map((_, index) => dayOfWeek(index + 1)),
      of: (value) => dayOfWeek(value),
    },
    TextStyle: {
      FULL: 'FULL',
      SHORT: 'SHORT',
      NARROW: 'NARROW',
      FULL_STANDALONE: 'FULL',
      SHORT_STANDALONE: 'SHORT',
    },
    Injekt: { get: () => application, getInstance: () => application },
    // Keiyoushi's `applicationContext`, and Android's default preferences.
    applicationContext: application,
    // The extension's build: Mihon's names for it (sources put them in their User-Agent).
    BuildConfig: { VERSION_NAME: '1.4.0', VERSION_CODE: 1, DEBUG: false, BUILD_TYPE: 'release' },
    PreferenceManager: { getDefaultSharedPreferences: () => sharedPreferences },
    // Mihon's three-state filter's states, by name too.
    TriState: { STATE_IGNORE: 0, STATE_INCLUDE: 1, STATE_EXCLUDE: 2 },
    STATE_IGNORE: 0,
    STATE_INCLUDE: 1,
    STATE_EXCLUDE: 2,
    RequestBody: {
      EMPTY: { isBody: true, text: '', type: null },
      create: (type, content) => ({ isBody: true, text: KT.str(content), type: KT.str(type) }),
    },
    CookieJar: { NO_COOKIES: { loadForRequest: () => [], saveFromResponse: () => KT.unit } },
    Credentials: {
      basic: (user, password) =>
        `Basic ${K.bin.toBase64(K.bin.utf8(`${KT.str(user)}:${KT.str(password)}`))}`,
    },
    Html: {
      escapeHtml: (text) =>
        KT.str(text)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;'),
      fromHtml: (text) => unescapeEntities(KT.str(text).replace(/<[^>]+>/g, '')),
      FROM_HTML_MODE_LEGACY: 0,
      FROM_HTML_MODE_COMPACT: 63,
    },
    HttpURLConnection: {
      HTTP_OK: 200,
      HTTP_MOVED_PERM: 301,
      HTTP_MOVED_TEMP: 302,
      HTTP_NOT_MODIFIED: 304,
      HTTP_BAD_REQUEST: 400,
      HTTP_UNAUTHORIZED: 401,
      HTTP_FORBIDDEN: 403,
      HTTP_NOT_FOUND: 404,
      HTTP_INTERNAL_ERROR: 500,
      HTTP_UNAVAILABLE: 503,
    },
    // java.time's `Duration`, as Kotlin's durations are here.
    Duration: {
      ofMillis: (n) => duration(Number(n)),
      ofSeconds: (n) => duration(Number(n) * 1000),
      ofMinutes: (n) => duration(Number(n) * 60000),
      ofHours: (n) => duration(Number(n) * 3600000),
      ofDays: (n) => duration(Number(n) * 86400000),
      ZERO: duration(0),
      INFINITE: duration(Infinity),
    },
    CharCategory: Object.fromEntries(CHAR_CATEGORIES.map(([, name]) => [name, name])),
    StaticLayout: { Builder: { obtain: staticLayoutBuilder } },
    Layout: layoutStatics,
    LineBreaker: layoutStatics,
    DurationUnit: {
      NANOSECONDS: timeUnit('NANOSECONDS', 1e-6),
      MICROSECONDS: timeUnit('MICROSECONDS', 1e-3),
      MILLISECONDS: timeUnit('MILLISECONDS', 1),
      SECONDS: timeUnit('SECONDS', 1000),
      MINUTES: timeUnit('MINUTES', 60000),
      HOURS: timeUnit('HOURS', 3600000),
      DAYS: timeUnit('DAYS', 86400000),
    },
    Collections: {
      synchronizedMap: (map) => map,
      synchronizedList: (list) => list,
      synchronizedSet: (set) => set,
      unmodifiableList: (list) => list,
      unmodifiableMap: (map) => map,
      emptyList: () => [],
      emptyMap: () => new Map(),
    },
    LazyThreadSafetyMode: { SYNCHRONIZED: 0, PUBLICATION: 1, NONE: 2 },
    URLUtil: {
      isValidUrl: (url) => /^(https?|file|content|data):/i.test(KT.str(url)),
      isNetworkUrl: (url) => /^https?:\/\//i.test(KT.str(url)),
      isHttpsUrl: (url) => /^https:\/\//i.test(KT.str(url)),
    },
    // Kotlin's `Result.success(…)` and `Result.failure(…)`.
    Result: {
      success: (value) => new Result(true, value),
      failure: (error) => new Result(false, null, KT.error(error)),
    },
    FORCE_NETWORK: { isCacheControl: true },
    FORCE_CACHE: { isCacheControl: true },
    // kotlin.time's `TimeSource.Monotonic.markNow().elapsedNow()`.
    // Java's `Runtime`: a phone's, for sources sizing work by it.
    File: {
      createTempFile: (prefix, suffix = '.tmp', directory = null) =>
        javaFile(
          directory ?? '/cache',
          `${KT.str(prefix)}${Math.floor(Math.random() * 1e9)}${KT.str(suffix ?? '.tmp')}`,
        ),
      separator: '/',
    },
    Runtime: {
      getRuntime: () => ({
        availableProcessors: () => 8,
        maxMemory: () => 512 * 1024 * 1024,
        totalMemory: () => 256 * 1024 * 1024,
        freeMemory: () => 128 * 1024 * 1024,
      }),
    },
    RoundingMode: {
      UP: 'UP',
      DOWN: 'DOWN',
      CEILING: 'CEILING',
      FLOOR: 'FLOOR',
      HALF_UP: 'HALF_UP',
      HALF_DOWN: 'HALF_DOWN',
      HALF_EVEN: 'HALF_EVEN',
    },
    // Jsoup's `Validate`: what a check that fails says.
    Validate: {
      notEmpty: (value, message) =>
        value === null || value === undefined || KT.str(value) === ''
          ? KT.raise(message ?? 'String must not be empty')
          : KT.unit,
      notNull: (value, message) =>
        value === null || value === undefined
          ? KT.raise(message ?? 'Object must not be null')
          : KT.unit,
      isTrue: (value, message) => (value ? KT.unit : KT.raise(message ?? 'Must be true')),
    },
    TimeSource: {
      Monotonic: {
        markNow: () => timeMark(Date.now()),
      },
    },
    GlobalScope: coroutineScope(),
    // `SSLContext.getInstance("TLS")`: Raygun's client checks certificates itself.
    SSLContext: {
      getInstance: () => ({ init: () => KT.unit, socketFactory: {}, getSocketFactory: () => ({}) }),
    },
    // `MonthDay.of(month, day).atYear(year)`.
    MonthDay: {
      of: (month, day) => ({
        __class: 'MonthDay',
        __classes: ['MonthDay'],
        monthValue: month,
        dayOfMonth: day,
        atYear: (year) => moment(Date.UTC(year, month - 1, day), 'LocalDate'),
      }),
    },
    // `Year.now().value`, `OffsetDateTime.parse(text)`.
    Year: {
      now: () => {
        const value = new Date().getUTCFullYear();
        return { __class: 'Year', __classes: ['Year'], value, getValue: () => value };
      },
    },
    OffsetDateTime: {
      parse: (text) => moment(wallClock(text, null, 'ZonedDateTime'), 'ZonedDateTime'),
    },
    // Android's graphics and Java's `ByteBuffer` (above); `lib:imagedecoder`'s `ImageDecoder`
    // and `lib:textinterceptor`'s helper.
    Bitmap: bitmapStatics,
    BitmapFactory: bitmapFactory,
    Color: colorStatics,
    Paint: paintStatics,
    Typeface: typefaceStatics,
    ByteBuffer: {
      wrap: (array, offset = 0, length = undefined) => byteBuffer(array, offset, length),
      allocate: (size) => byteBuffer(new Array(size).fill(0)),
      allocateDirect: (size) => byteBuffer(new Array(size).fill(0)),
    },
    ByteOrder: {
      LITTLE_ENDIAN: 'LITTLE_ENDIAN',
      BIG_ENDIAN: 'BIG_ENDIAN',
      nativeOrder: () => 'LITTLE_ENDIAN',
    },
    ImageDecoder: imageDecoder,
    TextInterceptorHelper: textInterceptorHelper,
    // Keiyoushi's `JSON_MEDIA_TYPE`.
    JSON_MEDIA_TYPE: 'application/json',
    // Mihon's `QuickJs`: a site's script, run apart (`runtime.rs`'s `evaluate`).
    QuickJs: {
      // Each `evaluate` runs in an engine of its own; one engine's earlier scripts (a library
      // it loads, then a call into it) run again before the next.
      create: () => {
        const ran = [];
        return {
          __class: 'QuickJs',
          __classes: ['QuickJs'],
          evaluate: (code) => {
            const script = KT.str(code);
            const value = K.evaluate([...ran, script].join(';\n'));
            ran.push(script);
            return value;
          },
          close: () => KT.unit,
        };
      },
    },
    MultipartBody: {
      Builder: (boundary = null) => multipartBuilder(boundary),
      FORM: 'multipart/form-data',
      MIXED: 'multipart/mixed',
    },
    // Mihon's `UpdateStrategy`: Raygun refreshes every title alike.
    UpdateStrategy: { ALWAYS_UPDATE: 0, ONLY_FETCH_ONCE: 1 },
    Random: random,
    Build: build,
    VERSION: build.VERSION,
    Normalizer: normalizer,
    Entities: {
      unescape: (text) => unescapeEntities(text),
      escape: (text) =>
        KT.str(text)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;'),
    },
    StandardCharsets: { UTF_8: 'UTF-8', ISO_8859_1: 'ISO-8859-1', US_ASCII: 'US-ASCII' },
    Charset: { forName: (name) => KT.str(name), defaultCharset: () => 'UTF-8' },
    MarkdownDescription: { Builder: () => markdownBuilder() },
    // OkHttp's `Cookie` and Android's `CookieManager`, over the extension's cookie jar.
    Cookie: {
      parse: (url, line) => parseCookie(url, line),
      parseAll: (url, headers) =>
        (headers?.values ? headers.values('Set-Cookie') : [])
          .map((line) => parseCookie(url, line))
          .filter(Boolean),
      Builder: () => cookieBuilder(),
    },
    CookieManager: { getInstance: () => cookieManager },
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
      // `Jsoup.parse(text, base, Parser.xmlParser())`, or `(text, parser)`.
      parse: (html, base = '', parser = null) => {
        if (base && base.__class === 'Parser') [base, parser] = ['', base];
        return Jsoup.parse(html, KT.str(base ?? ''), Boolean(parser && parser.xml));
      },
      parseBodyFragment: (html, base = '') =>
        Jsoup.parse(`<body>${KT.str(html)}</body>`, KT.str(base)),
      clean: (html) => Jsoup.parse(html).text(),
    },
    Fetch,
    // Java's locale constants (`Locale.FRENCH` reads French month names).
    Locale: {
      ENGLISH: K.locale('en'),
      US: K.locale('en', 'US'),
      ROOT: K.locale('root'),
      UK: K.locale('en', 'GB'),
      CANADA: K.locale('en', 'CA'),
      FRENCH: K.locale('fr'),
      FRANCE: K.locale('fr', 'FR'),
      CANADA_FRENCH: K.locale('fr', 'CA'),
      GERMAN: K.locale('de'),
      GERMANY: K.locale('de', 'DE'),
      ITALIAN: K.locale('it'),
      ITALY: K.locale('it', 'IT'),
      JAPANESE: K.locale('ja'),
      JAPAN: K.locale('ja', 'JP'),
      KOREAN: K.locale('ko'),
      KOREA: K.locale('ko', 'KR'),
      CHINESE: K.locale('zh'),
      CHINA: K.locale('zh', 'CN'),
      PRC: K.locale('zh', 'CN'),
      SIMPLIFIED_CHINESE: K.locale('zh', 'CN'),
      TRADITIONAL_CHINESE: K.locale('zh', 'TW'),
      TAIWAN: K.locale('zh', 'TW'),
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
      NANOSECONDS: timeUnit('NANOSECONDS', 1e-6),
      MICROSECONDS: timeUnit('MICROSECONDS', 1e-3),
      MILLISECONDS: timeUnit('MILLISECONDS', 1),
      SECONDS: timeUnit('SECONDS', 1000),
      MINUTES: timeUnit('MINUTES', 60000),
      HOURS: timeUnit('HOURS', 3600000),
      DAYS: timeUnit('DAYS', 86400000),
    },
    CacheControl: {
      FORCE_NETWORK: { isCacheControl: true },
      FORCE_CACHE: { isCacheControl: true },
      // Raygun's client keeps no HTTP cache: a request's cache settings change nothing.
      Builder: () => {
        const builder = {
          noCache: () => builder,
          noStore: () => builder,
          noTransform: () => builder,
          onlyIfCached: () => builder,
          immutable: () => builder,
          maxAge: () => builder,
          maxStale: () => builder,
          minFresh: () => builder,
          build: () => ({ isCacheControl: true }),
        };
        return builder;
      },
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
      // `create(type, content)` or `create(content, type)`: text, or bytes. A media type is
      // text here too, so the one that reads as a type is the type.
      create: (first, second) => {
        const typeLike = (value) =>
          typeof value === 'string' && /^[\w.+-]+\/[\w.+-]+(\s*;.*)?$/.test(value.trim());
        let [content, type] = [first, second];
        if ((typeLike(first) && !typeLike(second)) || first === null || first === undefined)
          [content, type] = [second, first];
        if (typeof content === 'string') return KT.textBody(content, type ?? null);
        return bytesBody(async () => u8Of(content), type ?? null);
      },
    },
    String: {
      // `String.format(Locale.US, "%02d", n)` too: the locale changes nothing here.
      format: (...args) => {
        if (args[0] && typeof args[0] === 'object' && args[0].language !== undefined) args.shift();
        const [pattern, ...rest] = args;
        return K.format(pattern, ...rest);
      },
      valueOf: (value) => KT.str(value),
      CASE_INSENSITIVE_ORDER: caseInsensitiveOrder,
      serializer: () => ({ isSerializer: true, name: 'String' }),
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
    Log: { e: () => 0, w: () => 0, i: () => 0, d: () => 0, v: () => 0, wtf: () => 0 },
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
    ChronoField: Object.fromEntries(Object.keys(CHRONO_FIELDS).map((name) => [name, name])),
    SignStyle: {
      NORMAL: 'NORMAL',
      ALWAYS: 'ALWAYS',
      NEVER: 'NEVER',
      NOT_NEGATIVE: 'NOT_NEGATIVE',
      EXCEEDS_PAD: 'EXCEEDS_PAD',
    },
    FormatStyle: { FULL: 'FULL', LONG: 'LONG', MEDIUM: 'MEDIUM', SHORT: 'SHORT' },
    LocalDate: {
      now: (where) => moment(Math.floor(nowIn(where) / DAY_MS) * DAY_MS, 'LocalDate'),
      parse: (text, format) => moment(wallClock(text, format, 'LocalDate'), 'LocalDate'),
      of: (year, month, day) => moment(Date.UTC(year, month - 1, day), 'LocalDate'),
      ofEpochDay: (days) => moment(days * DAY_MS, 'LocalDate'),
      from: (value) => moment(Math.floor(msOfTemporal(value) / DAY_MS) * DAY_MS, 'LocalDate'),
      MIN: moment(-8.64e15, 'LocalDate'),
      MAX: moment(8.64e15, 'LocalDate'),
      EPOCH: moment(0, 'LocalDate'),
    },
    LocalDateTime: {
      now: (where) => moment(nowIn(where), 'LocalDateTime'),
      parse: (text, format) => moment(wallClock(text, format, 'LocalDateTime'), 'LocalDateTime'),
      of: (year, month, day, hour = 0, minute = 0, second = 0) =>
        moment(Date.UTC(year, month - 1, day, hour, minute, second), 'LocalDateTime'),
      from: (value) => moment(msOfTemporal(value), 'LocalDateTime'),
    },
    // `YearMonth.of(2024, 3)`, `YearMonth.from(date)`, `YearMonth.now()`: a month, at its first day.
    YearMonth: {
      now: (where) => {
        const date = new Date(nowIn(where));
        return moment(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1), 'YearMonth');
      },
      of: (year, month) => moment(Date.UTC(year, Number(month) - 1, 1), 'YearMonth'),
      from: (value) => {
        const date = new Date(msOfTemporal(value));
        return moment(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1), 'YearMonth');
      },
      parse: (text, format) => {
        const date = new Date(
          format
            ? wallClock(text, format, 'LocalDate')
            : Date.parse(`${KT.str(text).trim()}-01T00:00:00Z`),
        );
        if (Number.isNaN(date.getTime()))
          throw new KotlinError(
            `Text '${KT.str(text)}' could not be parsed`,
            'DateTimeParseException',
          );
        return moment(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1), 'YearMonth');
      },
    },
    // `LocalTime.parse("10:30", format)`, `LocalTime.of(10, 30)`: a time of day, in milliseconds.
    LocalTime: {
      parse: (text, format) => {
        if (format && typeof format.parse === 'function') {
          const read = format.parse(text);
          if (read === null)
            throw new KotlinError(
              `Text '${KT.str(text)}' could not be parsed`,
              'DateTimeParseException',
            );
          return moment(
            (((read + format.offsetMinutes * 60000) % DAY_MS) + DAY_MS) % DAY_MS,
            'LocalTime',
          );
        }
        const found = /^(\d{1,2}):(\d{2})(?::(\d{2}))?/.exec(KT.str(text).trim());
        if (!found)
          throw new KotlinError(
            `Text '${KT.str(text)}' could not be parsed`,
            'DateTimeParseException',
          );
        const [, hour, minute, second = '0'] = found;
        return moment(
          ((Number(hour) * 60 + Number(minute)) * 60 + Number(second)) * 1000,
          'LocalTime',
        );
      },
      of: (hour, minute = 0, second = 0) =>
        moment(((Number(hour) * 60 + Number(minute)) * 60 + Number(second)) * 1000, 'LocalTime'),
      now: (where) => moment(nowIn(where) % DAY_MS, 'LocalTime'),
      MIDNIGHT: moment(0, 'LocalTime'),
      MIN: moment(0, 'LocalTime'),
      NOON: moment(12 * 3600000, 'LocalTime'),
      MAX: moment(DAY_MS - 1, 'LocalTime'),
    },
    WeekFields: {
      ISO: weekFields(1),
      SUNDAY_START: weekFields(7),
      of: (first) =>
        weekFields(
          first && first.language !== undefined
            ? ['US', 'CA', 'JP', 'KR', 'TW', 'BR', 'IL'].includes(first.country)
              ? 7
              : 1
            : first && typeof first.getValue === 'function'
              ? first.getValue()
              : 1,
        ),
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
    // Jsoup's `Parser`: `Jsoup.parse(text, base, Parser.xmlParser())` reads XML (an RSS feed's
    // `<link>` keeps its text).
    Parser: {
      unescapeEntities: (text) => unescapeEntities(text),
      xmlParser: () => ({ __class: 'Parser', xml: true }),
      htmlParser: () => ({ __class: 'Parser', xml: false }),
    },
    DateTimeFormatter: {
      ofPattern: (pattern, locale) => K.dateFormat(pattern, locale),
      ISO_LOCAL_DATE: K.dateFormat('yyyy-MM-dd'),
      ISO_DATE: K.dateFormat('yyyy-MM-dd'),
      BASIC_ISO_DATE: K.dateFormat('yyyyMMdd'),
      ISO_LOCAL_TIME: K.dateFormat('HH:mm[:ss]'),
      ISO_LOCAL_DATE_TIME: isoFormat("yyyy-MM-dd'T'HH:mm:ss"),
      ISO_DATE_TIME: isoFormat("yyyy-MM-dd'T'HH:mm:ss"),
      ISO_OFFSET_DATE_TIME: isoFormat("yyyy-MM-dd'T'HH:mm:ss"),
      ISO_ZONED_DATE_TIME: isoFormat("yyyy-MM-dd'T'HH:mm:ss"),
      ISO_INSTANT: isoFormat("yyyy-MM-dd'T'HH:mm:ss'Z'"),
      RFC_1123_DATE_TIME: K.dateFormat('EEE, d MMM yyyy HH:mm:ss z'),
      // English's styles: Raygun shows dates as it formats them.
      ofLocalizedDate: (style) => K.dateFormat(LOCALIZED_DATES[KT.str(style)] ?? 'M/d/yy'),
      ofLocalizedDateTime: (style) =>
        K.dateFormat(`${LOCALIZED_DATES[KT.str(style)] ?? 'M/d/yy'}, HH:mm`),
    },
    // `System.getProperty("http.agent")`: the agent Android's own HTTP stack names.
    System: {
      currentTimeMillis: () => Date.now(),
      nanoTime: () => Date.now() * 1e6,
      getProperty: (name) =>
        KT.str(name) === 'http.agent' ? 'Dalvik/2.1.0 (Linux; U; Android 14)' : null,
      getenv: () => null,
      lineSeparator: () => '\n',
      // `System.arraycopy(src, from, dest, to, length)`, overlapping ranges too.
      arraycopy: (source, from, target, to, length) => {
        const part = Array.prototype.slice.call(source, from, from + length);
        for (let index = 0; index < length; index++) target[to + index] = part[index];
        return KT.unit;
      },
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
    BigInteger: {
      ZERO: 0n,
      ONE: 1n,
      TWO: 2n,
      TEN: 10n,
      valueOf: (value) => bigIntOf(value),
    },
    Integer: javaInteger,
    Long: javaLong,
    Int: { MAX_VALUE: 2147483647, MIN_VALUE: -2147483648, SIZE_BITS: 32, SIZE_BYTES: 4 },
    // OkHttp's `Dns.SYSTEM` (Raygun's client resolves names itself), Kotlin's buffer size,
    // Android's text input types (settings screens).
    Dns: { SYSTEM: { __class: 'Dns', lookup: (host) => [KT.str(host)] } },
    DEFAULT_BUFFER_SIZE: 8192,
    InputType: {
      TYPE_CLASS_TEXT: 1,
      TYPE_CLASS_NUMBER: 2,
      TYPE_CLASS_PHONE: 3,
      TYPE_TEXT_VARIATION_URI: 16,
      TYPE_TEXT_VARIATION_EMAIL_ADDRESS: 32,
      TYPE_TEXT_VARIATION_PASSWORD: 128,
      TYPE_TEXT_VARIATION_VISIBLE_PASSWORD: 144,
      TYPE_NUMBER_FLAG_DECIMAL: 8192,
      TYPE_TEXT_FLAG_NO_SUGGESTIONS: 524288,
    },
    URLEncoder: { encode: (text) => encodeURIComponent(KT.str(text)).replace(/%20/g, '+') },
    URLDecoder: { decode: (text) => decodeURIComponent(KT.str(text).replace(/\+/g, ' ')) },
    Charsets: { UTF_8: 'UTF-8' },
    // Android's `Base64` (flags), and Kotlin's (`Base64.Default`, `Base64.UrlSafe`): bytes.
    Base64: {
      decode: (value) => fromBase64(Array.isArray(value) ? binaryOf(value) : value),
      encode: (value, flags = 0) => utf8Encode(base64Encode(value, flags)),
      encodeToString: (value, flags = 0) => base64Encode(value, flags),
      DEFAULT: 0,
      NO_PADDING: 1,
      NO_WRAP: 2,
      CRLF: 4,
      URL_SAFE: 8,
      Default: {
        decode: (value) => fromBase64(Array.isArray(value) ? binaryOf(value) : value),
        encode: (value) => base64Encode(value, 2),
        encodeToByteArray: (value) => utf8Encode(base64Encode(value, 2)),
      },
      UrlSafe: {
        decode: (value) => fromBase64(Array.isArray(value) ? binaryOf(value) : value),
        encode: (value) => base64Encode(value, 2 | 8),
      },
      Mime: {
        decode: (value) => fromBase64(Array.isArray(value) ? binaryOf(value) : value),
        encode: (value) => base64Encode(value, 0),
      },
      // Java's `java.util.Base64.getDecoder().decode(…)`, `getEncoder().encodeToString(…)`.
      getDecoder: () => javaBase64Decoder,
      getUrlDecoder: () => javaBase64Decoder,
      getMimeDecoder: () => javaBase64Decoder,
      getEncoder: () => javaBase64Encoder(2),
      getUrlEncoder: () => javaBase64Encoder(2 | 8),
      getMimeEncoder: () => javaBase64Encoder(0),
    },
    // Java's `Cipher`, `MessageDigest`, `Mac` and `SecretKeyFactory`, over the Rust natives.
    Cipher: {
      getInstance: (name) => aesCipher(transformation(name)),
      ENCRYPT_MODE: 1,
      DECRYPT_MODE: 2,
    },
    MessageDigest: { getInstance: (algorithm) => digester(KT.str(algorithm)) },
    Mac: { getInstance: (algorithm) => digester(KT.str(algorithm), true) },
    SecretKeyFactory: {
      getInstance: (algorithm) => ({
        generateSecret: (spec) => {
          const encoded = fromBase64(
            crypto({
              op: 'pbkdf2',
              algorithm: KT.str(algorithm),
              password: base64Of(utf8Encode(spec.password)),
              salt: base64Of(spec.salt),
              iterations: spec.iterations,
              length: Math.ceil((spec.keyLength ?? 256) / 8),
            }),
          );
          return { encoded, getEncoded: () => encoded };
        },
      }),
    },
    JsonNull: null,
    Instant: {
      parse: (text) => {
        const ms = isoInstant(text);
        if (ms === null) throw new KotlinError(`couldn't read ${KT.str(text)} as an instant`);
        return instant(ms);
      },
      parseOrNull: (text) => {
        const ms = isoInstant(text);
        return ms === null ? null : instant(ms);
      },
      // Keiyoushi's `Instant.tryParse`: milliseconds, or 0.
      tryParse: (text) => isoInstant(text) ?? 0,
      fromEpochMilliseconds: (ms) => instant(ms),
      fromEpochSeconds: (seconds) => instant(Number(seconds) * 1000),
      ofEpochMilli: (ms) => instant(ms),
      ofEpochSecond: (seconds) => instant(seconds * 1000),
      now: () => instant(Date.now()),
      // kotlinx's far ends and java.time's (as far as a date reaches here).
      DISTANT_PAST: instant(-8.64e15),
      DISTANT_FUTURE: instant(8.64e15),
      MIN: instant(-8.64e15),
      MAX: instant(8.64e15),
      EPOCH: instant(0),
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

  // Durations (`1.seconds`) as numbers' properties, and `n.toDuration(DurationUnit.SECONDS)`.
  for (const [unit, ms] of Object.entries(durations)) NUMBER[unit] = (n) => duration(n * ms);
  NUMBER.toDuration = (n, [unit]) => duration(n * unitMsOf(unit));

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
  /// Mihon's filters' constructor parameters, in order.
  const FILTER_PARAMS = {
    Select: ['name', 'values', 'state'],
    Sort: ['name', 'values', 'state'],
    Group: ['name', 'state'],
    Text: ['name', 'state'],
    CheckBox: ['name', 'state'],
    TriState: ['name', 'state'],
    Header: ['name'],
    Separator: ['name'],
  };
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
  // A lookup by a Kotlin name never finds JavaScript's own (`"x".toString()` is the text, not
  // `Object.prototype.toString`'s "[object Object]").
  for (const table of [
    STRING,
    REGEX,
    LIST,
    MAP,
    OBJECT,
    NUMBER,
    ANY,
    NULLABLE,
    JSON_PROPS,
    NEXT_JS,
    SEQUENCE,
    GLOBALS,
    OBJECTS,
  ])
    Object.setPrototypeOf(table, null);
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

  /// A class's function with overloads (`[required, total, [types], code]` each): the first
  /// that takes as many arguments and, where several do, arguments of its parameters' types.
  KT.overloaded = (overloads) => {
    const pick = (args) => {
      const counted = overloads.filter(
        ([least, most]) => args.length >= least && args.length <= most,
      );
      const typed = counted.find(([, , types]) =>
        types.every(
          (type, at) =>
            at >= args.length ||
            type === null ||
            args[at] === null ||
            args[at] === undefined ||
            KT.is(args[at], type),
        ),
      );
      return (typed ?? counted[0] ?? overloads[0])[3];
    };
    const most = Math.max(...overloads.map(([, total]) => Math.min(total, 64)));
    const dispatch = async (self_, ...args) => pick(args)(self_, ...args);
    Object.defineProperty(dispatch, 'length', { value: most + 1 });
    return dispatch;
  };
  /// Collections a declared class may be built on, and what it then is.
  const COLLECTION_BASES = {
    ArrayList: 'list',
    MutableList: 'list',
    AbstractMutableList: 'list',
    LinkedList: 'list',
    HashSet: 'set',
    LinkedHashSet: 'set',
    MutableSet: 'set',
    HashMap: 'map',
    LinkedHashMap: 'map',
    MutableMap: 'map',
  };
  /// The class a declared class builds on when the source declares it too: by the name as
  /// written (`Outer.Inner`), else by its last part. Mihon's own filters (`Filter.Group`) stay
  /// Mihon's even when the source has a class of the same short name (a DTO's `Group`).
  const parentOf = (definition) => {
    const written = definition.supers[0]?.split('<')[0];
    if (!written) return null;
    const simple = written.split('.').pop();
    if (simple === definition.name && written === simple) return null;
    const parts = written.split('.');
    if (parts.length > 1) {
      const exact = CLASSES[written];
      if (exact) return exact;
      if (parts[parts.length - 2] === 'Filter') return null;
    }
    const found = CLASSES[simple] ?? null;
    return found && found.definition === definition ? null : found;
  };
  /// A declared class's own member `name` (a function or a getter), or the nearest parent's.
  const memberOf = (holder, name) => {
    for (let at = holder, depth = 0; at && at.definition && depth < 8; depth++) {
      const member = at.definition.members[name];
      if (member !== undefined) return member;
      at = parentOf(at.definition);
    }
    return undefined;
  };
  KT.defineClass = (definition) => {
    /// A field of an answer as this class reads it: by its own `@SerialName` (and `@JsonNames`)
    /// when it has one, else by its name; conversions without the class's names read them all.
    const jsonField = (json, name) => {
      const own = definition.serialNames;
      if (!own) return field(json, name);
      const serial = own[name];
      if (serial !== undefined && Object.prototype.hasOwnProperty.call(json, serial))
        return json[serial];
      if (serial === undefined && Object.prototype.hasOwnProperty.call(json, name))
        return json[name];
      for (const other of aliases[name] ?? [])
        if (Object.prototype.hasOwnProperty.call(json, other)) return json[other];
      return undefined;
    };
    /// An instance with these fields (`undefined` ones take their defaults).
    const build = async (fields, json = null) => {
      // A class built on a collection (`class CdnSet : LinkedHashSet<String>()`) is one.
      const base = COLLECTION_BASES[definition.supers[0]?.split('<')[0].split('.').pop()];
      const object = base === 'map' ? new Map() : base ? [] : {};
      if (base === 'set') object.isSet = true;
      object.__class = definition.name;
      object.__classes = [
        definition.name,
        ...definition.supers.map((name) => name.split('<')[0].split('.').pop()),
      ];
      object.__fields = definition.params;
      for (const name of definition.params) {
        let value = fields[name];
        // A default reads the parameters before it, as they're set.
        if (value === undefined && definition.defaults[name])
          value = await definition.defaults[name](
            ...definition.params.map((other) => (other in object ? object[other] : undefined)),
          );
        object[name] = value ?? null;
      }
      const parentClass = parentOf(definition);
      if (definition.superArgs) {
        let parent = await definition.superArgs(...definition.params.map((name) => object[name]));
        // Mihon's filters' parameters by name (`Filter.Select(name = …, values = …)`), in order.
        if (Array.isArray(parent) && parent.__named && !parentClass) {
          const order = FILTER_PARAMS[filterKindOf(definition.supers)] ?? ['name', 'state'];
          const named_ = parent.__named;
          const positional = parent;
          parent = order.map((key, at) => (at < positional.length ? positional[at] : named_[key]));
          while (parent.length && parent[parent.length - 1] === undefined) parent.pop();
        }
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
      const fieldsToSet = [];
      for (const [name, member] of Object.entries(definition.members)) {
        if (member && (member.isField || member.isInit)) fieldsToSet.push([name, member]);
        else if (member && member.isGetter)
          Object.defineProperty(object, name, {
            get: () => member.function_(object),
            enumerable: false,
            configurable: true,
          });
        else {
          object[name] = (...rest) => member(object, ...rest);
          object[name].arity = Math.max(0, member.length - 1);
        }
      }
      for (const [name, function_] of Object.entries(definition.statics ?? {}))
        if (!Object.prototype.hasOwnProperty.call(object, name))
          object[name] = (...rest) => function_(...rest);
      // A parent the source declares too (`class SortFilter : SelectFilter("Sort", sort)`): made
      // from the arguments passed to it, it lends what this class doesn't have itself.
      if (parentClass) {
        const inherited = await parentClass.make(
          Array.isArray(object.__super) ? object.__super : [],
          (Array.isArray(object.__super) && object.__super.__named) || null,
          json,
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
            // The parent's own functions and getters run on this object, so they reach what
            // it overrides (an abstract `getCanvasDimensions()` a subclass has).
            const member = memberOf(parentClass, name);
            if (typeof member === 'function') {
              object[name] = (...rest) => member(object, ...rest);
              object[name].arity = Math.max(0, member.length - 1);
            } else if (member && member.isGetter) {
              Object.defineProperty(object, name, {
                get: () => member.function_(object),
                enumerable: false,
                configurable: true,
              });
            } else {
              Object.defineProperty(object, name, descriptor);
            }
          }
        }
        for (const name of inherited.__classes)
          if (!object.__classes.includes(name)) object.__classes.push(name);
      }
      // Fields in the order they're declared, once the instance has its functions and its
      // parent's (whose constructor runs first, as in Kotlin), and its `init` blocks among them.
      for (const [name, member] of fieldsToSet) {
        if (member.isInit) {
          await member.function_(object);
          continue;
        }
        // Decoded from JSON: a body field the answer has takes its value, as kotlinx does.
        const given = json ? jsonField(json, name) : undefined;
        Object.defineProperty(object, name, {
          value:
            given !== undefined
              ? await decodeAs(given, parseType(definition.types?.[name] ?? 'Any'))
              : await member.function_(object),
          writable: true,
          enumerable: false,
          configurable: true,
        });
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
    /// Made from constructor arguments, in order or by name (and, for a parent of a class
    /// decoded from JSON, the JSON its fields read).
    const make = async (args = [], named_ = null, json = null) => {
      const fields = {};
      definition.params.forEach((name, index) => {
        fields[name] =
          named_ && Object.prototype.hasOwnProperty.call(named_, name) ? named_[name] : args[index];
      });
      return build(fields, json);
    };
    /// Decoded from JSON: each field by its JSON name, as its type says.
    const fromJson = async (json, typeArgs = []) => {
      const fields = {};
      for (const name of definition.params) {
        const value = jsonField(json, name);
        const type = substitute(
          parseType(definition.types?.[name] ?? 'Any'),
          definition.typeParams ?? [],
          typeArgs,
        );
        fields[name] = value === undefined ? undefined : await decodeAs(value, type);
      }
      return build(fields, json);
    };
    const holder = {
      isClass: true,
      definition,
      name: definition.name,
      make,
      fromJson,
      required: definition.required ?? [],
      serialName: definition.serialName ?? definition.name,
      supers: definition.supers.map((name) => name.split('<')[0].split('.').pop()),
    };
    // Its companion's functions, on the class and, as Kotlin lets its code call them, its
    // instances.
    for (const [name, function_] of Object.entries(definition.statics ?? {}))
      holder[name] = (...rest) => function_(...rest);
    // A nested class (`ChapterList.Manga`) takes its short name only while no top-level class
    // has it: code naming `Manga` by itself means the top-level one.
    const isNested = (holder_) =>
      Boolean(holder_?.definition?.qualified) &&
      holder_.definition.qualified !== holder_.definition.name;
    const taken = CLASSES[definition.name];
    if (!isNested(holder) || !taken || isNested(taken)) CLASSES[definition.name] = holder;
    if (definition.qualified) CLASSES[definition.qualified] = holder;
    // An enum's entries: their fields from their arguments, and the class's functions.
    definition.entries.forEach((entry, ordinal) => {
      const value = {
        __class: definition.name,
        __classes: [definition.name, 'Enum'],
        name: entry,
        ordinal,
        toString: () => entry,
      };
      const args = definition.entryArgs?.[entry] ?? [];
      definition.params.forEach((param, index) => {
        if (param === 'name') return;
        if (index < args.length && args[index] !== undefined) value[param] = args[index];
        // A parameter left out takes its default (read when the code reads it).
        else if (definition.defaults[param])
          value[param] = Promise.resolve(
            definition.defaults[param](...definition.params.map((other) => value[other])),
          ).catch(() => null);
      });
      for (const [name, member] of Object.entries(definition.members)) {
        if (typeof member === 'function') value[name] = (...rest) => member(value, ...rest);
        else if (member && member.isGetter)
          Object.defineProperty(value, name, {
            get: () => member.function_(value),
            enumerable: false,
            configurable: true,
          });
      }
      holder[entry] = value;
    });
    holder.values = () => definition.entries.map((entry) => holder[entry]);
    holder.entries = definition.entries.map((entry) => holder[entry]);
    holder.valueOf = (text) =>
      Object.prototype.hasOwnProperty.call(holder, KT.str(text)) &&
      definition.entries.includes(KT.str(text))
        ? holder[KT.str(text)]
        : KT.raise(`No enum constant ${definition.name}.${KT.str(text)}`);
    if (definition.kind === 'object') {
      for (const [name, member] of Object.entries(definition.members)) {
        // A property (`val wordRegex by lazy { … }`) is worked out once, when first read; a
        // field may be set.
        if (member && (member.isGetter || member.isField)) {
          let value;
          let read = false;
          Object.defineProperty(holder, name, {
            get: () => {
              if (!read) {
                value = member.function_(holder);
                read = true;
              }
              return value;
            },
            set: (next) => {
              value = next;
              read = true;
            },
            enumerable: false,
          });
        } else holder[name] = (...rest) => member(holder, ...rest);
      }
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
    if (receiver instanceof RegExp) return Boolean(REGEX[name]);
    // Decoders `KT.m` answers for answers, bodies and bytes (`fun Response.read() = parseAsProto<T>()`).
    if (
      (PROTO_DECODERS.has(name) || DECODERS.has(name)) &&
      (receiver instanceof Response || receiver.isResponseBody || Array.isArray(receiver))
    )
      return true;
    if (hasNextJs(receiver, name)) return true;
    const kind = kindOf(receiver);
    if (kind === 'list' || kind === 'elements')
      return Boolean(LIST[name]) || (kind === 'elements' && typeof receiver[name] === 'function');
    if (kind === 'string') return Boolean(STRING[name]);
    if (kind === 'map') return Boolean(MAP[name]);
    if (kind === 'number' && NUMBER[name]) return true;
    if (typeof receiver[name] === 'function') return true;
    if (kind === 'object' && receiver[name] && receiver[name].isMethod) return true;
    if (kind === 'object' && receiver.__class === undefined && (OBJECT[name] || objectAsMap(name)))
      return true;
    return Boolean(receiver.isStringBuilder && (name in receiver || STRING[name]));
  };
  const hasField = (receiver, name) => {
    if (receiver === null || receiver === undefined) return false;
    // A list's, a string's or a map's own properties (`size` in `buildList { … }`).
    const kind = kindOf(receiver);
    if (kind === 'list' || kind === 'elements')
      return ['size', 'lastIndex', 'indices'].includes(name);
    if (kind === 'string') return ['length', 'lastIndex', 'indices', 'category'].includes(name);
    if (kind === 'map') return ['size', 'keys', 'values', 'entries'].includes(name);
    return (
      typeof receiver === 'object' &&
      (Object.prototype.hasOwnProperty.call(receiver, name) ||
        Boolean(receiver.isGetterHost && receiver.getters && receiver.getters[name]) ||
        classGetter(receiver, name))
    );
  };

  KT.rget = async (receivers, name) => {
    const last = receivers[receivers.length - 1];
    const sourceHas = Boolean(last && last.isSource && last.has(name));
    for (const receiver of receivers) {
      if (receiver === null || receiver === undefined) continue;
      if (receiver.isSource) {
        if (receiver.has(name)) return receiver.get(name);
        continue;
      }
      if (hasField(receiver, name)) return KT.p(receiver, name);
      // One of the source's extension properties on this receiver (`val Element.isValid`);
      // extension functions are registered with their receivers' types and are never read as
      // properties (`slug` in `apply` beside `fun SManga.slug()` is a field).
      if (
        last &&
        last.isSource &&
        last.hasExtension(name) &&
        (last.extensionProperty(name, receiver) || !last.extensionTyped(name))
      )
        return last.ext(name, receiver, []);
      // What a JSON value has (`contentOrNull` in `fun JsonElement.text() = when (this) …`).
      const kind = kindOf(receiver);
      if (
        JSON_PROPS[name] &&
        !sourceHas &&
        (['string', 'number', 'boolean', 'list'].includes(kind) ||
          (kind === 'object' && receiver.__class === undefined && !receiver.isJsonBuilder))
      )
        return JSON_PROPS[name](receiver);
    }
    const source = last;
    if (source.constants && source.constants.has(name)) return source.constants.get(name);
    if (source.classes && source.classes[name]) return source.classes[name];
    // An object of Mihon's or Kotlin's (`SManga`, `Locale`).
    return KT.global(name);
  };
  KT.rset = async (receivers, name, value) => {
    for (const receiver of receivers) {
      if (receiver === null || receiver === undefined) continue;
      if (receiver.isSource) {
        if (receiver.has(name)) return receiver.set(name, value);
        continue;
      }
      if (hasField(receiver, name)) return KT.setp(receiver, name, value);
    }
    throw new KotlinError(`Raygun's runtime doesn't know ${name}`);
  };
  KT.rcall = async (receivers, name, args, named_, lambda, type) => {
    // One of the source's extensions goes to the receiver of its type (`infoRow(…)` on a
    // `Document` inside `SManga.create().apply { … }`), when one is.
    const last = receivers[receivers.length - 1];
    const anyFits =
      last &&
      last.isSource &&
      last.hasExtension(name) &&
      receivers.some(
        (receiver) =>
          receiver !== null &&
          receiver !== undefined &&
          !receiver.isSource &&
          last.extensionFits(name, receiver),
      );
    for (const receiver of receivers) {
      if (receiver === null || receiver === undefined) continue;
      if (receiver.isSource) {
        if (receiver.hasFunction(name)) return receiver.call(name, withLambda(args, lambda));
        if (receiver.hasExtension(name)) continue;
        continue;
      }
      if (hasMember(receiver, name) || (receiver.isJsonBuilder && name in receiver))
        return KT.m(receiver, name, args, named_, lambda, type);
      const source = receivers[receivers.length - 1];
      if (
        source.isSource &&
        source.hasExtension(name) &&
        (!anyFits || source.extensionFits(name, receiver))
      )
        return source.ext(name, receiver, withLambda(args, lambda));
      // What any value has (`toJsonElement()` in `fun Foo.toJson() = toJsonElement()`).
      if (receiver !== null && receiver !== undefined && ANY[name])
        return KT.m(receiver, name, args, named_, lambda, type);
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
    const extensionOverloads = new Map();
    const inits = [];
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
      imageFetch: async () => Fetch.IMAGE,
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
      // KeiSource's deep links: none unless the source has them.
      getMangaByUrl: async () => KT.raise('getMangaByUrl not implemented'),
      getMangasByUrl: async (url, page) => {
        const found = await self.call('getMangaByUrl', [url]);
        return KT.call('MangasPage', [found === null || found === undefined ? [] : [found], false]);
      },
      // KeiSource's `getSearchManga`: a search (an address typed in isn't looked up).
      getSearchManga: async (page, query, filters) =>
        self.call('getSearchMangaList', [page, query, filters]),
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
      getAll: () => new Map(preferences.store),
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
      if (function_) return enter(function_, args);
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
      /// The class's `init { … }`, run before the first call.
      init: (function_) => inits.push(function_),
      // Extensions of one name on different receivers are told apart by the receiver's type
      // (the latest of those that fit, else the latest).
      /// An extension function; overloads are told apart by their receiver's type, then by
      /// `shape` (`[required, total, [types]]`): how many arguments they take, and of what types.
      extension: (name, function_, type = null, shape = null, property = false) => {
        const list = extensionOverloads.get(name) ?? [];
        list.push({ type, function_, shape, property });
        extensionOverloads.set(name, list);
        const fits = (entry, args) =>
          !entry.shape || (args.length >= entry.shape[0] && args.length <= entry.shape[1]);
        const typed = (entry, args) =>
          !entry.shape ||
          entry.shape[2].every(
            (kind, at) =>
              at >= args.length || kind === null || args[at] == null || KT.is(args[at], kind),
          );
        extensions.set(
          name,
          list.length === 1
            ? function_
            : (receiver, ...args) => {
                const ordered = [...list].reverse();
                const byReceiver = ordered.filter(
                  (entry) => entry.type && KT.is(receiver, entry.type),
                );
                const pool = byReceiver.length ? byReceiver : ordered;
                const fitting =
                  pool.find((entry) => fits(entry, args) && typed(entry, args)) ??
                  pool.find((entry) => fits(entry, args)) ??
                  pool[0];
                return fitting.function_(receiver, ...args);
              },
        );
      },
      has: (name) =>
        findProperty(name, top) >= 0 || Object.prototype.hasOwnProperty.call(base, name),
      hasFunction: (name) =>
        layers.some((layer) => layer.functions.has(name)) ||
        Object.prototype.hasOwnProperty.call(baseFunctions, name),
      hasExtension: (name) => extensions.has(name),
      /// Whether one of the extension's overloads takes `receiver` by its declared type.
      extensionFits: (name, receiver) =>
        (extensionOverloads.get(name) ?? []).some(
          (entry) => entry.type && KT.is(receiver, entry.type),
        ),
      /// Whether the extension's overloads say what they take (older conversions' extension
      /// properties don't).
      extensionTyped: (name) => (extensionOverloads.get(name) ?? []).some((entry) => entry.type),
      /// Whether `name` is an extension property read on `receiver` (by its declared type).
      extensionProperty: (name, receiver) =>
        (extensionOverloads.get(name) ?? []).some(
          (entry) => entry.property && (!entry.type || KT.is(receiver, entry.type)),
        ),
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
        // A member wins over an extension of the same name, as in Kotlin, when it takes the
        // arguments (`dto.toSManga(baseUrl)` beside the source's `Other.toSManga(manga)`).
        const own = receiver && receiver.__class !== undefined ? receiver[name] : undefined;
        if (typeof own === 'function' && (own.arity === undefined || args.length <= own.arity))
          return own(...args);
        // An extension property beside a field of the same name: the field, as in Kotlin.
        if (
          !args.length &&
          receiver &&
          receiver.__class !== undefined &&
          own !== undefined &&
          typeof own !== 'function' &&
          Object.prototype.hasOwnProperty.call(receiver, name)
        )
          return own;
        // A runtime value's own function wins when the extension doesn't take these arguments
        // (OkHttp's `addQueryParameter(name, value)` beside an extension taking a set of values).
        if (
          receiver !== null &&
          receiver !== undefined &&
          receiver.__class === undefined &&
          hasMember(receiver, name) &&
          !(extensionOverloads.get(name) ?? []).some((entry) =>
            extensionTakes(entry, receiver, args),
          )
        )
          return KT.m(receiver, name, args);
        return enter(function_, args, receiver);
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
    // The classes' `init` blocks, base-most first, once the client is set up.
    if (inits.length) {
      const before = self.ready;
      self.ready = (async () => {
        if (before) await before;
        for (const run of inits) await run();
      })();
      self.ready.catch(() => {});
    }
    return self;
  };

  // Mihon's `memo`s: what a source keeps with a title or a chapter between calls (Mihon stores
  // them with its library). A chapter's travels in its id, after the separator, as its address
  // alone may not tell it from another title's (`chapter-1` beside `{ mangaPath }`); a title's is
  // kept by its address, in memory and in the extension's storage, the newest kept first.
  const MEMO_SEPARATOR = '\u001f';
  const MEMO_BUDGET = 400 * 1024;
  const hasMemo = (memo) =>
    memo !== null &&
    typeof memo === 'object' &&
    !Array.isArray(memo) &&
    Object.keys(memo).length > 0;
  const chapterIdOf = (item) =>
    hasMemo(item.memo)
      ? `${item.url}${MEMO_SEPARATOR}${JSON.stringify(plain(item.memo))}`
      : item.url;
  const chapterOf = (id) => {
    const text = K.str(id);
    const at = text.indexOf(MEMO_SEPARATOR);
    if (at < 0) return chapter({ url: text });
    let memo = {};
    try {
      memo = JSON.parse(text.slice(at + 1));
    } catch {
      // An id cut short keeps its address.
    }
    return chapter({ url: text.slice(0, at), memo });
  };
  /// What's kept of a source's titles by address: the memo, and the name it was listed under
  /// (Mihon keeps a title's name when its details don't give one).
  const titleMemos = (sourceId) => {
    const key = `mihon.titles.${sourceId}`;
    let kept = null;
    let dirty = false;
    const load = () => {
      if (kept) return kept;
      kept = new Map();
      try {
        const stored = raygun.storage.get(key);
        if (stored && typeof stored === 'object')
          for (const [url, entry] of Object.entries(stored)) kept.set(url, entry);
      } catch {
        // Without storage they're kept for this run only.
      }
      return kept;
    };
    const keep = (url, change) => {
      const all = load();
      const known = all.get(url) ?? {};
      const entry = { ...known, ...change };
      if (JSON.stringify(entry) === JSON.stringify(known)) return;
      all.delete(url);
      all.set(url, entry);
      dirty = true;
    };
    return {
      get: (url) => load().get(K.str(url))?.memo ?? {},
      title: (url) => load().get(K.str(url))?.title ?? null,
      keep: (url, memo, title) => {
        const change = {};
        if (hasMemo(memo)) change.memo = plain(memo);
        if (title) change.title = K.str(title).trim();
        if (Object.keys(change).length) keep(K.str(url), change);
      },
      save: () => {
        if (!dirty || !kept) return;
        dirty = false;
        const entries = [...kept.entries()];
        let size = JSON.stringify(Object.fromEntries(entries)).length;
        while (size > MEMO_BUDGET && entries.length) {
          const [url, entry] = entries.shift();
          kept.delete(url);
          size -= url.length + JSON.stringify(entry).length + 6;
        }
        try {
          raygun.storage.set(key, Object.fromEntries(entries));
        } catch {
          // Kept for this run.
        }
      },
    };
  };

  const listing = (page, memos) => {
    for (const item of page.mangas ?? [])
      if (item && item.url) memos.keep(item.url, item.memo, item.title);
    memos.save();
    return {
      items: (page.mangas ?? [])
        .map((item) => ({
          id: item.url,
          title: K.str(item.title).trim(),
          coverUrl: item.thumbnail_url || undefined,
          coverHeaders: undefined,
        }))
        .filter((item) => item.id && item.title),
      hasNextPage: Boolean(page.hasNextPage),
    };
  };

  mihon.source = (config) => {
    renames = { ...renames, ...(config.renames ?? {}) };
    aliases = { ...aliases, ...(config.aliases ?? {}) };
    messages = { ...messages, ...(config.messages ?? {}) };
    assets = { ...assets, ...(config.assets ?? {}) };
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
      const memos = titleMemos(source.id);
      // What each chapter was listed with (its name, number, scanlator), by id: Mihon hands
      // `getPageList` the whole chapter, and some sources read its name there.
      const chapterFacts = new Map();
      const keepFacts = (id, item) => {
        chapterFacts.delete(id);
        chapterFacts.set(id, {
          name: item.name,
          chapter_number: item.chapter_number,
          scanlator: item.scanlator,
          date_upload: item.date_upload,
        });
        if (chapterFacts.size > 4000) chapterFacts.delete(chapterFacts.keys().next().value);
      };
      const titleOf = (id) => manga({ url: id, memo: memos.get(id) });
      const update = async (id, details) => {
        await ready();
        const answer = await self.call('fetchMangaUpdate', [titleOf(id), [], details, !details]);
        if (answer && answer.manga) {
          memos.keep(id, answer.manga.memo, answer.manga.title);
          memos.save();
        }
        return answer;
      };
      raygun.providers.register({
        kind: 'manga.source',
        id: source.id,
        name: source.name,
        popular: async ({ page = 1 }) => {
          await ready();
          return covers(listing(await self.call('getPopularManga', [page]), memos));
        },
        latest: async ({ page = 1 }) => {
          await ready();
          if (!KT.truthy(await self.get('supportsLatest')))
            throw new Error('provider has no method latest');
          return covers(listing(await self.call('getLatestUpdates', [page]), memos));
        },
        // The source's own filters (`getFilterList()`); none when they couldn't be converted.
        filters: async () => {
          await ready();
          return describeFilters(await filterList());
        },
        search: async ({ query = '', page = 1, filters = {} }) => {
          await ready();
          const list = applyFilters(await filterList(), filters ?? {});
          return covers(listing(await self.call('getSearchMangaList', [page, query, list]), memos));
        },
        details: async ({ id }) => {
          const answer = await update(id, true);
          const item = answer.manga;
          // Details without a name keep the one the title was listed under, as Mihon does.
          const title = K.str(item?.title ?? '').trim() || memos.title(id) || '';
          return toDetails(t, id, {
            ...item,
            title,
            webUrl: await self.call('getMangaUrl', [titleOf(id)]).catch(() => undefined),
          });
        },
        chapters: async ({ id }) => {
          const answer = await update(id, false);
          return {
            chapters: (answer.chapters ?? []).map((item) => ({
              id: (keepFacts(chapterIdOf(item), item), chapterIdOf(item)),
              name: K.str(item.name).trim() || undefined,
              number: item.chapter_number >= 0 ? item.chapter_number : undefined,
              dateUpload: item.date_upload > 0 ? item.date_upload : undefined,
              scanlator: item.scanlator || undefined,
            })),
          };
        },
        pages: async ({ chapterId }) => {
          await ready();
          const asked = chapterOf(chapterId);
          const facts = chapterFacts.get(chapterId);
          if (facts) Object.assign(asked, facts);
          const pages = await self.call('getPageList', [asked]);
          // Pages that are sub pages (`Page(i, url)`): their pictures' addresses, as Mihon's
          // reader asks `getImageUrl` for them, a few at a time.
          const lazy = pages.filter((page) => !page.imageUrl && page.url);
          if (lazy.length && self.hasFunction('getImageUrl')) {
            for (let at = 0; at < lazy.length; at += 4) {
              await Promise.all(
                lazy.slice(at, at + 4).map(async (page) => {
                  try {
                    page.imageUrl = KT.str(await self.call('getImageUrl', [page]));
                  } catch (error) {
                    if (lazy.length === 1) throw error;
                  }
                }),
              );
            }
          }
          // A source's `XorInterceptor`s: the key each picture's request gives, for Raygun's
          // image fetcher (`#raygun-unscramble=xor:<hex>`).
          const client = await self.get('client').catch(() => null);
          const xors = (client?.interceptors ?? []).filter((interceptor) => interceptor?.isXor);
          const xorKey = async (request) => {
            for (const interceptor of xors) {
              const key = await interceptor.keyFor(request);
              if (key && key.length) return key;
            }
            return null;
          };
          const out = [];
          for (const page of pages) {
            if (!page.imageUrl) continue;
            const request = await self.call('getImageRequest', [page]);
            let url = unscrambled(request.url.toString(), config.imageRecipe);
            const key = xors.length ? await xorKey(request) : null;
            if (key) url = `${url.split('#')[0]}#raygun-unscramble=xor:${hexOf(key)}`;
            // A source whose client draws or decodes its pictures fetches them itself (`image`
            // below); Raygun puts the provider's id after the mark.
            else if (config.imagesFromSource) url = `${url}#raygun-source`;
            out.push({ url, headers: request.headers.object() });
          }
          return { pages: out };
        },
        // A page's picture through the source's client and its interceptors, as Mihon's
        // `getImage` fetches it: the bytes they answer with, in base64.
        image: async ({ url, headers = {} }) => {
          await ready();
          const client = await self.get('client');
          const request = new Request({
            url,
            headers: new Headers(Object.entries(headers ?? {})),
            kind: { isFetch: true, kind: 'image' },
          });
          const response = await client.newCall(request).awaitSuccess();
          const bytes = await bodyU8(response.body);
          return { base64: K.bin.toBase64(bytes) };
        },
      });
    }
  };

  KT.statusName = (status) => STATUS_NAMES[status] ?? 'unknown';
  globalThis.KT = KT;
})();
