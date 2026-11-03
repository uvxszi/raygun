import { useState } from 'react';
import { useTranslation } from 'react-i18next';

export const DAY_MS = 24 * 60 * 60 * 1000;

/** Start of the local day `ms` falls in. */
export const dayStart = (ms: number) => {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
};

/** Groups updates by the local day they were found (or `when` says), keeping their order. */
export const groupByDay = <Item extends object>(
  items: Item[],
  when: (item: Item) => number = (item) => (item as { foundAt?: number | null }).foundAt ?? 0,
) => {
  const groups: { day: number; items: Item[] }[] = [];
  for (const item of items) {
    const day = dayStart(when(item));
    const last = groups.at(-1);
    if (last?.day === day) {
      last.items.push(item);
    } else {
      groups.push({ day, items: [item] });
    }
  }
  return groups;
};

/** The start of the day `days` after `day` (calendar days: a DST change makes one 23 or 25 h). */
const shiftDays = (day: number, days: number) => {
  const date = new Date(day);
  date.setDate(date.getDate() + days);
  return date.getTime();
};

/** "Today", "Yesterday", "Tomorrow" or the date, for a day heading. */
export const useDayLabel = () => {
  const { t, i18n } = useTranslation();
  const [today] = useState(() => dayStart(Date.now()));
  return (day: number) => {
    if (day === today) return t('manga.today');
    if (day === shiftDays(today, -1)) return t('manga.yesterday');
    if (day === shiftDays(today, 1)) return t('manga.tomorrow');
    return new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(day);
  };
};
