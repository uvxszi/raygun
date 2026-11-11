import type { LiveNow } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/** "12.3K" in the reader's language: how many watch a live stream. */
const compactCount = (count: number, language: string): string => {
  try {
    return new Intl.NumberFormat(language, {
      notation: 'compact',
      maximumFractionDigits: 1,
    }).format(count);
  } catch {
    return String(count);
  }
};

/**
 * A live stream's mark on its card: a red "Live", and how many watch when its source says.
 */
export const LiveMark: FC<{ live: LiveNow }> = ({ live }) => {
  const { t, i18n } = useTranslation();
  const viewers =
    typeof live.viewers === 'number' ? compactCount(live.viewers, i18n.language) : null;
  const label = viewers ? t('watch.liveWatching', { viewers }) : t('watch.liveNow');
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      data-testid="live-mark"
      className="inline-flex items-center gap-1"
    >
      <span className="rounded-md bg-red-600 px-1.5 py-0.5 text-3xs font-bold tracking-wide text-white uppercase">
        {t('watch.liveBadge')}
      </span>
      {viewers && (
        <span className="rounded-md bg-black/70 px-1.5 py-0.5 text-3xs font-semibold text-white tabular-nums">
          {viewers}
        </span>
      )}
    </span>
  );
};
