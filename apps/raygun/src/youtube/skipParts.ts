import type { SegmentCategory, SkipSegment } from '@raygun/bindings';

/**
 * SponsorBlock's parts in YouTube's own players in Raygun's window (trailers, YouTube links): the
 * same rules `raygun-app`'s `sponsorblock::LoadSkips` keeps for songs, for players Rust doesn't
 * follow. A part is jumped over as playback reaches it (NouTube's way, nonbili/NouTube,
 * AGPL-3.0), along with the parts that follow right on; Undo plays it, and it isn't jumped over
 * again in that video.
 */

/** Reports this close to a part's end don't skip it. */
const NEAR_END_MS = 500;
/** Parts this short aren't skipped. */
const SHORTEST_MS = 1_000;
/** Parts starting within this much of where a jump lands go in the same jump. */
const JOINED_MS = 1_000;
/** After a jump, reports from before it are let through for this long. */
const SETTLE_MS = 3_000;

/** SponsorBlock's colours for its kinds of parts. */
export const SEGMENT_COLORS: Record<SegmentCategory, string> = {
  sponsor: '#00d400',
  selfpromo: '#ffff00',
  interaction: '#cc00ff',
  intro: '#00ffff',
  outro: '#0202ed',
  preview: '#008fd6',
  musicOfftopic: '#ff9900',
  filler: '#7300ff',
};

export type PartSkip = { segment: SkipSegment; toMs: number };

/** A part's start and end, ms. */
export const spanOf = (part: SkipSegment) => ({
  startMs: part.startMs ?? 0,
  endMs: part.endMs ?? 0,
});

/** What the embed host page passes to Raygun's player script, to mark on YouTube's seek bar. */
export const partsCommand = (videoId: string, parts: SkipSegment[]) => ({
  type: 'segments',
  videoId,
  segments: parts.map((part) => ({ ...spanOf(part), category: part.category })),
});

export class PartSkipper {
  private undone = new Set<string>();
  private settling: { until: number; landed: number } | null = null;

  /** The part to jump over at `positionMs`, if one is reached; `now` tells late reports apart. */
  due(
    parts: SkipSegment[],
    positionMs: number,
    durationMs: number | null,
    now: number,
  ): PartSkip | null {
    if (this.settling) {
      if (now < this.settling.until && positionMs < this.settling.landed - NEAR_END_MS) {
        return null;
      }
      this.settling = null;
    }
    const skipped = parts
      .filter((part) => part.action === 'skip' && !this.undone.has(part.id))
      .map((part) => ({ part, ...spanOf(part) }))
      .filter(({ startMs, endMs }) => endMs - startMs >= SHORTEST_MS);
    const reached = skipped.find(
      ({ startMs, endMs }) => startMs <= positionMs && positionMs < endMs - NEAR_END_MS,
    );
    if (!reached) return null;
    let toMs = reached.endMs;
    for (;;) {
      const from = toMs;
      const further = skipped
        .filter(({ startMs, endMs }) => startMs <= from + JOINED_MS && endMs > from)
        .reduce<number | null>((end, { endMs }) => Math.max(end ?? endMs, endMs), null);
      if (further === null) break;
      toMs = further;
    }
    if (durationMs && durationMs > 0) toMs = Math.min(toMs, durationMs);
    this.settling = { until: now + SETTLE_MS, landed: toMs };
    return { segment: reached.part, toMs };
  }

  /** Plays `part` after all: not jumped over again. */
  undo(part: SkipSegment) {
    this.undone.add(part.id);
    this.settling = null;
  }
}
