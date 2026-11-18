import type { TFunction } from 'i18next';

import { formatTime } from '../player/formatTime';

import type { LinkFacts, QualityRange, SourceGroup, SourceLink } from './protocol';

/**
 * "2160p (4K) · HDR", "720p", "1080p (1920×816)": a row of the Quality menu or an option's
 * picture, by its tier, with the picture's own size when Rust names one (`exact`).
 */
export const stepName = (
  height: number,
  range: QualityRange,
  t: TFunction,
  exact?: string | null,
): string => {
  const name = exact
    ? `${height}p (${exact})`
    : height >= 4000
      ? t('videoPlayer.quality8k', { n: height })
      : height >= 2000
        ? t('videoPlayer.quality4k', { n: height })
        : `${height}p`;
  if (range === 'hdr') return `${name} · HDR`;
  if (range === 'dv') return `${name} · Dolby Vision`;
  return name;
};

/** "25 Mb/s", "800 kb/s". */
export const formatSpeed = (kbps: number): string =>
  kbps >= 1_000 ? `${(kbps / 1_000).toFixed(1).replace(/\.0$/, '')} Mb/s` : `${kbps} kb/s`;

/** "15.2 GB", "700 MB". */
export const formatSize = (bytes: number): string =>
  bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1e6))} MB`;

/** An option's name in the clean list: its picture, or its own short name when that's unknown. */
export const optionTitle = (link: SourceLink, t: TFunction): string => {
  const facts = link.facts;
  if (facts?.height) return stepName(facts.height, facts.range, t, facts.exact);
  const own = link.label.split(' · ')[0]?.trim() ?? '';
  return own.slice(0, 60) || t('watch.video');
};

/** "HEVC 10-bit · DD+ 5.1 · 15.2 GB · needs ~20 Mb/s · 25 Mb/s measured · WEB-DL · playing". */
export const optionDetail = (link: SourceLink, group: SourceGroup, t: TFunction): string | null => {
  const facts: Partial<LinkFacts> = link.facts ?? {};
  const codec =
    [facts.codec, facts.bitDepth && facts.bitDepth > 8 ? `${facts.bitDepth}-bit` : null]
      .filter(Boolean)
      .join(' ') || null;
  const measured = link.speedKbps ?? group.speedKbps ?? null;
  const status = link.current
    ? t('videoPlayer.sourcePlaying')
    : link.failed
      ? t('videoPlayer.failed')
      : link.worked
        ? t('videoPlayer.workedLastTime')
        : null;
  const seeds = link.torrent?.seeds ?? null;
  const parts = [
    codec,
    link.torrent
      ? seeds === null
        ? t('videoPlayer.torrent')
        : t('videoPlayer.torrentSeeds', { count: seeds, n: seeds })
      : null,
    facts.audio ?? null,
    facts.sizeBytes ? formatSize(facts.sizeBytes) : null,
    link.bitrateKbps ? t('videoPlayer.bitrateNeeds', { n: formatSpeed(link.bitrateKbps) }) : null,
    measured ? t('videoPlayer.speedMeasured', { n: formatSpeed(measured) }) : null,
    facts.release ?? null,
    status,
  ].filter((part): part is string => Boolean(part));
  return parts.length > 0 ? parts.join(' · ') : null;
};

const RANGE_ORDER: Record<QualityRange, number> = { dv: 2, hdr: 1, sdr: 0 };

/** A source's options, best picture first, then faster, then the higher bit rate. */
export const sortedOptions = (group: SourceGroup): { link: SourceLink; index: number }[] =>
  group.links
    .map((link, index) => ({ link, index }))
    .sort(
      (a, b) =>
        (b.link.facts?.height ?? -1) - (a.link.facts?.height ?? -1) ||
        RANGE_ORDER[b.link.facts?.range ?? 'sdr'] - RANGE_ORDER[a.link.facts?.range ?? 'sdr'] ||
        (b.link.speedKbps ?? -1) - (a.link.speedKbps ?? -1) ||
        (b.link.bitrateKbps ?? -1) - (a.link.bitrateKbps ?? -1) ||
        a.index - b.index,
    );

/** A row of a source's options: links of one tier and range, best first. */
export type OptionRow = { links: { link: SourceLink; index: number }[]; preferred: boolean };

/**
 * A source's options as Sources lists them: Rust's rows (`qualities`: each tier and range once,
 * the preferred resolution's first), else each link alone, best picture first.
 */
export const optionRows = (group: SourceGroup): OptionRow[] => {
  const fromRust = (group.qualities ?? [])
    .map((row) => ({
      preferred: row.preferred,
      links: row.links.flatMap((index) => {
        const link = group.links[index];
        return link ? [{ link, index }] : [];
      }),
    }))
    .filter((row) => row.links.length > 0);
  if (fromRust.length > 0) return fromRust;
  return sortedOptions(group).map((option) => ({ links: [option], preferred: false }));
};

/** "HEVC · 15.2 GB · ~20 Mb/s · WEB-DL", then its own name: one of a row's links, to pick from. */
export const alternativeDetail = (link: SourceLink, group: SourceGroup, t: TFunction): string =>
  [optionDetail(link, group, t), link.label.split('\n')[0]?.trim().slice(0, 80)]
    .filter(Boolean)
    .join(' · ');

/** Why a link isn't played by itself and how long it measured, for its line in Sources. */
export const leftOutText = (link: SourceLink, t: (key: string) => string): string | null => {
  const reason =
    link.leftOut === 'off'
      ? t('videoPlayer.leftOutOff')
      : link.leftOut === 'trailer'
        ? t('videoPlayer.leftOutTrailer')
        : link.leftOut === 'short'
          ? t('videoPlayer.leftOutShort')
          : null;
  const length = link.lengthMs ? formatTime(link.lengthMs) : null;
  return [reason, length].filter(Boolean).join(' · ') || null;
};
