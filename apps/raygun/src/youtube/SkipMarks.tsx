import type { SkipSegment } from '@raygun/bindings';
import type { FC } from 'react';

import { SEGMENT_COLORS, spanOf } from './skipParts';

const share = (ms: number, durationMs: number) => Math.max(0, Math.min(1, ms / durationMs));

/**
 * SponsorBlock's parts along a seek bar, in SponsorBlock's colours: solid for parts that are
 * skipped, fainter for those only shown. Laid over the slider's track; it takes no presses.
 */
export const SkipMarks: FC<{ segments: SkipSegment[]; durationMs: number }> = ({
  segments,
  durationMs,
}) => {
  if (segments.length === 0 || durationMs <= 0) return null;
  return (
    <span
      aria-hidden
      data-testid="skip-marks"
      className="pointer-events-none absolute inset-x-0 top-1/2 h-1 -translate-y-1/2"
    >
      {segments.map((segment) => {
        const { startMs, endMs } = spanOf(segment);
        const left = share(startMs, durationMs);
        const width = Math.max(0.004, share(endMs, durationMs) - left);
        return (
          <span
            key={segment.id}
            data-category={segment.category}
            className="absolute inset-y-0 rounded-full"
            style={{
              left: `${left * 100}%`,
              width: `${width * 100}%`,
              background: SEGMENT_COLORS[segment.category],
              opacity: segment.action === 'skip' ? 0.9 : 0.5,
            }}
          />
        );
      })}
    </span>
  );
};
