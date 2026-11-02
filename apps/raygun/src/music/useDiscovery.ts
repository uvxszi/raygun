import { commands } from '@raygun/bindings';
import type { MusicChart, MusicTag, TrackTags } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

/** The country the app language points at ("en-GB" → "gb"), for charts by country (M46.9). */
export const autoRegion = (): string => {
  try {
    const region = new Intl.Locale(navigator.language).maximize().region;
    return region && /^[a-z]{2}$/i.test(region) ? region.toLowerCase() : 'us';
  } catch {
    return 'us';
  }
};

/** The charts' country: the one picked in Settings, or the app language's. */
export const useChartRegion = () => {
  const settings = useSettings();
  return settings.data?.discovery?.region || autoRegion();
};

/** Every source's charts for the chosen country. */
export const useMusicCharts = () => {
  const region = useChartRegion();
  return useQuery({
    queryKey: ['music', 'charts', region],
    queryFn: () => unwrapResult(commands.musicCharts(region)),
    staleTime: 30 * 60_000,
  });
};

/** A chart's address in the app: `<provider id>:<chart id>`. */
export const chartKey = (chart: Pick<MusicChart, 'id'> & { provider: { id: string } }) =>
  `${chart.provider.id}:${chart.id}`;

/** A chart's songs; `enabled` holds the request until the row is near the window. */
export const useChartSongs = (providerId: string, chartId: string, enabled = true) => {
  const region = useChartRegion();
  return useQuery({
    queryKey: ['music', 'chart', providerId, chartId, region],
    queryFn: () => unwrapResult(commands.musicChart(providerId, chartId, region)),
    enabled: enabled && providerId !== '' && chartId !== '',
    staleTime: 20 * 60_000,
  });
};

/** Home's rows pick these charts first, then any chart that isn't a genre's. */
const POPULAR = ['dev.raygun.apple-charts/charts:most-played', 'dev.raygun.deezer/charts:0'];
const TRENDING = ['dev.raygun.listenbrainz/charts:week', 'dev.raygun.listenbrainz/charts:month'];

export const pickChart = (charts: MusicChart[], which: 'popular' | 'trending') => {
  const preferred = which === 'popular' ? POPULAR : TRENDING;
  return (
    preferred
      .map((key) => charts.find((chart) => chartKey(chart) === key))
      .find((chart) => chart !== undefined) ??
    (which === 'popular' ? charts.find((chart) => !chart.tag) : undefined) ??
    null
  );
};

/** The tags across the user's songs, the most common first. */
export const useTagSummary = () =>
  useQuery({
    queryKey: ['music', 'tags', 'summary'],
    queryFn: () => unwrapResult(commands.musicTagSummary()),
    staleTime: 60_000,
  });

/** Every genre and mood Raygun knows, moods first. */
export const useTagVocabulary = () =>
  useQuery({
    queryKey: ['music', 'tags', 'vocabulary'],
    queryFn: () => commands.musicTagVocabulary(),
    staleTime: Infinity,
  });

/** Favorites and playlist songs with a tag. */
export const useTagSongs = (name: string) =>
  useQuery({
    queryKey: ['music', 'tags', 'songs', name],
    queryFn: () => unwrapResult(commands.musicTagSongs(name)),
    enabled: name !== '',
  });

/** Stored tags for songs by track key, as a map; songs without tags are missing from it. */
export const useTagsFor = (keys: string[]) =>
  useQuery({
    queryKey: ['music', 'tags', 'for', keys],
    queryFn: async () => {
      const rows: TrackTags[] = await unwrapResult(commands.musicTagsFor(keys));
      return new Map<string, MusicTag[]>(rows.map((row) => [row.trackKey, row.tags]));
    },
    enabled: keys.length > 0,
    staleTime: 60_000,
  });

/** A colour for a tile, steady for its name (Spotify's Browse all). */
const TILE_COLORS = [
  '#dc148c',
  '#006450',
  '#8400e7',
  '#1e3264',
  '#e8115b',
  '#148a08',
  '#bc5900',
  '#e91429',
  '#477d95',
  '#7358ff',
  '#af2896',
  '#503750',
  '#0d73ec',
  '#ba5d07',
  '#8d67ab',
  '#27856a',
];

export const tileColor = (name: string) => {
  let hash = 0;
  for (const character of name) hash = (hash * 31 + (character.codePointAt(0) ?? 0)) >>> 0;
  return TILE_COLORS[hash % TILE_COLORS.length] ?? TILE_COLORS[0];
};
