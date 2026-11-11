// `DOMParser` for plugins that scrape pages (Bandcamp, KHInsider, OmniSource, Kugou), over the
// documents Raygun keeps in Rust (`lnreader::dom`): a node is a handle, and only what a plugin
// reads (an attribute, some text) crosses into its heap. The read-only part of the DOM plugins
// use: queries, attributes, text and HTML, and moving around the tree.
import { type Natives, unwrap } from './native';

type Info = {
  type: 'tag' | 'script' | 'style' | 'text' | 'comment' | 'root' | 'directive';
  name?: string;
  attribs?: Record<string, string>;
  data?: string;
};

/** A list of nodes, as `querySelectorAll` and `children` give them. */
type NodeList<T> = T[] & { item(index: number): T | null };

const listOf = <T>(nodes: T[]): NodeList<T> =>
  Object.assign(nodes, { item: (index: number) => nodes[index] ?? null });

const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;
const DOCUMENT_NODE = 9;

/** `data-track-id` → `trackId`. */
const datasetKey = (name: string) =>
  name.slice('data-'.length).replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase());

export const createDomParser = (natives: Natives) => {
  const run = <T>(op: string, args: Record<string, unknown>): T =>
    unwrap<T>(natives.dom(op, JSON.stringify(args)));

  class DomNode {
    readonly #handle: number;
    #info: Info | null = null;

    constructor(handle: number) {
      this.#handle = handle;
    }

    get #meta(): Info {
      this.#info ??= run<Info[]>('info', { nodes: [this.#handle] })[0] ?? { type: 'directive' };
      return this.#info;
    }

    #nodes(op: string, args: Record<string, unknown> = {}): DomNode[] {
      return run<number[]>(op, { nodes: [this.#handle], ...args }).map(wrap);
    }

    get #isElement() {
      const { type } = this.#meta;
      return type === 'tag' || type === 'script' || type === 'style';
    }

    get nodeType() {
      const { type } = this.#meta;
      if (this.#isElement) return ELEMENT_NODE;
      if (type === 'text') return TEXT_NODE;
      if (type === 'comment') return COMMENT_NODE;
      return DOCUMENT_NODE;
    }

    get localName() {
      return this.#isElement ? (this.#meta.name ?? '').toLowerCase() : null;
    }

    get tagName() {
      return this.#isElement ? (this.#meta.name ?? '').toUpperCase() : undefined;
    }

    get nodeName() {
      if (this.#isElement) return this.tagName;
      const { type } = this.#meta;
      return type === 'text' ? '#text' : type === 'comment' ? '#comment' : '#document';
    }

    get textContent(): string {
      const { type, data } = this.#meta;
      if (type === 'text' || type === 'comment') return data ?? '';
      return run<string>('text', { nodes: [this.#handle] });
    }

    get innerText() {
      return this.textContent;
    }

    get data() {
      return this.#meta.data ?? '';
    }

    get nodeValue() {
      const { type, data } = this.#meta;
      return type === 'text' || type === 'comment' ? (data ?? '') : null;
    }

    get innerHTML(): string {
      return run<string | null>('html', { nodes: [this.#handle] }) ?? '';
    }

    get outerHTML(): string {
      return run<string>('outer', { nodes: [this.#handle] });
    }

    getAttribute(name: string): string | null {
      return this.#meta.attribs?.[String(name).toLowerCase()] ?? null;
    }

    hasAttribute(name: string) {
      return this.getAttribute(name) !== null;
    }

    getAttributeNames() {
      return Object.keys(this.#meta.attribs ?? {});
    }

    get attributes() {
      return Object.entries(this.#meta.attribs ?? {}).map(([name, value]) => ({ name, value }));
    }

    get id() {
      return this.getAttribute('id') ?? '';
    }

    get className() {
      return this.getAttribute('class') ?? '';
    }

    get classList() {
      const names = this.className.split(/\s+/).filter(Boolean);
      return Object.assign([...names], {
        contains: (name: string) => names.includes(name),
        item: (index: number) => names[index] ?? null,
      });
    }

    get dataset(): Record<string, string> {
      return Object.fromEntries(
        Object.entries(this.#meta.attribs ?? {})
          .filter(([name]) => name.startsWith('data-'))
          .map(([name, value]) => [datasetKey(name), value]),
      );
    }

    // Links and pictures as written; a parsed page has no address to resolve them against.
    get href() {
      return this.getAttribute('href') ?? '';
    }

    get src() {
      return this.getAttribute('src') ?? '';
    }

    get title() {
      return this.getAttribute('title') ?? '';
    }

    get value() {
      return this.getAttribute('value') ?? '';
    }

    /** Matches below this node (never the node itself, as the DOM has it). */
    querySelectorAll(selector: string): NodeList<DomNode> {
      const handles = run<number[]>('select', {
        nodes: [this.#handle],
        selector: String(selector),
      });
      return listOf(handles.filter((handle) => handle !== this.#handle).map(wrap));
    }

    querySelector(selector: string): DomNode | null {
      return this.querySelectorAll(selector)[0] ?? null;
    }

    getElementsByTagName(name: string) {
      return this.querySelectorAll(name === '*' ? '*' : String(name));
    }

    getElementsByClassName(names: string) {
      const classes = String(names).split(/\s+/).filter(Boolean);
      return this.querySelectorAll(classes.map((name) => `.${name}`).join(''));
    }

    getElementById(id: string) {
      return this.querySelector(`[id="${String(id).replace(/["\\]/g, '\\$&')}"]`);
    }

    matches(selector: string) {
      return (
        run<number[]>('filter', { nodes: [this.#handle], selector: String(selector) }).length > 0
      );
    }

    closest(selector: string): DomNode | null {
      if (this.#isElement && this.matches(selector)) return this;
      const ancestors = run<number[]>('parents', { nodes: [this.#handle] });
      const matching = run<number[]>('filter', { nodes: ancestors, selector: String(selector) });
      return matching.length > 0 ? wrap(matching[0] as number) : null;
    }

    get children() {
      return listOf(this.#nodes('children'));
    }

    get childNodes() {
      return listOf(this.#nodes('children', { all: true }));
    }

    get childElementCount() {
      return this.children.length;
    }

    get firstElementChild(): DomNode | null {
      return this.children[0] ?? null;
    }

    get lastElementChild(): DomNode | null {
      return this.children.at(-1) ?? null;
    }

    get firstChild(): DomNode | null {
      return this.childNodes[0] ?? null;
    }

    get lastChild(): DomNode | null {
      return this.childNodes.at(-1) ?? null;
    }

    get parentElement(): DomNode | null {
      return this.#nodes('parent')[0] ?? null;
    }

    get parentNode(): DomNode | null {
      return this.parentElement;
    }

    get nextElementSibling(): DomNode | null {
      return this.#nodes('siblings', { forward: true })[0] ?? null;
    }

    get previousElementSibling(): DomNode | null {
      return this.#nodes('siblings', { forward: false })[0] ?? null;
    }

    get nextSibling(): DomNode | null {
      return this.#nodes('siblings', { forward: true, any: true })[0] ?? null;
    }

    get previousSibling(): DomNode | null {
      return this.#nodes('siblings', { forward: false, any: true })[0] ?? null;
    }

    // Pages are read here, not edited or watched.
    addEventListener() {}
    removeEventListener() {}
  }

  class DomDocument extends DomNode {
    get documentElement() {
      return this.querySelector('html') ?? this.firstElementChild;
    }

    get head() {
      return this.querySelector('head');
    }

    get body() {
      return this.querySelector('body');
    }

    override get title() {
      return this.querySelector('title')?.textContent.trim() ?? '';
    }
  }

  const wrap = (handle: number) => new DomNode(handle);

  return class DOMParserPolyfill {
    /** Parses `text` as an HTML page, whatever type is asked (XML is read the same way). */
    parseFromString(text: string) {
      return new DomDocument(run<number>('load', { html: String(text ?? '') }));
    }
  };
};
