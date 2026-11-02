import { LoaderCircle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ChartRow } from '../music/ChartRow';
import { chartKey, useMusicCharts } from '../music/useDiscovery';
import { TopBar } from '../shell/TopBar';

/** Music in Browse all (M46.9): every source's charts as rows, the main ones before genres'. */
export const MusicChartsPage: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const all = charts.data?.charts ?? [];
  const ordered = [...all.filter((chart) => !chart.tag), ...all.filter((chart) => chart.tag)];
  return (
    <section data-testid="music-charts-page" className="flex flex-col gap-7">
      <TopBar back fallback="/browse" title={t('discover.music')} />
      {charts.isPending && (
        <LoaderCircle aria-label={t('discover.loading')} className="text-muted animate-spin" />
      )}
      {charts.data && charts.data.errors.length > 0 && (
        <p className="text-muted text-sm wrap-anywhere">
          {t('discover.chartsFailed', { errors: charts.data.errors.join('; ') })}
        </p>
      )}
      {charts.isSuccess && all.length === 0 && (
        <p className="text-muted text-sm">{t('discover.noCharts')}</p>
      )}
      {ordered.map((chart) => (
        <ChartRow key={chartKey(chart)} chart={chart} />
      ))}
    </section>
  );
};
