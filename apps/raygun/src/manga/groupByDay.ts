import type { MangaUpdateItem } from '@raygun/bindings';

export const DAY_MS = 24 * 60 * 60 * 1000;

/** Start of the local day `ms` falls in. */
export const dayStart = (ms: number) => {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
};

/** Groups updates by the local day they were found, keeping their order (newest first). */
export const groupByDay = (items: MangaUpdateItem[]) => {
  const groups: { day: number; items: MangaUpdateItem[] }[] = [];
  for (const item of items) {
    const day = dayStart(item.foundAt ?? 0);
    const last = groups.at(-1);
    if (last?.day === day) {
      last.items.push(item);
    } else {
      groups.push({ day, items: [item] });
    }
  }
  return groups;
};
