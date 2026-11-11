import type { TFunction } from 'i18next';

/**
 * The sites site scripts' match patterns name, for people (`raygun_ext_host::site`'s
 * `site_label`): `https://*.youtube.com/*` is youtube.com, `https://www.example.org/*`
 * example.org, and a pattern for every host says so. Each once, in order.
 */
export const siteLabels = (patterns: readonly string[], t: TFunction): string[] => {
  const labels = patterns.map((pattern) => {
    const host = /^(?:\*|https?):\/\/([^/]+)\//.exec(pattern)?.[1] ?? pattern;
    if (host === '*') return t('extensions.everySite');
    return host.replace(/^\*\./, '').replace(/^www\./, '');
  });
  return [...new Set(labels)];
};
