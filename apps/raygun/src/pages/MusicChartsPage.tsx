import type { ChartGroup, MusicChart } from '@raygun/bindings';
import { LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { issueSummary } from '../lib/issue';
import { ChartRow } from '../music/ChartRow';
import { chartKey, useMusicCharts } from '../music/useDiscovery';
import { TopBar } from '../shell/TopBar';

/** The order Browse → Music lists kinds of chart in (M46.19). */
const GROUPS: ChartGroup[] = [
  'top',
  'trending',
  'new',
  'viral',
  'underground',
  'decade',
  'mood',
  'genre',
];

/**
 * Music in Browse all (M46.9): every source's charts as rows, under a heading per kind (M46.19):
 * top, trending, new, viral, underground, decades, moods, then genres.
 */
export const MusicChartsPage: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const all = charts.data?.charts ?? [];
  const groups = GROUPS.map((group) => ({
    group,
    charts: all.filter((chart: MusicChart) => chart.group === group),
  })).filter((entry) => entry.charts.length > 0);
  return (
    <section data-testid="music-charts-page" className="flex flex-col gap-7">
      <TopBar back fallback="/browse" title={t('discover.music')} />
      {charts.isPending && (
        <LoaderCircle aria-label={t('discover.loading')} className="text-muted animate-spin" />
      )}
      {charts.data && charts.data.errors.length > 0 && (
        <p className="text-muted text-sm wrap-anywhere">
          {t('discover.chartsFailed', {
            errors: charts.data.errors.map((error) => issueSummary(error, t)).join('; '),
          })}
        </p>
      )}
      {charts.isSuccess && all.length === 0 && (
        <p className="text-muted text-sm">{t('discover.noCharts')}</p>
      )}
      {groups.map((entry) => (
        <section
          key={entry.group}
          aria-label={t(`discover.groups.${entry.group}`)}
          className="flex flex-col gap-5"
        >
          <h2 className="text-2xl font-bold">{t(`discover.groups.${entry.group}`)}</h2>
          {entry.charts.map((chart) => (
            <ChartRow key={chartKey(chart)} chart={chart} />
          ))}
        </section>
      ))}
    </section>
  );
};
