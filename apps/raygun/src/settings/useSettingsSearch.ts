import { useRouterState } from '@tanstack/react-router';
import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { matchesSearch } from '../shell/useListSearch';
import { useIsDesktop } from '../shell/useIsDesktop';
import { SETTINGS_INDEX } from './searchIndex';
import { SETTINGS_ENTRIES, settingsEntry } from './sections';

/** Results shown at most; a few words narrow them quickly. */
const MAX_RESULTS = 40;
/** How long a found row stays marked. */
const FOUND_MS = 1800;

type Result = { key: string; title: string; trail: string; to: string; hash?: string };

/** Where an index entry lives: a Settings entry's screen or page, or a page's own address. */
const addressOf = (at: string) =>
  at.startsWith('/') ? at : (settingsEntry(at)?.to ?? `/settings/${at}`);

/** The Settings screen a page belongs to, for the result's second line. */
const sectionOf = (at: string) => (at.startsWith('/settings/player') ? 'player' : at);

/** Settings matching `query`: categories by their name, rows by their title or group. */
export const useSettingsSearch = (query: string): Result[] => {
  const { t, i18n } = useTranslation();
  const desktop = useIsDesktop();
  return useMemo(() => {
    if (!query.trim()) return [];
    const sectionTitle = (id: string) => t(`settings.section.${id}.title`);
    const entries: Result[] = SETTINGS_ENTRIES.filter((entry) =>
      matchesSearch(query, [sectionTitle(entry.id)]),
    ).map((entry) => ({
      key: `entry:${entry.id}`,
      title: sectionTitle(entry.id),
      trail: t('settings.title'),
      to: entry.to ?? `/settings/${entry.id}`,
    }));
    const rows: Result[] = SETTINGS_INDEX.filter(
      (row) =>
        i18n.exists(row.title) && (row.only === undefined || (row.only === 'desktop') === desktop),
    )
      .map((row) => {
        const title = t(row.title);
        const section = sectionTitle(sectionOf(row.at));
        const group = row.group && row.group !== row.title ? t(row.group) : null;
        return {
          key: `${row.at}:${row.title}`,
          title,
          trail: [section, group].filter(Boolean).join(' › '),
          to: addressOf(row.at),
          hash: row.title,
        };
      })
      .filter((row) => matchesSearch(query, [row.title, row.trail]));
    return [...entries, ...rows].slice(0, MAX_RESULTS);
  }, [query, t, i18n, desktop]);
};

/** The row or group a search result points at (by its title, or an element's id). */
const findTarget = (title: string, hash: string) => {
  const escaped = CSS.escape(title);
  return (
    document.querySelector<HTMLElement>(`[data-pref="${escaped}"]`) ??
    document.querySelector<HTMLElement>(`section[aria-label="${escaped}"]`) ??
    document.getElementById(hash)
  );
};

/**
 * On a settings screen opened from search (the row's title key in the address's hash): scrolls
 * to the row once it shows, and marks it a moment.
 */
export const useFoundPref = () => {
  const { t, i18n } = useTranslation();
  const hash = useRouterState({ select: (state) => state.location.hash });
  useEffect(() => {
    if (!hash || typeof CSS === 'undefined') return;
    const title = i18n.exists(hash) ? t(hash) : hash;
    let marked: HTMLElement | null = null;
    let tries = 0;
    let frame = 0;
    let timer = 0;
    // Rows show once settings load: look again for a moment.
    const look = () => {
      const target = findTarget(title, hash);
      if (!target) {
        if ((tries += 1) < 60) frame = requestAnimationFrame(look);
        return;
      }
      marked = target;
      target.scrollIntoView?.({ block: 'center' });
      target.dataset.found = '';
      timer = window.setTimeout(() => delete target.dataset.found, FOUND_MS);
    };
    look();
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
      if (marked) delete marked.dataset.found;
    };
  }, [hash, t, i18n]);
};
