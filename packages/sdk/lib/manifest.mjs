// The checks Raygun makes of `raygun-extension.json`, mirrored from the host
// (crates/raygun-ext-host/src/manifest.rs and permissions.rs) so mistakes show before the app
// refuses the extension, and the provider kinds the app calls (`kinds.json`).

import { readFileSync } from 'node:fs';

/**
 * @typedef {{ label: string, required: string[], optional: string[], args: Record<string, Record<string, unknown>> }} KindInfo
 * @typedef {{ errors: string[], warnings: string[] }} Checked
 */

/**
 * Every provider kind the app calls, with its required and optional calls and the arguments the
 * app passes each (as `raygun-ext test` fills them in).
 * @type {Record<string, KindInfo>}
 */
export const KINDS = JSON.parse(readFileSync(new URL('./kinds.json', import.meta.url), 'utf8'));

/** The fields Raygun reads; it refuses a manifest with any other. */
const FIELDS = new Set([
  'id',
  'name',
  'version',
  'apiVersion',
  'runtime',
  'entry',
  'provides',
  'permissions',
  'contentRating',
  'settings',
  'description',
  'author',
  'website',
  // Written by Raygun's Nuclear converter.
  'nuclear',
]);
const PERMISSION_FIELDS = new Set(['net', 'webview']);
const SETTING_FIELDS = new Set(['key', 'type', 'label', 'options', 'default']);
const SETTING_TYPES = ['text', 'number', 'boolean', 'select'];
const RATINGS = ['safe', 'suggestive', 'nsfw'];

// semver.org's pattern: no leading zeros, dotted pre-release and build identifiers.
const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

/** @param {string} label @param {string} extra */
const isLabel = (label, extra) =>
  label.length > 0 &&
  /^[a-z0-9]/.test(label) &&
  [...label].every((char) => /[a-z0-9]/.test(char) || extra.includes(char));

/** Reverse-DNS: at least two dot-separated labels of `[a-z0-9-]`, each starting alphanumerically. */
export const isExtensionId = (/** @type {unknown} */ id) =>
  typeof id === 'string' &&
  id.length <= 100 &&
  id.split('.').length >= 2 &&
  id.split('.').every((label) => isLabel(label, '-'));

/** Provider kinds look like `music.metadata`, `manga.source`, `lyrics`. */
export const isKind = (/** @type {unknown} */ kind) =>
  typeof kind === 'string' &&
  kind.length <= 64 &&
  kind.split('.').every((label) => isLabel(label, '-'));

/** A provider id within an extension: `[a-z0-9][a-z0-9_-]*`, at most 64 characters. */
export const isProviderId = (/** @type {unknown} */ id) =>
  typeof id === 'string' && id.length <= 64 && isLabel(id, '-_');

/**
 * A `permissions.net` pattern as the host reads it: `*`, a host, or `*.` and a host with a dot.
 * @param {string} pattern
 * @returns {{ any: true } | { host: string, subdomains: boolean } | null}
 */
export const parseHostPattern = (pattern) => {
  const text = pattern.trim().toLowerCase();
  if (text === '*') return { any: true };
  const subdomains = text.startsWith('*.');
  const host = subdomains ? text.slice(2) : text;
  const valid =
    host !== '' &&
    host.length <= 253 &&
    host
      .split('.')
      .every((label) => label !== '' && !label.startsWith('-') && /^[a-z0-9-]+$/.test(label)) &&
    (!subdomains || host.includes('.'));
  return valid ? { host, subdomains } : null;
};

/**
 * A host name or address outside this machine and the local network, as `*` allows.
 * @param {string} host
 */
const isPublicHost = (host) => {
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) {
    const [first = 0, second = 0] = host.split('.').map(Number);
    return !(
      first === 0 ||
      first === 10 ||
      first === 127 ||
      (first === 100 && second >= 64 && second < 128) ||
      (first === 169 && second === 254) ||
      (first === 172 && second >= 16 && second < 32) ||
      (first === 192 && second === 168) ||
      first >= 224
    );
  }
  return !(
    host === 'localhost' ||
    ['.localhost', '.local', '.internal', '.lan', '.home.arpa'].some((end) => host.endsWith(end)) ||
    !host.includes('.')
  );
};

/**
 * Whether `permissions.net` lets the extension reach `host`.
 * @param {unknown} patterns
 * @param {string} host
 */
export const allowsHost = (patterns, host) => {
  const name = host.toLowerCase().replace(/\.$/, '');
  return (Array.isArray(patterns) ? patterns : []).some((pattern) => {
    const parsed = typeof pattern === 'string' ? parseHostPattern(pattern) : null;
    if (!parsed) return false;
    if ('any' in parsed) return isPublicHost(name);
    if (!parsed.subdomains) return name === parsed.host;
    return name.endsWith(`.${parsed.host}`) && name.length > parsed.host.length + 1;
  });
};

/**
 * @param {Record<string, unknown>} setting
 * @param {unknown} value
 */
const settingAccepts = (setting, value) => {
  switch (setting.type) {
    case 'text':
      return typeof value === 'string';
    case 'number':
      return typeof value === 'number' && Number.isFinite(value);
    case 'boolean':
      return typeof value === 'boolean';
    case 'select':
      return (
        typeof value === 'string' &&
        Array.isArray(setting.options) &&
        setting.options.includes(value)
      );
    default:
      return false;
  }
};

/**
 * @param {unknown} settings
 * @param {string[]} errors
 */
const checkSettings = (settings, errors) => {
  if (settings === undefined) return;
  if (!Array.isArray(settings)) {
    errors.push('settings must be a list of settings');
    return;
  }
  const keys = new Set();
  settings.forEach((setting, index) => {
    if (typeof setting !== 'object' || setting === null || Array.isArray(setting)) {
      errors.push(`settings[${index}] must be an object with a key and a type`);
      return;
    }
    const { key, type, label, options } = setting;
    const name = typeof key === 'string' && key !== '' ? `setting "${key}"` : `settings[${index}]`;
    for (const field of Object.keys(setting)) {
      if (!SETTING_FIELDS.has(field)) {
        errors.push(
          `${name}: unknown field "${field}" (a setting has key, type, label, options and default)`,
        );
      }
    }
    if (typeof key !== 'string' || key === '') errors.push(`${name} needs a key`);
    else if (keys.has(key)) errors.push(`${name} is declared twice`);
    keys.add(key);
    if (typeof type !== 'string' || !SETTING_TYPES.includes(type)) {
      errors.push(`${name}: type must be one of ${SETTING_TYPES.join(', ')}`);
    }
    if (label !== undefined && label !== null && typeof label !== 'string') {
      errors.push(`${name}: label must be text`);
    }
    if (
      options !== undefined &&
      (!Array.isArray(options) || options.some((option) => typeof option !== 'string'))
    ) {
      errors.push(`${name}: options must be a list of text`);
    } else if (type === 'select' && (!Array.isArray(options) || options.length === 0)) {
      errors.push(`${name}: a select needs options`);
    }
    if (
      'default' in setting &&
      setting.default !== null &&
      !settingAccepts(setting, setting.default)
    ) {
      errors.push(
        type === 'select'
          ? `${name}: the default must be one of its options`
          : `${name}: the default must be a ${type === 'text' ? 'string' : type}`,
      );
    }
  });
};

/**
 * Problems with a manifest: `errors` make Raygun refuse the extension, `warnings` are mistakes it
 * passes over (a kind it never calls, a website it drops).
 * @param {unknown} value
 * @returns {Checked}
 */
export const checkManifest = (value) => {
  /** @type {string[]} */
  const errors = [];
  /** @type {string[]} */
  const warnings = [];
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { errors: ['the manifest must be a JSON object'], warnings };
  }
  const manifest = /** @type {Record<string, unknown>} */ (value);
  const { id, name, version, apiVersion, runtime, entry, provides, permissions } = manifest;
  for (const field of Object.keys(manifest)) {
    if (!FIELDS.has(field)) {
      errors.push(`unknown field "${field}": Raygun refuses manifests with fields it doesn't know`);
    }
  }
  if (!isExtensionId(id)) {
    errors.push('id must be reverse-DNS in lower case, like dev.example.my-source');
  }
  if (typeof name !== 'string' || name.trim() === '' || [...name.trim()].length > 80) {
    errors.push('name must be 1-80 characters');
  }
  if (typeof version !== 'string' || !SEMVER.test(version)) {
    errors.push('version must be semver, like 1.0.0');
  }
  if (apiVersion !== 1) errors.push('apiVersion must be 1');
  if (runtime !== 'js') errors.push('runtime must be "js"');
  if (
    typeof entry !== 'string' ||
    !entry.endsWith('.js') ||
    entry.includes('\\') ||
    entry.includes(':') ||
    entry.split('/').some((part) => part === '' || part === '.' || part === '..')
  ) {
    errors.push('entry must be a relative path to a .js file, like dist/index.js');
  }
  if (!Array.isArray(provides) || provides.length === 0) {
    errors.push('provides must list at least one provider kind, like "music.metadata"');
  } else {
    for (const kind of provides) {
      if (!isKind(kind)) errors.push(`provides: ${JSON.stringify(kind)} isn't a provider kind`);
      else if (!(kind in KINDS)) {
        warnings.push(
          `provides: Raygun never calls providers of kind "${kind}" (it calls ${Object.keys(KINDS).join(', ')})`,
        );
      }
    }
  }
  if (permissions !== undefined) {
    if (typeof permissions !== 'object' || permissions === null || Array.isArray(permissions)) {
      errors.push('permissions must be an object with net (and webview)');
    } else {
      const { net, webview } = /** @type {Record<string, unknown>} */ (permissions);
      for (const field of Object.keys(permissions)) {
        if (!PERMISSION_FIELDS.has(field)) {
          errors.push(`permissions: unknown field "${field}" (permissions have net and webview)`);
        }
      }
      if (net !== undefined && !Array.isArray(net)) {
        errors.push('permissions.net must be a list of hosts');
      }
      for (const pattern of Array.isArray(net) ? net : []) {
        if (typeof pattern !== 'string' || parseHostPattern(pattern) === null) {
          errors.push(
            `permissions.net: ${JSON.stringify(pattern)} isn't a host; use api.example.org, *.example.org or *`,
          );
        }
      }
      if (webview !== undefined && typeof webview !== 'boolean') {
        errors.push('permissions.webview must be true or false');
      } else if (webview === true) {
        warnings.push('permissions.webview is reserved and does nothing yet; leave it out');
      }
    }
  }
  const rating = manifest.contentRating;
  if (rating !== undefined && (typeof rating !== 'string' || !RATINGS.includes(rating))) {
    errors.push(`contentRating must be one of ${RATINGS.join(', ')}`);
  }
  for (const field of ['description', 'author', 'website']) {
    const text = manifest[field];
    if (text !== undefined && text !== null && typeof text !== 'string') {
      errors.push(`${field} must be text`);
    }
  }
  const { website } = manifest;
  if (typeof website === 'string' && !/^https?:\/\//.test(website)) {
    warnings.push('website must start with https:// or http://; Raygun ignores it otherwise');
  }
  if (manifest.nuclear !== undefined) {
    warnings.push('nuclear is written by Raygun when it converts a Nuclear plugin; leave it out');
  }
  checkSettings(manifest.settings, errors);
  return { errors, warnings };
};

/**
 * The errors alone: what makes Raygun refuse the manifest.
 * @param {unknown} manifest
 * @returns {string[]}
 */
export const manifestProblems = (manifest) => checkManifest(manifest).errors;
