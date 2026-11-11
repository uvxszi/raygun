import type { TFunction } from 'i18next';

import type { LinkFacts, QualityRange, SourceGroup, SourceLink } from './protocol';

/** "2160p (4K) · HDR", "720p": a step of the Quality menu or an option's picture. */
export const stepName = (height: number, range: QualityRange, t: TFunction): string => {
  const name =
    height >= 4000
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
  if (facts?.height) return stepName(facts.height, facts.range, t);
  const own = link.label.split(' · ')[0]?.trim() ?? '';
  return own.slice(0, 60) || t('watch.video');
};

/** "HEVC 10-bit · DD+ 5.1 · 15.2 GB · needs ~20 Mb/s · 25 Mb/s measured · WEB-DL · playing". */
export const optionDetail = (
  link: SourceLink,
  group: SourceGroup,
  t: TFunction,
): string | null => {
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
  const parts = [
    codec,
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
