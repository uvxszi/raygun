import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ladderStep } from './mediaEngine';
import type { MediaEngine } from './mediaEngine';
import { PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import type { QualityRow } from './protocol';
import { stepName } from './qualityText';
import { useVideoPlayerStore } from './videoPlayerStore';

type Props = {
  videoKey: string;
  positionMs: number;
  engine: MediaEngine | null;
  onClose: () => void;
  /** A link of another size was asked for: what to say while it comes. */
  onAsked: (text: string) => void;
};

/**
 * The Quality button's panel: resolutions from 4K down, as Rust lists them (`qualities`,
 * `video_quality::rows`). A step the stream playing has (an HLS variant) switches here; one the
 * source playing has as another link, or only another source has, is asked of Rust, which plays
 * it from where the video is. Tapping the step that plays again tries the next source with it.
 */
export const QualityPanel: FC<Props> = ({ videoKey, positionMs, engine, onClose, onAsked }) => {
  const { t } = useTranslation();
  const rows = useVideoPlayerStore((state) => state.qualities);
  const session = useVideoPlayerStore((state) => state.session);
  const variants = engine?.variants() ?? [];
  const auto = variants.length > 1 && engine?.quality() === -1;

  const detail = (row: QualityRow): string | null => {
    switch (row.state) {
      case 'playing':
        return [t('videoPlayer.qualityPlaying'), row.codec].filter(Boolean).join(' · ');
      case 'otherSource':
        return t('videoPlayer.qualityElsewhere', { n: row.source });
      case 'missing':
        return t('videoPlayer.qualityMissing');
      default:
        return row.codec;
    }
  };

  const pick = (row: QualityRow) => {
    const name = stepName(row.height, row.range, t);
    if (row.state === 'variant') {
      // The best variant of this height and range.
      const variant = variants
        .filter(
          (candidate) => ladderStep(candidate.height) === row.height && candidate.range === row.range,
        )
        .at(-1);
      if (variant) engine?.setQuality(variant.id);
      onClose();
      return;
    }
    const again = row.state === 'playing';
    const source =
      row.state === 'otherSource'
        ? row.source
        : (useVideoPlayerStore.getState().sources.find((group) =>
            group.links.some((link) => link.current),
          )?.name ?? null);
    onAsked(
      again
        ? t('videoPlayer.qualityNextSource', { n: name })
        : t('videoPlayer.loadingSource', { n: [source, name].filter(Boolean).join(' · ') }),
    );
    useVideoPlayerStore.setState({ pickedSource: true });
    sendRequest(videoKey, 'quality', positionMs, { key: row.key });
    onClose();
  };

  return (
    <PlayerPanel title={t('videoPlayer.quality')} onClose={onClose}>
      <PanelOption
        label={t('videoPlayer.qualityAuto')}
        selected={auto}
        onSelect={() => {
          engine?.setQuality(-1);
          if (session) sendRequest(videoKey, 'quality', positionMs, { key: null });
          onClose();
        }}
      />
      {rows.length === 0 && (
        <p className="px-3 py-2 text-sm text-white/60">{t('videoPlayer.qualityWaiting')}</p>
      )}
      {rows.map((row) => (
        <PanelOption
          key={row.key}
          label={stepName(row.height, row.range, t)}
          hint={detail(row)}
          selected={row.state === 'playing' && !auto}
          disabled={row.state === 'missing' || (row.state === 'playing' && !session)}
          onSelect={() => pick(row)}
        />
      ))}
    </PlayerPanel>
  );
};
