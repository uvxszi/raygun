import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { MediaEngine } from './mediaEngine';
import { PanelHeading, PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import type { QualityRow } from './protocol';
import { optionTitle, stepName } from './qualityText';
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
 * The Quality button's panel: the tiers the stream and the sources have, largest first, as Rust
 * lists them (`qualities`, `video_quality::rows`), each with the picture's own size when it
 * isn't the tier's plain one ("1080p (1920×816)"). A row the stream playing has (an HLS variant)
 * switches here; one the source playing has as another link, or only another source has, is
 * asked of Rust, which plays it from where the video is. Tapping the row that plays again tries
 * the next source with it. Automatic is ticked while nothing was picked here; the row playing is
 * always set apart, and ticked too when it was picked.
 */
export const QualityPanel: FC<Props> = ({ videoKey, positionMs, engine, onClose, onAsked }) => {
  const { t } = useTranslation();
  const rows = useVideoPlayerStore((state) => state.qualities);
  const wanted = useVideoPlayerStore((state) => state.qualityWanted);
  const session = useVideoPlayerStore((state) => state.session);
  const variants = engine?.variants() ?? [];
  const auto = (engine?.quality() ?? -1) === -1 && wanted === null;

  const detail = (row: QualityRow): string | null => {
    switch (row.state) {
      case 'playing':
        return [t('videoPlayer.qualityPlaying'), row.codec].filter(Boolean).join(' · ');
      case 'otherSource':
        return t('videoPlayer.qualityElsewhere', { n: row.source });
      default:
        return row.codec;
    }
  };

  // The preferred resolution's options from every source, before the tiers (a session's).
  const preferred = useVideoPlayerStore((state) => state.preferredOptions);
  const sources = useVideoPlayerStore((state) => state.sources);
  const preferredShown = session
    ? (preferred?.options ?? []).flatMap(({ group, link }) => {
        const source = sources[group];
        const option = source?.links[link];
        return source && option ? [{ group, link, source, option }] : [];
      })
    : [];
  const choosePreferred = (group: number, link: number, name: string) => {
    if (sources[group]?.links[link]?.current) {
      onClose();
      return;
    }
    onAsked(t('videoPlayer.loadingSource', { n: name }));
    useVideoPlayerStore.setState({ pickedSource: true });
    sendRequest(videoKey, 'source', positionMs, { group, link });
    onClose();
  };

  const pick = (row: QualityRow) => {
    const name = stepName(row.height, row.range, t, row.exact);
    if (row.state === 'variant') {
      // The best of the variants this row holds, by their own sides as reported.
      const sizes = row.sizes ?? [];
      const variant = variants
        .filter(
          (candidate) =>
            candidate.range === row.range &&
            sizes.some(
              (size) => size.width === candidate.width && size.height === candidate.height,
            ),
        )
        .at(-1);
      if (variant) engine?.setQuality(variant.id);
      onClose();
      return;
    }
    if (row.state === 'playing' && auto) {
      // Automatic and this one plays: it's held as it is, nothing loads again; a session keeps
      // the step (`keep`) so no later source takes over.
      engine?.keepQuality();
      if (session) sendRequest(videoKey, 'quality', positionMs, { key: row.key, keep: true });
      onClose();
      return;
    }
    const again = row.state === 'playing';
    const source =
      row.state === 'otherSource'
        ? row.source
        : (useVideoPlayerStore
            .getState()
            .sources.find((group) => group.links.some((link) => link.current))?.name ?? null);
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
      {preferredShown.length > 0 && (
        <div role="group" aria-label={t('videoPlayer.preferredOptions')} className="flex flex-col">
          <PanelHeading>
            {t('videoPlayer.preferredOptions')} · {preferred?.height}p
          </PanelHeading>
          {preferredShown.map(({ group, link, source, option }) => (
            <PanelOption
              key={`preferred-${group}-${link}`}
              label={`${optionTitle(option, t)} · ${source.name}`}
              hint={option.current ? t('videoPlayer.qualityPlaying') : null}
              selected={option.current && !auto}
              current={option.current}
              onSelect={() => choosePreferred(group, link, source.name)}
            />
          ))}
        </div>
      )}
      {preferredShown.length > 0 && <PanelHeading>{t('videoPlayer.allQualities')}</PanelHeading>}
      {rows.length === 0 && (
        <p className="px-3 py-2 text-sm text-white/60">{t('videoPlayer.qualityWaiting')}</p>
      )}
      {rows.map((row) => (
        <PanelOption
          key={row.key}
          label={stepName(row.height, row.range, t, row.exact)}
          hint={detail(row)}
          selected={row.state === 'playing' && !auto}
          current={row.state === 'playing'}
          disabled={row.state === 'playing' && !session && !auto}
          onSelect={() => pick(row)}
        />
      ))}
    </PlayerPanel>
  );
};
