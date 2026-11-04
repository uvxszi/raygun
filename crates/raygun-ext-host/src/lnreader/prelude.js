// LNReader's plugin runtime over Raygun's host API. LNReader (LNReader/lnreader,
// src/plugins/pluginManager.ts, MIT) evaluates each compiled plugin (LNReader/lnreader-plugins,
// MIT) with a `require` handing out cheerio, htmlparser2, dayjs, urlencode and its own `@libs`
// (fetch, storage, filters, novel statuses). This prelude builds each of them over Raygun's
// natives (the documents stay in Rust: `dom.rs`), then presents the plugin as a Raygun novel
// source. Evaluated after Raygun's prelude, before the plugin.
'use strict';
(() => {
  const native = globalThis.__rgLnr;
  delete globalThis.__rgLnr;
  const host = globalThis.raygun;

  const unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== undefined) throw new Error(envelope.error);
    return envelope.value;
  };
  const dom = (op, args) => unwrap(native.dom(op, JSON.stringify(args)));
  const str = (value) => (value === undefined || value === null ? '' : String(value));
  const define = (name, value) =>
    Object.defineProperty(globalThis, name, {
      value,
      writable: true,
      configurable: true,
      enumerable: false,
    });
  define('window', globalThis);
  define('global', globalThis);
  define('self', globalThis);

  // ---- Timers ----

  let timerId = 0;
  const cancelled = new Set();
  define('setTimeout', (callback, ms, ...args) => {
    const id = ++timerId;
    void host.time.sleep(Number(ms) || 0).then(() => {
      if (cancelled.delete(id)) return;
      if (typeof callback === 'function') callback(...args);
    });
    return id;
  });
  define('clearTimeout', (id) => {
    cancelled.add(id);
  });

  // ---- Base64 ----

  const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  define('btoa', (input) => {
    const text = str(input);
    let out = '';
    for (let i = 0; i < text.length; i += 3) {
      const [a, b, c] = [text.charCodeAt(i), text.charCodeAt(i + 1), text.charCodeAt(i + 2)];
      if (a > 255 || b > 255 || c > 255)
        throw new Error('btoa: the text has non-Latin-1 characters');
      const bits = (a << 16) | ((b || 0) << 8) | (c || 0);
      out += BASE64[(bits >> 18) & 63] + BASE64[(bits >> 12) & 63];
      out += i + 1 < text.length ? BASE64[(bits >> 6) & 63] : '=';
      out += i + 2 < text.length ? BASE64[bits & 63] : '=';
    }
    return out;
  });
  define('atob', (input) => {
    const text = str(input).replace(/[\s=]/g, '');
    let out = '';
    let bits = 0;
    let count = 0;
    for (const char of text) {
      const value = BASE64.indexOf(char === '-' ? '+' : char === '_' ? '/' : char);
      if (value < 0) throw new Error('atob: the text is not base64');
      bits = (bits << 6) | value;
      count += 6;
      if (count >= 8) {
        count -= 8;
        out += String.fromCharCode((bits >> count) & 255);
      }
    }
    return out;
  });

  // ---- URL and URLSearchParams ----

  const formDecode = (text) => {
    try {
      return decodeURIComponent(text.replace(/\+/g, ' '));
    } catch {
      return text;
    }
  };
  const formEncode = (text) => encodeURIComponent(text).replace(/%20/g, '+');

  class URLSearchParams {
    constructor(init) {
      this._list = [];
      if (typeof init === 'string') {
        for (const part of init.replace(/^\?/, '').split('&')) {
          if (!part) continue;
          const at = part.indexOf('=');
          this._list.push(
            at < 0
              ? [formDecode(part), '']
              : [formDecode(part.slice(0, at)), formDecode(part.slice(at + 1))],
          );
        }
      } else if (init && typeof init[Symbol.iterator] === 'function') {
        for (const [name, value] of init) this._list.push([str(name), str(value)]);
      } else if (init && typeof init === 'object') {
        for (const [name, value] of Object.entries(init)) this._list.push([name, str(value)]);
      }
    }
    append(name, value) {
      this._list.push([str(name), str(value)]);
    }
    set(name, value) {
      const at = this._list.findIndex(([key]) => key === str(name));
      this.delete(name);
      this._list.splice(at < 0 ? this._list.length : at, 0, [str(name), str(value)]);
    }
    get(name) {
      return this._list.find(([key]) => key === str(name))?.[1] ?? null;
    }
    getAll(name) {
      return this._list.filter(([key]) => key === str(name)).map(([, value]) => value);
    }
    has(name) {
      return this._list.some(([key]) => key === str(name));
    }
    delete(name) {
      this._list = this._list.filter(([key]) => key !== str(name));
    }
    forEach(callback) {
      for (const [name, value] of this._list) callback(value, name, this);
    }
    keys() {
      return this._list.map(([name]) => name)[Symbol.iterator]();
    }
    values() {
      return this._list.map(([, value]) => value)[Symbol.iterator]();
    }
    entries() {
      return this._list.map((pair) => [...pair])[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
    get size() {
      return this._list.length;
    }
    toString() {
      return this._list
        .map(([name, value]) => `${formEncode(name)}=${formEncode(value)}`)
        .join('&');
    }
  }

  class URL {
    constructor(href, base) {
      const parts = unwrap(native.url(str(href), base === undefined ? '' : str(base)));
      this.protocol = parts.protocol;
      this.username = parts.username;
      this.password = parts.password;
      this.hostname = parts.hostname;
      this.port = parts.port;
      this.pathname = parts.pathname;
      this.hash = parts.hash;
      this.origin = parts.origin;
      this.searchParams = new URLSearchParams(parts.search);
    }
    get host() {
      return this.port ? `${this.hostname}:${this.port}` : this.hostname;
    }
    get search() {
      const query = this.searchParams.toString();
      return query ? `?${query}` : '';
    }
    set search(value) {
      this.searchParams = new URLSearchParams(str(value));
    }
    get href() {
      const user = this.username
        ? `${this.username}${this.password ? `:${this.password}` : ''}@`
        : '';
      return `${this.protocol}//${user}${this.host}${this.pathname}${this.search}${this.hash}`;
    }
    toString() {
      return this.href;
    }
    toJSON() {
      return this.href;
    }
  }
  define('URLSearchParams', URLSearchParams);
  define('URL', URL);

  // ---- Headers, FormData, fetch (LNReader's @libs/fetch) ----

  class Headers {
    constructor(init) {
      this._map = new Map();
      if (init instanceof Headers) {
        for (const [name, value] of init.entries()) this.set(name, value);
      } else if (init && typeof init === 'object') {
        for (const [name, value] of Object.entries(init)) {
          if (value !== undefined && value !== null) this.set(name, value);
        }
      }
    }
    get(name) {
      return this._map.get(str(name).toLowerCase()) ?? null;
    }
    set(name, value) {
      this._map.set(str(name).toLowerCase(), str(value));
    }
    append(name, value) {
      const key = str(name).toLowerCase();
      this._map.set(key, this._map.has(key) ? `${this._map.get(key)}, ${str(value)}` : str(value));
    }
    has(name) {
      return this._map.has(str(name).toLowerCase());
    }
    delete(name) {
      this._map.delete(str(name).toLowerCase());
    }
    forEach(callback) {
      for (const [name, value] of this._map) callback(value, name, this);
    }
    entries() {
      return this._map.entries();
    }
    keys() {
      return this._map.keys();
    }
    values() {
      return this._map.values();
    }
    [Symbol.iterator]() {
      return this._map.entries();
    }
  }

  class FormData {
    constructor() {
      this._list = [];
    }
    append(name, value) {
      this._list.push([str(name), str(value)]);
    }
    set(name, value) {
      this.delete(name);
      this.append(name, value);
    }
    get(name) {
      return this._list.find(([key]) => key === str(name))?.[1] ?? null;
    }
    getAll(name) {
      return this._list.filter(([key]) => key === str(name)).map(([, value]) => value);
    }
    has(name) {
      return this._list.some(([key]) => key === str(name));
    }
    delete(name) {
      this._list = this._list.filter(([key]) => key !== str(name));
    }
    entries() {
      return this._list.map((pair) => [...pair])[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
  }
  define('Headers', Headers);
  define('FormData', FormData);

  /** A request body as text; forms go as `application/x-www-form-urlencoded`, which the sites
   * plugins post to (WordPress's `admin-ajax.php` among them) take as well as multipart. */
  const bodyOf = (body, headers) => {
    if (body === undefined || body === null) return null;
    if (typeof body === 'string') return body;
    if (body instanceof URLSearchParams || body instanceof FormData) {
      if (!headers.has('content-type')) {
        headers.set('content-type', 'application/x-www-form-urlencoded;charset=UTF-8');
      }
      return new URLSearchParams(body instanceof FormData ? body._list : body._list).toString();
    }
    return str(body);
  };

  // Answers that failed lately. Plugins often read a refused page (a browser check) as if it were
  // the site and find nothing in it; a call that comes back empty then fails with what its
  // requests met instead, so the app says why.
  const failures = [];
  const failedSince = (start) => failures.find((failure) => failure.at >= start);
  const failIfEmpty = (start, empty) => {
    const failure = failedSince(start);
    if (empty && failure) {
      const where = (() => {
        try {
          return new URL(failure.url).hostname;
        } catch {
          return failure.url;
        }
      })();
      throw new Error(`${where} answered HTTP ${failure.status}`);
    }
  };

  const fetchApi = async (url, init = {}) => {
    const headers = new Headers(init.headers);
    const body = bodyOf(init.body, headers);
    const response = await host.http.fetch(str(url), {
      method: str(init.method || 'GET').toUpperCase(),
      headers: Object.fromEntries(headers.entries()),
      body,
    });
    if (!response.ok) {
      failures.push({ at: Date.now(), status: response.status, url: response.url });
      if (failures.length > 20) failures.shift();
    }
    return {
      ok: response.ok,
      status: response.status,
      statusText: '',
      url: response.url,
      redirected: response.url !== str(url),
      headers: new Headers(response.headers),
      text: () => response.text(),
      json: () => response.json(),
      arrayBuffer: () => {
        throw new Error("Raygun's LNReader runtime reads answers as text only");
      },
    };
  };
  define('fetch', fetchApi);

  const fetchText = async (url, init, encoding) => {
    try {
      const response = await fetchApi(url, init ?? {});
      if (!response.ok) return '';
      // The body comes as UTF-8; pages in another charset read with replacement characters.
      void encoding;
      return await response.text();
    } catch {
      return '';
    }
  };

  // ---- The documents (cheerio and htmlparser2) ----

  const HANDLE = Symbol('lnreader.handle');
  /** Node objects by handle, so `el === el` holds across selections. */
  const known = new Map();
  const nodeOf = (handle) => {
    let node = known.get(handle);
    if (node === undefined) {
      if (known.size > 20000) known.clear();
      node = new DomNode(handle);
      known.set(handle, node);
    }
    return node;
  };
  const forget = (handles) => {
    for (const handle of handles) {
      const node = known.get(handle);
      if (node) node._info = null;
    }
  };

  /** A node as domhandler (cheerio's tree) presents it: `type`, `name`, `attribs`, `data`. */
  class DomNode {
    constructor(handle) {
      Object.defineProperty(this, HANDLE, { value: handle });
      this._info = null;
    }
    get _i() {
      if (this._info === null) this._info = dom('info', { nodes: [this[HANDLE]] })[0];
      return this._info;
    }
    get type() {
      return this._i.type;
    }
    get name() {
      return this._i.name;
    }
    get tagName() {
      return this._i.name;
    }
    get nodeType() {
      const type = this.type;
      return type === 'text' ? 3 : type === 'comment' ? 8 : type === 'root' ? 9 : 1;
    }
    get attribs() {
      return this._i.attribs ?? {};
    }
    get data() {
      return this._i.data;
    }
    get nodeValue() {
      return this._i.data ?? null;
    }
    get children() {
      return dom('children', { nodes: [this[HANDLE]], all: true }).map(nodeOf);
    }
    get childNodes() {
      return this.children;
    }
    get firstChild() {
      return this.children[0] ?? null;
    }
    get lastChild() {
      const children = this.children;
      return children[children.length - 1] ?? null;
    }
    get parent() {
      const [parent] = dom('parent', { nodes: [this[HANDLE]] });
      return parent === undefined ? null : nodeOf(parent);
    }
    get parentNode() {
      return this.parent;
    }
    get next() {
      const [next] = dom('siblings', { nodes: [this[HANDLE]], forward: true, any: true });
      return next === undefined ? null : nodeOf(next);
    }
    get nextSibling() {
      return this.next;
    }
    get prev() {
      const [prev] = dom('siblings', { nodes: [this[HANDLE]], forward: false, any: true });
      return prev === undefined ? null : nodeOf(prev);
    }
    get previousSibling() {
      return this.prev;
    }
  }

  const isNode = (value) => value instanceof DomNode;
  const handlesOf = (value) => {
    if (value instanceof Cheerio) return value[HANDLE];
    if (isNode(value)) return [value[HANDLE]];
    if (Array.isArray(value)) return value.filter(isNode).map((node) => node[HANDLE]);
    return [];
  };
  const unique = (handles) => [...new Set(handles)];

  /** Splits a selector list at its top-level commas. */
  const alternatives = (selector) => {
    const parts = [];
    let depth = 0;
    let quote = null;
    let start = 0;
    for (let i = 0; i < selector.length; i++) {
      const char = selector[i];
      if (quote) {
        if (char === quote && selector[i - 1] !== '\\') quote = null;
      } else if (char === '"' || char === "'") {
        quote = char;
      } else if (char === '(' || char === '[') {
        depth++;
      } else if (char === ')' || char === ']') {
        depth--;
      } else if (char === ',' && depth === 0) {
        parts.push(selector.slice(start, i).trim());
        start = i + 1;
      }
    }
    parts.push(selector.slice(start).trim());
    return parts.filter(Boolean);
  };

  // jQuery's positional pseudo-classes pick from the whole match; at a selector's end they're
  // applied to what the rest found. Elsewhere `:first`/`:last` read as Jsoup's first and last
  // child, the nearest it has.
  const POSITIONAL = /:(first|last|even|odd|eq\((-?\d+)\)|gt\((-?\d+)\)|lt\((-?\d+)\))\s*$/;
  const pickPosition = (handles, found) => {
    const [, kind, eq, gt, lt] = found;
    const at = (index) => (index < 0 ? handles.length + index : index);
    if (kind === 'first') return handles.slice(0, 1);
    if (kind === 'last') return handles.slice(-1);
    if (kind === 'even') return handles.filter((_, index) => index % 2 === 0);
    if (kind === 'odd') return handles.filter((_, index) => index % 2 === 1);
    if (eq !== undefined) {
      const index = at(Number(eq));
      return index >= 0 && index < handles.length ? [handles[index]] : [];
    }
    if (gt !== undefined) return handles.slice(at(Number(gt)) + 1);
    return handles.slice(0, Math.max(0, at(Number(lt))));
  };
  const jsoupSelector = (selector) =>
    selector
      .replace(/:first(?![-\w(])/g, ':first-child')
      .replace(/:last(?![-\w(])/g, ':last-child');

  const selectIn = (handles, selector) => {
    if (handles.length === 0) return [];
    const found = [];
    for (const part of alternatives(str(selector))) {
      const positional = POSITIONAL.exec(part);
      if (positional) {
        const rest = part.slice(0, positional.index).trim() || '*';
        found.push(
          ...pickPosition(
            dom('select', { nodes: handles, selector: jsoupSelector(rest) }),
            positional,
          ),
        );
      } else {
        found.push(...dom('select', { nodes: handles, selector: jsoupSelector(part) }));
      }
    }
    return unique(found);
  };

  const matching = (handles, selector) => {
    if (handles.length === 0) return [];
    const kept = new Set();
    for (const part of alternatives(str(selector))) {
      const positional = POSITIONAL.exec(part);
      if (positional) {
        const rest = part.slice(0, positional.index).trim();
        const base = rest
          ? dom('filter', { nodes: handles, selector: jsoupSelector(rest) })
          : handles;
        for (const handle of pickPosition(base, positional)) kept.add(handle);
      } else {
        for (const handle of dom('filter', { nodes: handles, selector: jsoupSelector(part) }))
          kept.add(handle);
      }
    }
    return handles.filter((handle) => kept.has(handle));
  };

  /** HTML for content handed to `append` and the like, and the nodes it moves. */
  const contentOf = (contents) => {
    let html = '';
    const moved = [];
    for (const content of contents.flat()) {
      if (content === undefined || content === null) continue;
      if (content instanceof Cheerio || isNode(content)) {
        const handles = handlesOf(content);
        html += dom('outer', { nodes: handles });
        moved.push(...handles);
      } else {
        html += str(content);
      }
    }
    return { html, moved };
  };

  /** What `map` returns: an array that also answers cheerio's `get` and `toArray`. */
  class Values extends Array {
    get(index) {
      if (index === undefined) return Array.from(this);
      return this[index < 0 ? this.length + index : index];
    }
    toArray() {
      return Array.from(this);
    }
    each(callback) {
      for (let i = 0; i < this.length; i++) {
        if (callback.call(this[i], i, this[i]) === false) break;
      }
      return this;
    }
  }

  class Cheerio {
    constructor(handles, previous, root) {
      Object.defineProperty(this, HANDLE, { value: handles });
      Object.defineProperty(this, '_prev', { value: previous ?? null });
      Object.defineProperty(this, '_root', { value: root ?? null, writable: true });
      this.length = handles.length;
      for (let i = 0; i < handles.length; i++) this[i] = nodeOf(handles[i]);
    }
    _make(handles) {
      return new Cheerio(handles, this, this._root);
    }
    [Symbol.iterator]() {
      return this.toArray()[Symbol.iterator]();
    }
    toArray() {
      return this[HANDLE].map(nodeOf);
    }
    get(index) {
      if (index === undefined) return this.toArray();
      return this[index < 0 ? this.length + index : index];
    }
    eq(index) {
      const at = index < 0 ? this.length + index : index;
      return this._make(at >= 0 && at < this.length ? [this[HANDLE][at]] : []);
    }
    first() {
      return this.eq(0);
    }
    last() {
      return this.eq(-1);
    }
    slice(start, end) {
      return this._make(this[HANDLE].slice(start, end));
    }
    index(target) {
      if (target === undefined) {
        const first = this[0];
        if (!first) return -1;
        const parent = first.parent;
        const siblings = parent ? dom('children', { nodes: [parent[HANDLE]] }) : [];
        return siblings.indexOf(first[HANDLE]);
      }
      const wanted =
        typeof target === 'string' ? this._root.find(target)[HANDLE][0] : handlesOf(target)[0];
      return this[HANDLE].indexOf(wanted);
    }
    each(callback) {
      for (let i = 0; i < this.length; i++) {
        if (callback.call(this[i], i, this[i]) === false) break;
      }
      return this;
    }
    map(callback) {
      const results = [];
      for (let i = 0; i < this.length; i++) {
        const result = callback.call(this[i], i, this[i]);
        if (result === undefined || result === null) continue;
        if (Array.isArray(result)) results.push(...result);
        else results.push(result);
      }
      if (results.length > 0 && results.every(isNode)) return this._make(handlesOf(results));
      return Values.from(results);
    }
    _pick(test, keep) {
      const handles = this[HANDLE];
      let matches;
      if (typeof test === 'string') {
        const set = new Set(matching(handles, test));
        matches = (handle) => set.has(handle);
      } else if (typeof test === 'function') {
        const answers = new Map(
          handles.map((handle, i) => [
            handle,
            Boolean(test.call(nodeOf(handle), i, nodeOf(handle))),
          ]),
        );
        matches = (handle) => answers.get(handle);
      } else {
        const set = new Set(handlesOf(test));
        matches = (handle) => set.has(handle);
      }
      return handles.filter((handle) => matches(handle) === keep);
    }
    filter(test) {
      return this._make(this._pick(test, true));
    }
    not(test) {
      return this._make(this._pick(test, false));
    }
    is(test) {
      return this._pick(test, true).length > 0;
    }
    has(test) {
      return this._make(
        this[HANDLE].filter((handle) =>
          typeof test === 'string'
            ? selectIn([handle], test).length > 0
            : handlesOf(test).some((inner) => dom('parents', { nodes: [inner] }).includes(handle)),
        ),
      );
    }
    find(selector) {
      if (typeof selector !== 'string') {
        const scope = this[HANDLE];
        return this._make(
          handlesOf(selector).filter((inner) =>
            dom('parents', { nodes: [inner] }).some((ancestor) => scope.includes(ancestor)),
          ),
        );
      }
      return this._make(selectIn(this[HANDLE], selector));
    }
    children(selector) {
      const children = dom('children', { nodes: this[HANDLE] });
      return this._make(selector ? matching(children, selector) : children);
    }
    contents() {
      return this._make(dom('children', { nodes: this[HANDLE], all: true }));
    }
    parent(selector) {
      const parents = dom('parent', { nodes: this[HANDLE] });
      return this._make(selector ? matching(parents, selector) : parents);
    }
    parents(selector) {
      const parents = dom('parents', { nodes: this[HANDLE] });
      return this._make(selector ? matching(parents, selector) : parents);
    }
    parentsUntil(selector) {
      const found = [];
      for (const handle of this[HANDLE]) {
        for (const ancestor of dom('parents', { nodes: [handle] })) {
          if (selector && matching([ancestor], selector).length > 0) break;
          found.push(ancestor);
        }
      }
      return this._make(unique(found));
    }
    closest(selector) {
      const found = [];
      for (const handle of this[HANDLE]) {
        const chain = [handle, ...dom('parents', { nodes: [handle] })];
        const [nearest] = typeof selector === 'string' ? matching(chain, selector) : [];
        if (nearest !== undefined) found.push(nearest);
      }
      return this._make(unique(found));
    }
    _siblings(forward, all, selector) {
      const siblings = dom('siblings', { nodes: this[HANDLE], forward, all });
      return this._make(selector ? matching(siblings, selector) : siblings);
    }
    next(selector) {
      return this._siblings(true, false, selector);
    }
    prev(selector) {
      return this._siblings(false, false, selector);
    }
    nextAll(selector) {
      return this._siblings(true, true, selector);
    }
    prevAll(selector) {
      return this._siblings(false, true, selector);
    }
    siblings(selector) {
      const own = new Set(this[HANDLE]);
      const all = [
        ...dom('siblings', { nodes: this[HANDLE], forward: false, all: true }).reverse(),
        ...dom('siblings', { nodes: this[HANDLE], forward: true, all: true }),
      ].filter((handle) => !own.has(handle));
      return this._make(selector ? matching(unique(all), selector) : unique(all));
    }
    _until(forward, selector) {
      const found = [];
      for (const handle of this[HANDLE]) {
        for (const sibling of dom('siblings', { nodes: [handle], forward, all: true })) {
          if (selector && matching([sibling], selector).length > 0) break;
          found.push(sibling);
        }
      }
      return this._make(unique(found));
    }
    nextUntil(selector) {
      return this._until(true, selector);
    }
    prevUntil(selector) {
      return this._until(false, selector);
    }
    end() {
      return this._prev ?? this._make([]);
    }
    addBack(selector) {
      const previous = this._prev ? this._prev[HANDLE] : [];
      const extra = selector ? matching(previous, selector) : previous;
      return this._make(unique([...extra, ...this[HANDLE]]));
    }
    add(other, context) {
      const more =
        typeof other === 'string'
          ? context
            ? $of(this)(other, context)
            : this._root.find(other)
          : other;
      return this._make(unique([...this[HANDLE], ...handlesOf(more)]));
    }
    text(value) {
      if (value === undefined) return dom('text', { nodes: this[HANDLE] });
      dom('setText', { nodes: this[HANDLE], text: str(value) });
      return this;
    }
    html(value) {
      if (value === undefined)
        return this.length ? dom('html', { nodes: [this[HANDLE][0]] }) : null;
      dom('setHtml', { nodes: this[HANDLE], html: contentOf([value]).html });
      return this;
    }
    attr(name, value) {
      if (name && typeof name === 'object') {
        for (const [key, next] of Object.entries(name)) this.attr(key, next);
        return this;
      }
      if (value === undefined) {
        const first = this[0];
        if (!first || first.nodeType !== 1) return undefined;
        if (name === undefined) return { ...first.attribs };
        const attribs = first.attribs;
        const key = str(name).toLowerCase();
        return Object.prototype.hasOwnProperty.call(attribs, key) ? attribs[key] : undefined;
      }
      if (typeof value === 'function') {
        this.each((i, node) => {
          const next = value.call(node, i, node.attribs[str(name).toLowerCase()]);
          if (next !== undefined) {
            dom('setAttr', {
              nodes: [node[HANDLE]],
              name: str(name),
              value: next === null ? null : str(next),
            });
          }
        });
      } else {
        dom('setAttr', {
          nodes: this[HANDLE],
          name: str(name),
          value: value === null ? null : str(value),
        });
      }
      forget(this[HANDLE]);
      return this;
    }
    removeAttr(names) {
      for (const name of str(names).split(/\s+/).filter(Boolean)) {
        dom('setAttr', { nodes: this[HANDLE], name, value: null });
      }
      forget(this[HANDLE]);
      return this;
    }
    prop(name, value) {
      if (value !== undefined) return this.attr(name, value);
      const first = this[0];
      if (!first) return undefined;
      switch (name) {
        case 'tagName':
        case 'nodeName':
          return str(first.name).toUpperCase();
        case 'outerHTML':
          return dom('outer', { nodes: [first[HANDLE]] });
        case 'innerHTML':
          return dom('html', { nodes: [first[HANDLE]] });
        case 'textContent':
        case 'innerText':
          return dom('text', { nodes: [first[HANDLE]] });
        case 'nodeType':
          return first.nodeType;
        default:
          return this.attr(name);
      }
    }
    data(name) {
      const attribs = this[0]?.attribs ?? {};
      const read = (text) => {
        if (text === 'true') return true;
        if (text === 'false') return false;
        if (text === 'null') return null;
        if (text !== '' && !Number.isNaN(Number(text))) return Number(text);
        if (/^[[{]/.test(text)) {
          try {
            return JSON.parse(text);
          } catch {
            return text;
          }
        }
        return text;
      };
      if (name === undefined) {
        const all = {};
        for (const [key, value] of Object.entries(attribs)) {
          if (key.startsWith('data-')) {
            all[key.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] =
              read(value);
          }
        }
        return all;
      }
      const key = `data-${str(name).replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;
      return key in attribs ? read(attribs[key]) : undefined;
    }
    val() {
      const first = this[0];
      if (!first) return undefined;
      return first.name === 'textarea' ? this.text() : this.attr('value');
    }
    hasClass(name) {
      return this.toArray().some((node) =>
        str(node.attribs.class).split(/\s+/).includes(str(name)),
      );
    }
    addClass(names) {
      for (const node of this.toArray()) {
        const classes = str(node.attribs.class).split(/\s+/).filter(Boolean);
        for (const name of str(names).split(/\s+/).filter(Boolean)) {
          if (!classes.includes(name)) classes.push(name);
        }
        dom('setAttr', { nodes: [node[HANDLE]], name: 'class', value: classes.join(' ') });
      }
      forget(this[HANDLE]);
      return this;
    }
    removeClass(names) {
      const gone = str(names).split(/\s+/).filter(Boolean);
      for (const node of this.toArray()) {
        const classes = str(node.attribs.class)
          .split(/\s+/)
          .filter((name) => name && (names === undefined ? false : !gone.includes(name)));
        dom('setAttr', { nodes: [node[HANDLE]], name: 'class', value: classes.join(' ') });
      }
      forget(this[HANDLE]);
      return this;
    }
    toggleClass(name) {
      for (const node of this.toArray()) {
        const single = this._make([node[HANDLE]]);
        if (single.hasClass(name)) single.removeClass(name);
        else single.addClass(name);
      }
      return this;
    }
    css() {
      return undefined;
    }
    remove(selector) {
      dom('remove', { nodes: selector ? matching(this[HANDLE], selector) : this[HANDLE] });
      return this;
    }
    empty() {
      dom('empty', { nodes: this[HANDLE] });
      return this;
    }
    _insert(contents, place) {
      const { html, moved } = contentOf(contents);
      if (html) dom('insert', { nodes: this[HANDLE], html, place });
      if (moved.length) dom('remove', { nodes: moved });
      return this;
    }
    append(...contents) {
      return this._insert(contents, 'append');
    }
    prepend(...contents) {
      return this._insert(contents, 'prepend');
    }
    after(...contents) {
      return this._insert(contents, 'after');
    }
    before(...contents) {
      return this._insert(contents, 'before');
    }
    appendTo(target) {
      $of(this)(target).append(this);
      return this;
    }
    prependTo(target) {
      $of(this)(target).prepend(this);
      return this;
    }
    insertAfter(target) {
      $of(this)(target).after(this);
      return this;
    }
    insertBefore(target) {
      $of(this)(target).before(this);
      return this;
    }
    replaceWith(content) {
      const contents =
        typeof content === 'function' ? [content.call(this[0], 0, this[0])] : [content];
      const { html, moved } = contentOf(contents);
      dom('replace', { nodes: this[HANDLE], html });
      if (moved.length) dom('remove', { nodes: moved });
      return this;
    }
    wrap(wrapper) {
      const shell = contentOf([wrapper]).html;
      const close = shell.lastIndexOf('</');
      if (close < 0) return this;
      for (const node of this.toArray()) {
        const outer = dom('outer', { nodes: [node[HANDLE]] });
        dom('replace', {
          nodes: [node[HANDLE]],
          html: shell.slice(0, close) + outer + shell.slice(close),
        });
      }
      return this;
    }
    clone() {
      return this._make(dom('clone', { nodes: this[HANDLE] }));
    }
    toString() {
      return dom('outer', { nodes: this[HANDLE] });
    }
  }

  /** The `$` a selection came from: one over its page's root. */
  const $of = (selection) => {
    const root = selection._root;
    return (selector, context) => {
      if (selector instanceof Cheerio) return selector;
      if (isNode(selector) || Array.isArray(selector))
        return new Cheerio(handlesOf(selector), null, root);
      return context === undefined
        ? root.find(selector)
        : new Cheerio(handlesOf(context), null, root).find(selector);
    };
  };

  const load = (content) => {
    const html =
      content instanceof Cheerio || isNode(content) || Array.isArray(content)
        ? dom('outer', { nodes: handlesOf(content) })
        : str(content);
    const rootHandle = dom('load', { html });
    const root = new Cheerio([rootHandle], null, null);
    root._root = root;
    const $ = (selector, context) => {
      if (selector === undefined || selector === null || selector === '')
        return new Cheerio([], null, root);
      if (selector instanceof Cheerio) return selector;
      if (isNode(selector) || Array.isArray(selector))
        return new Cheerio(handlesOf(selector), null, root);
      if (typeof selector === 'string') {
        const text = selector.trim();
        if (text.startsWith('<') && text.endsWith('>')) {
          return new Cheerio(dom('parse', { html: text }), null, root);
        }
        if (context === undefined) return root.find(selector);
        const scope =
          typeof context === 'string'
            ? root.find(context)
            : new Cheerio(handlesOf(context), null, root);
        return scope.find(selector);
      }
      return new Cheerio([], null, root);
    };
    $.root = () => root;
    $.html = (selector) =>
      selector === undefined ? dom('html', { nodes: [rootHandle] }) : $(selector).toString();
    $.text = (selector) => (selector === undefined ? root : $(selector)).text();
    $.load = load;
    $.contains = (container, contained) =>
      dom('parents', { nodes: handlesOf(contained) }).includes(handlesOf(container)[0]);
    $.parseHTML = (html) => new Cheerio(dom('parse', { html: str(html) }), null, root).toArray();
    $.fn = Cheerio.prototype;
    return $;
  };

  /** htmlparser2's `Parser`: the page's events, from its parsed tree, as each `write` comes. */
  class Parser {
    constructor(handler = {}) {
      this._handler = handler ?? {};
    }
    write(chunk) {
      const handler = this._handler;
      for (const [kind, first, second] of dom('tokens', { html: str(chunk) })) {
        if (kind === 'open') {
          handler.onopentagname?.(first);
          for (const [name, value] of Object.entries(second ?? {}))
            handler.onattribute?.(name, value, '"');
          handler.onopentag?.(first, second ?? {}, false);
        } else if (kind === 'text') {
          handler.ontext?.(first);
        } else if (kind === 'close') {
          handler.onclosetag?.(first, false);
        }
      }
    }
    end(chunk) {
      if (chunk !== undefined) this.write(chunk);
      this._handler.onend?.();
    }
    parseComplete(data) {
      this.write(data);
      this.end();
    }
    reset() {}
    pause() {}
    resume() {}
  }

  // ---- dayjs (the parts plugins use: parsing, add/subtract, format) ----

  const UNIT = {
    y: 'year',
    year: 'year',
    years: 'year',
    M: 'month',
    month: 'month',
    months: 'month',
    w: 'week',
    week: 'week',
    weeks: 'week',
    d: 'day',
    day: 'day',
    days: 'day',
    D: 'day',
    date: 'day',
    h: 'hour',
    hour: 'hour',
    hours: 'hour',
    m: 'minute',
    minute: 'minute',
    minutes: 'minute',
    s: 'second',
    second: 'second',
    seconds: 'second',
    ms: 'millisecond',
    millisecond: 'millisecond',
    milliseconds: 'millisecond',
  };
  const MONTHS = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  const DAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
  const pad = (number, size = 2) => String(Math.abs(number)).padStart(size, '0');

  class Day {
    constructor(date) {
      this.$d = date;
    }
    isValid() {
      return !Number.isNaN(this.$d.getTime());
    }
    valueOf() {
      return this.$d.getTime();
    }
    unix() {
      return Math.floor(this.valueOf() / 1000);
    }
    toDate() {
      return new Date(this.$d.getTime());
    }
    toISOString() {
      return this.$d.toISOString();
    }
    toJSON() {
      return this.isValid() ? this.toISOString() : null;
    }
    toString() {
      return this.$d.toUTCString();
    }
    year() {
      return this.$d.getFullYear();
    }
    month() {
      return this.$d.getMonth();
    }
    date() {
      return this.$d.getDate();
    }
    day() {
      return this.$d.getDay();
    }
    hour() {
      return this.$d.getHours();
    }
    minute() {
      return this.$d.getMinutes();
    }
    second() {
      return this.$d.getSeconds();
    }
    add(amount, unit = 'ms') {
      const date = new Date(this.$d.getTime());
      const count = Number(amount) || 0;
      switch (UNIT[unit] ?? UNIT[str(unit).toLowerCase()] ?? 'millisecond') {
        case 'year':
          date.setFullYear(date.getFullYear() + count);
          break;
        case 'month':
          date.setMonth(date.getMonth() + count);
          break;
        case 'week':
          date.setDate(date.getDate() + 7 * count);
          break;
        case 'day':
          date.setDate(date.getDate() + count);
          break;
        case 'hour':
          date.setTime(date.getTime() + count * 3600000);
          break;
        case 'minute':
          date.setTime(date.getTime() + count * 60000);
          break;
        case 'second':
          date.setTime(date.getTime() + count * 1000);
          break;
        default:
          date.setTime(date.getTime() + count);
      }
      return new Day(date);
    }
    subtract(amount, unit) {
      return this.add(-(Number(amount) || 0), unit);
    }
    startOf(unit) {
      const date = new Date(this.$d.getTime());
      switch (UNIT[unit] ?? 'day') {
        case 'year':
          date.setMonth(0, 1);
        // falls through
        case 'month':
          if (UNIT[unit] === 'month') date.setDate(1);
        // falls through
        default:
          date.setHours(0, 0, 0, 0);
      }
      return new Day(date);
    }
    isBefore(other) {
      return this.valueOf() < dayjs(other).valueOf();
    }
    isAfter(other) {
      return this.valueOf() > dayjs(other).valueOf();
    }
    diff(other, unit = 'ms') {
      const ms = this.valueOf() - dayjs(other).valueOf();
      const per = {
        year: 31536000000,
        month: 2592000000,
        week: 604800000,
        day: 86400000,
        hour: 3600000,
        minute: 60000,
        second: 1000,
      };
      return Math.trunc(ms / (per[UNIT[unit]] ?? 1));
    }
    format(template = 'YYYY-MM-DDTHH:mm:ssZ') {
      if (!this.isValid()) return 'Invalid Date';
      const date = this.$d;
      const offset = -date.getTimezoneOffset();
      const zone = `${offset >= 0 ? '+' : '-'}${pad(Math.floor(Math.abs(offset) / 60))}:${pad(Math.abs(offset) % 60)}`;
      const tokens = {
        YYYY: String(date.getFullYear()),
        YY: String(date.getFullYear()).slice(-2),
        MMMM: MONTHS[date.getMonth()],
        MMM: MONTHS[date.getMonth()].slice(0, 3),
        MM: pad(date.getMonth() + 1),
        M: String(date.getMonth() + 1),
        DD: pad(date.getDate()),
        D: String(date.getDate()),
        dddd: DAYS[date.getDay()],
        ddd: DAYS[date.getDay()].slice(0, 3),
        dd: DAYS[date.getDay()].slice(0, 2),
        d: String(date.getDay()),
        HH: pad(date.getHours()),
        H: String(date.getHours()),
        hh: pad(date.getHours() % 12 || 12),
        h: String(date.getHours() % 12 || 12),
        mm: pad(date.getMinutes()),
        m: String(date.getMinutes()),
        ss: pad(date.getSeconds()),
        s: String(date.getSeconds()),
        SSS: pad(date.getMilliseconds(), 3),
        A: date.getHours() < 12 ? 'AM' : 'PM',
        a: date.getHours() < 12 ? 'am' : 'pm',
        ZZ: zone.replace(':', ''),
        Z: zone,
      };
      return str(template).replace(
        /\[([^\]]*)]|YYYY|YY|MMMM|MMM|MM|M|DD|D|dddd|ddd|dd|d|HH|H|hh|h|mm|m|ss|s|SSS|A|a|ZZ|Z/g,
        (match, literal) => literal ?? tokens[match],
      );
    }
  }

  /** `dayjs(text, format)` for the numeric formats plugins write (`DD/MM/YYYY`, `YYYY-MM-DD`). */
  const parseWithFormat = (text, format) => {
    const fields = format.match(/YYYY|YY|MM|M|DD|D|HH|H|mm|m|ss|s/g);
    if (!fields) return null;
    const pattern = format
      .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      .replace(/YYYY|YY|MM|M|DD|D|HH|H|mm|m|ss|s/g, (field) =>
        field.length === 4 ? '(\\d{4})' : '(\\d{1,2})',
      );
    const found = new RegExp(`^${pattern}`).exec(str(text).trim());
    if (!found) return null;
    const value = { YYYY: 1970, MM: 1, DD: 1, HH: 0, mm: 0, ss: 0 };
    fields.forEach((field, index) => {
      const number = Number(found[index + 1]);
      const key = { YY: 'YYYY', M: 'MM', D: 'DD', H: 'HH', m: 'mm', s: 'ss' }[field] ?? field;
      value[key] = field === 'YY' ? 2000 + number : number;
    });
    return new Day(new Date(value.YYYY, value.MM - 1, value.DD, value.HH, value.mm, value.ss));
  };

  const dayjs = (input, format) => {
    if (input instanceof Day) return new Day(new Date(input.valueOf()));
    if (input === undefined) return new Day(new Date());
    if (input === null) return new Day(new Date(NaN));
    if (typeof input === 'string' && typeof format === 'string') {
      return parseWithFormat(input, format) ?? new Day(new Date(NaN));
    }
    return new Day(input instanceof Date ? new Date(input.getTime()) : new Date(input));
  };
  dayjs.extend = () => dayjs;
  dayjs.locale = () => 'en';
  dayjs.isDayjs = (value) => value instanceof Day;
  dayjs.unix = (seconds) => new Day(new Date(Number(seconds) * 1000));
  dayjs.default = dayjs;

  // ---- urlencode ----

  const isUtf8 = (charset) => !charset || /^utf-?8$/i.test(str(charset));
  const urlencode = {
    encode: (text, charset) =>
      isUtf8(charset)
        ? encodeURIComponent(str(text))
        : unwrap(native.encode(str(text), str(charset))),
    decode: (text, charset) =>
      isUtf8(charset) ? formDecode(str(text)) : unwrap(native.decode(str(text), str(charset))),
  };

  // ---- LNReader's @libs ----

  const NovelStatus = {
    Unknown: 'Unknown',
    Ongoing: 'Ongoing',
    Completed: 'Completed',
    Licensed: 'Licensed',
    PublishingFinished: 'Publishing Finished',
    Cancelled: 'Cancelled',
    OnHiatus: 'On Hiatus',
    STUB: 'STUB',
    Inactive: 'Inactive',
  };
  const FilterTypes = {
    TextInput: 'Text',
    Picker: 'Picker',
    CheckboxGroup: 'Checkbox',
    Switch: 'Switch',
    ExcludableCheckboxGroup: 'XCheckbox',
  };
  // LNReader's placeholder cover lives on GitHub; Raygun shows its own instead.
  const defaultCover = '';

  // Plugins read their settings through `storage.get`: the user's value (a choice's label read
  // back as its value) wins over what the plugin stored.
  const OPTIONS_KEY = 'lnreader.settingOptions';
  const STORED = 'lnreader.storage.';
  const settingValue = (key) => {
    let value;
    try {
      value = host.settings.get(key);
    } catch {
      value = null;
    }
    if (value === null || value === undefined) return undefined;
    const options = (host.storage.get(OPTIONS_KEY) ?? {})[key];
    if (Array.isArray(options)) {
      const option = options.find(([label]) => label === value);
      if (option) return option[1];
    }
    return value;
  };
  const storage = {
    set(key, value, expires) {
      host.storage.set(STORED + str(key), {
        created: Date.now(),
        value: value === undefined ? null : value,
        expires: expires instanceof Date ? expires.getTime() : (expires ?? null),
      });
    },
    get(key, raw) {
      const fromSettings = settingValue(str(key));
      if (fromSettings !== undefined)
        return raw ? { created: new Date(), value: fromSettings } : fromSettings;
      const item = host.storage.get(STORED + str(key));
      if (!item) return undefined;
      if (item.expires && Date.now() > item.expires) {
        host.storage.remove(STORED + str(key));
        return undefined;
      }
      return raw ? { ...item, created: new Date(item.created) } : item.value;
    },
    getAllKeys() {
      return host.storage
        .keys()
        .filter((key) => key.startsWith(STORED))
        .map((key) => key.slice(STORED.length));
    },
    delete(key) {
      host.storage.remove(STORED + str(key));
    },
    clearAll() {
      for (const key of this.getAllKeys()) this.delete(key);
    },
  };
  // A browser's `localStorage` and `sessionStorage` (a WebView's, in LNReader): kept for the run.
  const pageStorage = () => {
    const db = {};
    return { get: () => db };
  };
  const libs = {
    htmlparser2: { Parser },
    cheerio: { load },
    dayjs: dayjs,
    urlencode: urlencode,
    '@libs/novelStatus': { NovelStatus },
    '@libs/fetch': {
      fetchApi,
      fetchText,
      fetchProto: () => {
        throw new Error(
          "This plugin talks protobuf (fetchProto), which Raygun's LNReader runtime doesn't",
        );
      },
      fetchFile: async () => '',
    },
    '@libs/isAbsoluteUrl': {
      isUrlAbsolute: (url) => /^(?:[a-z][a-z\d+\-.]*:)?\/\//i.test(str(url)),
    },
    '@libs/filterInputs': { FilterTypes },
    '@libs/defaultCover': { defaultCover },
    '@libs/storage': { storage, localStorage: pageStorage(), sessionStorage: pageStorage() },
    '@libs/aes': {
      gcm: () => {
        throw new Error(
          "This plugin decrypts pages with AES-GCM, which Raygun's LNReader runtime doesn't",
        );
      },
    },
    '@libs/utils': {
      utf8ToBytes: (text) =>
        Uint8Array.from(unescape(encodeURIComponent(str(text))), (char) => char.charCodeAt(0)),
      bytesToUtf8: (bytes) => decodeURIComponent(escape(String.fromCharCode(...bytes))),
    },
    '@/types/constants': { NovelStatus, defaultCover },
  };
  define('__lnrRequire', (name) => {
    const lib = libs[name];
    if (lib === undefined)
      throw new Error(`This plugin needs ${name}, which Raygun's LNReader runtime doesn't have`);
    return lib;
  });

  // ---- The plugin as a Raygun novel source ----

  /** Chapters' later pages read for a novel listed page by page. */
  const MAX_CHAPTER_PAGES = 60;
  const STATUS = {
    Ongoing: 'ongoing',
    Completed: 'completed',
    'Publishing Finished': 'publishingFinished',
    Cancelled: 'cancelled',
    'On Hiatus': 'hiatus',
  };

  Object.defineProperty(globalThis, '__lnrStart', {
    configurable: true,
    value: (exports) => {
      delete globalThis.__lnrStart;
      const plugin =
        exports && typeof exports === 'object' && 'default' in exports ? exports.default : exports;
      if (
        !plugin ||
        typeof plugin.popularNovels !== 'function' ||
        typeof plugin.parseNovel !== 'function'
      ) {
        throw new Error('This LNReader plugin defines no source');
      }
      const site = str(plugin.site);

      // Its settings, as Raygun shows them; a choice's labels and values kept for `storage.get`.
      const choices = {};
      for (const [key, setting] of Object.entries(plugin.pluginSettings ?? {})) {
        if (!setting || typeof setting !== 'object') continue;
        const options = Array.isArray(setting.options)
          ? setting.options.map((option) => [
              str(option?.label ?? option).trim(),
              str(option?.value ?? option),
            ])
          : [];
        if (options.length) choices[key] = options;
        try {
          unwrap(
            native.registerSetting(
              JSON.stringify({
                key,
                label: str(setting.label),
                type: str(setting.type),
                value: setting.value ?? null,
                options,
              }),
            ),
          );
        } catch (error) {
          host.log.warn(`setting ${key}: ${error.message}`);
        }
      }
      host.storage.set(OPTIONS_KEY, choices);

      const absolute = (url) => {
        const text = str(url).trim();
        if (!text) return null;
        try {
          const resolved = new URL(text, site || undefined).href;
          return /^https?:/i.test(resolved) ? resolved : null;
        } catch {
          return null;
        }
      };
      const coverHeaders = { ...(plugin.imageRequestInit?.headers ?? {}) };
      const summary = (novel) => ({
        id: str(novel.path),
        title: str(novel.name).trim(),
        coverUrl: absolute(novel.cover),
        coverHeaders,
      });
      const listing = (novels) => {
        const items = (Array.isArray(novels) ? novels : [])
          .filter((novel) => novel && str(novel.path) && str(novel.name).trim())
          .map(summary);
        return { items, hasNextPage: items.length > 0 };
      };

      // Raygun's filter choices, as the plugin's `{ key: { type, value } }`.
      const filterValues = (chosen) => {
        if (!plugin.filters) return undefined;
        const values = {};
        for (const [key, filter] of Object.entries(plugin.filters)) {
          let value = filter.value;
          const picked = chosen?.[key];
          if (picked !== undefined) {
            if (filter.type === FilterTypes.CheckboxGroup) value = picked.include ?? [];
            else if (filter.type === FilterTypes.ExcludableCheckboxGroup) {
              value = { include: picked.include ?? [], exclude: picked.exclude ?? [] };
            } else value = picked;
          }
          values[key] = { type: filter.type, value };
        }
        return values;
      };
      const raygunFilters = () =>
        Object.entries(plugin.filters ?? {}).flatMap(([key, filter]) => {
          const label = str(filter.label) || key;
          const options = (filter.options ?? []).map((option) => ({
            value: str(option.value),
            label: str(option.label),
          }));
          switch (filter.type) {
            case FilterTypes.Picker:
              return options.length
                ? [{ kind: 'select', key, label, options, default: str(filter.value) }]
                : [];
            case FilterTypes.Switch:
              return [{ kind: 'toggle', key, label, default: Boolean(filter.value) }];
            case FilterTypes.CheckboxGroup:
            case FilterTypes.ExcludableCheckboxGroup:
              return options.length ? [{ kind: 'tags', key, label, options }] : [];
            // Raygun's filters have no free text.
            default:
              return [];
          }
        });

      // A title page asks for details, then chapters: one parse serves both.
      const parsed = new Map();
      const novelAt = async (path) => {
        const known = parsed.get(path);
        if (known && Date.now() - known.at < 60000) return known.novel;
        const novel = await plugin.parseNovel(path);
        parsed.set(path, { at: Date.now(), novel });
        if (parsed.size > 8) parsed.delete(parsed.keys().next().value);
        return novel ?? {};
      };
      const names = (text) =>
        str(text)
          .split(/\s*[,;]\s*/)
          .map((name) => name.trim())
          .filter(Boolean);
      const when = (text) => {
        if (text === undefined || text === null || text === '') return null;
        const time = Date.parse(str(text));
        return Number.isFinite(time) ? time : null;
      };

      /** A listing; empty after a refused request, the refusal. */
      const listed = async (ask) => {
        const start = Date.now();
        const result = listing(await ask());
        failIfEmpty(start, result.items.length === 0);
        return result;
      };

      host.providers.register({
        kind: 'novel.source',
        id: 'lnreader',
        name: str(plugin.name) || undefined,
        popular: ({ page }) =>
          listed(() =>
            plugin.popularNovels(page, { showLatestNovels: false, filters: filterValues() }),
          ),
        latest: ({ page }) =>
          listed(() =>
            plugin.popularNovels(page, { showLatestNovels: true, filters: filterValues() }),
          ),
        search: ({ query, page, filters }) => {
          const text = str(query).trim();
          if (text) return listed(() => plugin.searchNovels(text, page));
          return listed(() =>
            plugin.popularNovels(page, { showLatestNovels: false, filters: filterValues(filters) }),
          );
        },
        filters: async () => raygunFilters(),
        details: async ({ id }) => {
          const start = Date.now();
          const novel = await novelAt(id);
          failIfEmpty(start, !str(novel.name).trim());
          let url;
          try {
            url = absolute(
              typeof plugin.resolveUrl === 'function' ? plugin.resolveUrl(id, true) : id,
            );
          } catch {
            url = absolute(id);
          }
          return {
            id,
            title: str(novel.name).trim() || id,
            coverUrl: absolute(novel.cover),
            coverHeaders,
            authors: names(novel.author),
            artists: names(novel.artist),
            description: novel.summary ? str(novel.summary).trim() : null,
            genres: names(novel.genres),
            status: STATUS[str(novel.status)] ?? 'unknown',
            url,
          };
        },
        chapters: async ({ id }) => {
          const start = Date.now();
          const novel = await novelAt(id);
          let chapters = Array.isArray(novel.chapters) ? [...novel.chapters] : [];
          failIfEmpty(start, chapters.length === 0);
          const pages = Math.min(Number(novel.totalPages) || 1, MAX_CHAPTER_PAGES);
          if (pages > 1 && typeof plugin.parsePage === 'function') {
            for (let page = 2; page <= pages; page++) {
              const more = await plugin.parsePage(id, String(page));
              chapters = chapters.concat(Array.isArray(more?.chapters) ? more.chapters : []);
            }
          }
          return {
            chapters: chapters
              .filter((chapter) => chapter && str(chapter.path))
              .map((chapter) => ({
                id: str(chapter.path),
                name: str(chapter.name).trim() || null,
                number:
                  Number.isFinite(Number(chapter.chapterNumber)) &&
                  chapter.chapterNumber !== null &&
                  chapter.chapterNumber !== undefined
                    ? Number(chapter.chapterNumber)
                    : null,
                dateUpload: when(chapter.releaseTime),
                scanlator: Array.isArray(chapter.scanlator)
                  ? chapter.scanlator.join(', ')
                  : (chapter.scanlator ?? null),
              })),
          };
        },
        passage: async ({ chapterId }) => {
          const start = Date.now();
          const html = str(await plugin.parseChapter(chapterId));
          failIfEmpty(start, !html.trim());
          return { html, baseUrl: absolute(site) ?? undefined };
        },
      });
    },
  });
})();
