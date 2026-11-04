import type { SourceGroup } from './protocol';

/**
 * What the player says under its loading ring (as the Android player does): while a session
 * finds a video, how many sources have answered; while a link loads, which one, and when links
 * are tried in turn, how many have been.
 */
export type WaitingNote =
  | { kind: 'checked'; answered: number; total: number }
  | { kind: 'trying'; number: number; of: number; source: string }
  | { kind: 'source'; source: string };

/** A link's source as the viewer knows it: "Vidstream · 1080p", or a stream's own name. */
export const sourceName = (group: SourceGroup, linkIndex: number): string | null => {
  const name = group.name.trim();
  // A Stremio stream's label starts with its name, which often names the addon already.
  const label = (group.links[linkIndex]?.label ?? '').split(' · ')[0]?.trim() ?? '';
  if (!label || label === name) return name || null;
  if (!name || label.startsWith(name)) return label;
  return `${name} · ${label}`;
};

/** "3 of 8 sources checked" while a session's sources answer; null with fewer than two, or
 * once all have. */
export const sourcesChecked = (groups: SourceGroup[]): WaitingNote | null => {
  const answered = groups.filter((group) => group.state !== 'resolving').length;
  return groups.length < 2 || answered >= groups.length
    ? null
    : { kind: 'checked', answered, total: groups.length };
};

/** The link loading: "Trying source 2 of 5 · Vidstream", or only its name when it was picked in
 * Sources or is the first. */
export const sourceNote = (groups: SourceGroup[], picked: boolean): WaitingNote | null => {
  let total = 0;
  let failed = 0;
  let source: string | null = null;
  groups.forEach((group) => {
    group.links.forEach((link, index) => {
      total += 1;
      if (link.failed) failed += 1;
      if (link.current) source = sourceName(group, index);
    });
  });
  if (source === null) return null;
  const number = failed + 1;
  if (picked || total < 2 || number < 2) return { kind: 'source', source };
  return { kind: 'trying', number, of: Math.max(total, number), source };
};
