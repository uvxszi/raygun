// JSON that keeps 64-bit integers whole. Sync rows carry HLC stamps above 2^53, which
// `JSON.parse` would round; here integers that don't fit a number exactly become BigInts, and
// BigInts are written back as plain digits.

const WHITESPACE = new Set([' ', '\t', '\n', '\r']);

class Reader {
  private at = 0;
  constructor(private readonly text: string) {}

  read(): unknown {
    const value = this.value();
    this.skip();
    if (this.at !== this.text.length) this.fail('trailing text');
    return value;
  }

  private fail(why: string): never {
    throw new SyntaxError(`bad JSON at ${this.at}: ${why}`);
  }

  private skip() {
    while (this.at < this.text.length && WHITESPACE.has(this.text[this.at] ?? '')) this.at += 1;
  }

  private value(): unknown {
    this.skip();
    const char = this.text[this.at];
    switch (char) {
      case '{':
        return this.object();
      case '[':
        return this.array();
      case '"':
        return this.string();
      case 't':
        return this.word('true', true);
      case 'f':
        return this.word('false', false);
      case 'n':
        return this.word('null', null);
      default:
        if (char === '-' || (char !== undefined && char >= '0' && char <= '9'))
          return this.number();
        return this.fail(`unexpected ${char ?? 'end'}`);
    }
  }

  private word(word: string, value: unknown) {
    if (!this.text.startsWith(word, this.at)) this.fail(`expected ${word}`);
    this.at += word.length;
    return value;
  }

  private number(): number | bigint {
    const match = /^-?(?:0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?/.exec(this.text.slice(this.at));
    if (!match) return this.fail('bad number');
    const literal = match[0];
    this.at += literal.length;
    if (match[1] === undefined && match[2] === undefined) {
      const number = Number(literal);
      return Number.isSafeInteger(number) ? number : BigInt(literal);
    }
    return Number(literal);
  }

  private string(): string {
    // The literal's end: a quote no backslash escapes.
    let end = this.at + 1;
    while (end < this.text.length) {
      const char = this.text[end];
      if (char === '\\') end += 2;
      else if (char === '"') break;
      else end += 1;
    }
    if (end >= this.text.length) this.fail('unterminated string');
    const literal = this.text.slice(this.at, end + 1);
    this.at = end + 1;
    return JSON.parse(literal) as string;
  }

  private array(): unknown[] {
    this.at += 1;
    const items: unknown[] = [];
    this.skip();
    if (this.text[this.at] === ']') {
      this.at += 1;
      return items;
    }
    for (;;) {
      items.push(this.value());
      this.skip();
      const char = this.text[this.at];
      this.at += 1;
      if (char === ']') return items;
      if (char !== ',') this.fail('expected , or ]');
    }
  }

  private object(): Record<string, unknown> {
    this.at += 1;
    const object: Record<string, unknown> = {};
    this.skip();
    if (this.text[this.at] === '}') {
      this.at += 1;
      return object;
    }
    for (;;) {
      this.skip();
      if (this.text[this.at] !== '"') this.fail('expected a key');
      const key = this.string();
      this.skip();
      if (this.text[this.at] !== ':') this.fail('expected :');
      this.at += 1;
      object[key] = this.value();
      this.skip();
      const char = this.text[this.at];
      this.at += 1;
      if (char === '}') return object;
      if (char !== ',') this.fail('expected , or }');
    }
  }
}

/** Parses JSON, keeping integers beyond 2^53 as BigInts. */
export const parseJson = (text: string): unknown => new Reader(text).read();

/** Writes JSON, BigInts as plain digits; `undefined` fields are left out as `JSON.stringify` does. */
export const stringifyJson = (value: unknown): string => {
  if (typeof value === 'bigint') return value.toString();
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value) ?? 'null';
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => (item === undefined ? 'null' : stringifyJson(item))).join(',')}]`;
  }
  const fields = Object.entries(value as Record<string, unknown>)
    .filter(([, field]) => field !== undefined && typeof field !== 'function')
    .map(([key, field]) => `${JSON.stringify(key)}:${stringifyJson(field)}`);
  return `{${fields.join(',')}}`;
};
