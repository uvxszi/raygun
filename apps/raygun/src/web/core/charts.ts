// Charts from `music.charts` sources in the browser (raygun-ext-host's `music_charts` and
// `music_chart`): Home's chart rows and Browse → Music's charts.
import type {
  ChartGroup,
  MusicChart_Serialize,
  MusicCharts_Serialize,
  OnlineTrack,
  ProviderInfo_Serialize,
} from '@raygun/bindings';

import { trackOf } from '../extensions/service';
import type { ExtensionsService } from '../extensions/service';

const GROUPS = new Set<ChartGroup>([
  'top',
  'trending',
  'new',
  'viral',
  'genre',
  'mood',
  'decade',
  'underground',
]);

const text = (value: unknown) =>
  typeof value === 'string' && value.trim() ? value.trim() : null;

/** A `charts` answer (`parse_charts`): charts with an id and a name. */
export const chartsOf = (
  provider: ProviderInfo_Serialize,
  answer: unknown,
): MusicChart_Serialize[] => {
  const charts = (answer as { charts?: unknown })?.charts;
  if (!Array.isArray(charts)) return [];
  return charts.flatMap((raw) => {
    const chart = (raw ?? {}) as Record<string, unknown>;
    const id =
      typeof chart.id === 'number' && Number.isFinite(chart.id) ? String(chart.id) : text(chart.id);
    const name = text(chart.name);
    if (!id || !name) return [];
    const tag = text(chart.tag);
    const group = text(chart.group) as ChartGroup | null;
    const artwork = text(chart.artworkUrl);
    return [
      {
        provider,
        id,
        name,
        description: text(chart.description),
        artworkUrl: artwork && /^https?:\/\//i.test(artwork) ? artwork : null,
        tag,
        group: group && GROUPS.has(group) ? group : tag ? 'genre' : 'top',
      },
    ];
  });
};

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

export class ChartsService {
  private kept: { at: number; region: string | null; answer: MusicCharts_Serialize } | null = null;

  constructor(private readonly extensions: ExtensionsService) {}

  async charts(region: string | null): Promise<MusicCharts_Serialize> {
    if (this.kept && this.kept.region === region && Date.now() - this.kept.at < 30 * 60_000) {
      return this.kept.answer;
    }
    const providers = (await this.extensions.listed('music.charts')).filter(
      (provider) => provider.enabled,
    );
    const charts: MusicChart_Serialize[] = [];
    const errors: string[] = [];
    await Promise.all(
      providers.map(async ({ info }) => {
        try {
          charts.push(...chartsOf(info, await this.extensions.call(info.id, 'charts', { region })));
        } catch (error) {
          errors.push(`${info.name}: ${message(error)}`);
        }
      }),
    );
    const answer = { charts, errors };
    if (charts.length > 0) this.kept = { at: Date.now(), region, answer };
    return answer;
  }

  async chart(providerId: string, chartId: string, region: string | null): Promise<OnlineTrack[]> {
    const answer = await this.extensions.call<{ tracks?: unknown[] }>(providerId, 'chart', {
      id: chartId,
      region,
      limit: 50,
    });
    return (answer?.tracks ?? [])
      .map(trackOf)
      .filter((track): track is OnlineTrack => track !== null);
  }
}
