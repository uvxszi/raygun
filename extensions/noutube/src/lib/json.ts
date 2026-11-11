// Reading YouTube's JSON, whose shape changes without notice: any field may be missing or of
// another type, so values stay `unknown` and are read through these helpers.

/** An object (or array) of YouTube's JSON, its fields unknown. */
export type JsonObject = Record<string, unknown>;

export const isObject = (value: unknown): value is JsonObject =>
  typeof value === 'object' && value !== null;

/** The value at `path` (keys and array indexes) under `value`; undefined once a step is missing. */
export const pick = (value: unknown, ...path: (string | number)[]): unknown => {
  let current = value;
  for (const key of path) {
    if (!isObject(current)) return undefined;
    current = current[key];
  }
  return current;
};

export const pickString = (value: unknown, ...path: (string | number)[]): string | undefined => {
  const found = pick(value, ...path);
  return typeof found === 'string' ? found : undefined;
};

export const pickArray = (value: unknown, ...path: (string | number)[]): unknown[] => {
  const found = pick(value, ...path);
  return Array.isArray(found) ? found : [];
};

/** A text node of YouTube's as text: a plain string, `simpleText`, `content` or its `runs` joined. */
export const textOf = (node: unknown): string => {
  if (typeof node === 'string') return node;
  if (!isObject(node)) return '';
  if (typeof node.simpleText === 'string') return node.simpleText;
  if (typeof node.content === 'string') return node.content;
  if (Array.isArray(node.runs)) {
    return node.runs.map((run: unknown) => pickString(run, 'text') ?? '').join('');
  }
  return '';
};
