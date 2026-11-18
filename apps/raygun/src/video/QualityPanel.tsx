import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { MediaEngine } from './mediaEngine';
import { PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import type { LinkPlace, QualityRow } from './protocol';
import { alternativeDetail, optionTitle, stepName } from './qualityText';
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
 * The Quality button's panel: the tiers the stream and the sources have, as Rust lists them
 * (`qualities`, `video_quality::rows`): the preferred resolution's row first with a small
 * "Preferred" tag, then largest first, each tier and range once, with the picture's own size when
 * it isn't the tier's plain one ("1080p (1920×816)"). A row the stream playing has (an HLS
 * variant) switches here; one the source playing has as another link, or only another source
 * has, is asked of Rust, which plays its best link from where the video is. The row picked,
 * tapped again, lists every link with that tier (`alternatives`) to pick one. Automatic is ticked
 * while nothing was picked here or in Sources; the row playing is always set apart, and ticked too when it was
 * picked.
 */
export const QualityPanel: FC<Props> = ({ videoKey, positionMs, engine, onClose, onAsked }) => {
  const { t } = useTranslation();
  const rows = useVideoPlayerStore((state) => state.qualities);
  const wanted = useVideoPlayerStore((state) => state.qualityWanted);
  const handPicked = useVideoPlayerStore((state) => state.qualityHandPicked);
  const session = useVideoPlayerStore((state) => state.session);
  const sources = useVideoPlayerStore((state) => state.sources);
  const variants = engine?.variants() ?? [];
  // A link picked in Sources holds too: Automatic isn't what plays until it's chosen again.
  const auto = (engine?.quality() ?? -1) === -1 && wanted === null && !handPicked;
  /** The row whose links show (its key). */
  const [listed, setListed] = useState<string | null>(null);

  const detail = (row: QualityRow): string | null => {
    const more = (row.alternatives?.length ?? 0) - 1;
    const words =
      row.state === 'playing'
        ? [t('videoPlayer.qualityPlaying'), row.codec]
        : row.state === 'otherSource'
          ? [t('videoPlayer.qualityElsewhere', { n: row.source })]
          : [row.codec];
    if (row.state === 'playing' && !auto && more > 0) {
      words.push(t('videoPlayer.otherCopiesPick', { count: more, n: more }));
    }
    return words.filter(Boolean).join(' · ') || null;
  };

  /** One of a row's links, picked from its list: it plays from where the video is. */
  const chooseLink = ({ group, link }: LinkPlace) => {
    const source = sources[group];
    if (!source || source.links[link]?.current) {
      onClose();
      return;
    }
    onAsked(t('videoPlayer.loadingSource', { n: source.name }));
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
    if (row.state === 'playing') {
      // Picked, and tapped again: its links, to choose one.
      if ((row.alternatives?.length ?? 0) > 1) setListed(listed === row.key ? null : row.key);
      else onClose();
      return;
    }
    const source =
      row.state === 'otherSource'
        ? row.source
        : (useVideoPlayerStore
            .getState()
            .sources.find((group) => group.links.some((link) => link.current))?.name ?? null);
    onAsked(t('videoPlayer.loadingSource', { n: [source, name].filter(Boolean).join(' · ') }));
    useVideoPlayerStore.setState({ pickedSource: true });
    sendRequest(videoKey, 'quality', positionMs, { key: row.key });
    onClose();
  };

  const linkList = (row: QualityRow) => (
    <div
      role="menu"
      aria-label={t('videoPlayer.linksWith', { n: stepName(row.height, row.range, t) })}
      className="ml-4 flex flex-col border-l border-white/15 pl-1"
    >
      {(row.alternatives ?? []).flatMap((place) => {
        const source = sources[place.group];
        const link = source?.links[place.link];
        if (!source || !link) return [];
        return [
          <PanelOption
            key={`${place.group}-${place.link}`}
            label={`${optionTitle(link, t)} · ${source.name}`}
            hint={alternativeDetail(link, source, t) || null}
            selected={link.current}
            current={link.current}
            onSelect={() => chooseLink(place)}
          />,
        ];
      })}
    </div>
  );

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
      {rows.map((row) => {
        const several = row.state === 'playing' && !auto && (row.alternatives?.length ?? 0) > 1;
        return (
          <div key={row.key} className="flex flex-col">
            <PanelOption
              label={stepName(row.height, row.range, t, row.exact)}
              badge={row.preferred ? t('videoPlayer.preferredOptions') : null}
              hint={detail(row)}
              selected={row.state === 'playing' && !auto}
              current={row.state === 'playing'}
              disabled={row.state === 'playing' && !session && !auto}
              expanded={several ? listed === row.key : undefined}
              onSelect={() => pick(row)}
            />
            {several && listed === row.key && linkList(row)}
          </div>
        );
      })}
    </PlayerPanel>
  );
};
