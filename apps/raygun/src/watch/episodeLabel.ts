const number = (value: number) => (Number.isInteger(value) ? String(value) : value.toFixed(1));

/** "S1 E2 · Name", "E2 · Name" or the name alone. */
export const episodeLabel = (episode: {
  season: number | null;
  number: number | null;
  name: string;
}) =>
  episode.season !== null && episode.number !== null
    ? `S${number(episode.season)} E${number(episode.number)} · ${episode.name}`
    : episode.number !== null && !episode.name.endsWith(` ${number(episode.number)}`)
      ? `E${number(episode.number)} · ${episode.name}`
      : episode.name;

const UNITS = [
  { unit: 'gigabyte', size: 1024 ** 3 },
  { unit: 'megabyte', size: 1024 ** 2 },
  { unit: 'kilobyte', size: 1024 },
] as const;

/** A file size in the user's language: "512 kB", "4.6 MB", "820 MB", "1.4 GB". */
export const formatBytes = (bytes: number, locale: string) => {
  const { unit, size } = UNITS.find((candidate) => bytes >= candidate.size) ?? UNITS[2];
  const value = bytes / size;
  return new Intl.NumberFormat(locale, {
    style: 'unit',
    unit,
    maximumFractionDigits: value < 10 && unit !== 'kilobyte' ? 1 : 0,
  }).format(value);
};
