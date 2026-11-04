import { namesSeason, parseTitle } from './videoMatch';

/** "12", or "12.5" (as Rust's `format_number`). */
export const formatEpisodeNumber = (value: number) => String(value);

const PLAIN_NUMBER =
  /^(?:(?:episode|épisode|episodio|folge|eps?)\.?\s*|e|#|第\s*)?0*(\d+(?:\.\d+)?)\s*(?:話|话|集|回)?$/iu;
const SEASON_NUMBER =
  /^(?:s0*(\d+)\s*[._-]?\s*e0*(\d+(?:\.\d+)?)|season\s*0*(\d+)\s*[,._:-]?\s*episode\s*0*(\d+(?:\.\d+)?)|0*(\d+)x0*(\d+))$/iu;

/** Whether an episode's name only repeats its number: "Episode 5", "Ep. 05", "S01E05", "5". */
export const isGenericName = (name: string, season: number | null, value: number) => {
  const trimmed = name.trim();
  if (!trimmed) return true;
  const plain = PLAIN_NUMBER.exec(trimmed);
  if (plain) return Number(plain[1]) === value;
  const both = SEASON_NUMBER.exec(trimmed);
  if (!both) return false;
  const named = Number(both[1] ?? both[3] ?? both[5]);
  const episode = Number(both[2] ?? both[4] ?? both[6]);
  return episode === value && (season === null || named === season);
};

/**
 * "S1 E2 · Name", "E2 · Name" or the name alone; a name that only repeats the number
 * ("Episode 2") isn't repeated after it. As `video_match::episode_label` in Rust. `showSeason`
 * false leaves the season out (see `showsSeasons`).
 */
export const episodeLabel = (
  episode: { season: number | null; number: number | null; name: string },
  showSeason = true,
) => {
  const name = episode.name.trim();
  if (episode.number === null) return name;
  const season = showSeason ? episode.season : null;
  const value = formatEpisodeNumber(episode.number);
  const generic = isGenericName(name, season, episode.number);
  if (season !== null) {
    const prefix = `S${formatEpisodeNumber(season)} E${value}`;
    return generic ? prefix : `${prefix} · ${name}`;
  }
  if (name.endsWith(` ${value}`)) return name;
  return generic ? `E${value}` : `E${value} · ${name}`;
};

/**
 * Whether a title's episode labels show the season. Kitsu lists each season as an entry of its
 * own whose episodes are all "season 1" while its name says which season it is ("Attack on
 * Titan Season 2"); there the season is left out rather than shown wrong.
 */
export const showsSeasons = (name: string, episodes: { season: number | null }[]) =>
  !(
    episodes.every((episode) => episode.season === null || episode.season === 1) &&
    namesSeason(parseTitle(name))
  );

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
