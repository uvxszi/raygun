import { parseJson, stringifyJson } from './json';

describe('JSON with 64-bit integers', () => {
  it('keeps HLC stamps whole both ways', () => {
    const text = '{"updated_at_hlc":117606644913012736,"deleted":0,"title":"Song \\"1\\""}';
    const row = parseJson(text) as Record<string, unknown>;
    expect(row.updated_at_hlc).toBe(117606644913012736n);
    expect(row.deleted).toBe(0);
    expect(row.title).toBe('Song "1"');
    expect(stringifyJson(row)).toBe(text);
  });

  it('reads what JSON.parse reads', () => {
    const value = {
      list: [1, -2.5, 3e10, true, false, null, 'ünïcödé ✓'],
      nested: { empty: {}, none: [] },
    };
    expect(parseJson(JSON.stringify(value))).toEqual(value);
    expect(parseJson(' [ 1 , { "a" : "b" } ] ')).toEqual([1, { a: 'b' }]);
  });

  it('leaves out undefined fields as JSON.stringify does', () => {
    expect(stringifyJson({ a: 1, b: undefined, c: [undefined] })).toBe('{"a":1,"c":[null]}');
  });

  it('refuses broken text', () => {
    for (const broken of ['{', '[1,]', '{"a" 1}', 'nul', '"open', '1 2']) {
      expect(() => parseJson(broken)).toThrow(SyntaxError);
    }
  });
});
