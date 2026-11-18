import type { Cue } from './subtitles';

/**
 * Smart Subtitles' "Join lines close together": cues that overlap, or follow each other within
 * a moment while one of them is short (lines changing quickly), show together as one cue (their
 * lines stacked, each once) instead of flickering. A joined cue stays within a few seconds and
 * a few lines; past that, cues show as they are. The same rules as Android's `joinClose`
 * (`PlayerSubtitleJoin.kt`).
 */

/** Cues this close (ms) join when one of them is short. */
export const JOIN_GAP_MS = 250;
/** A cue shorter than this (ms) is a quick change. */
export const SHORT_CUE_MS = 1200;
/** A joined cue lasts this long at most (ms). */
export const MAX_JOINED_MS = 6000;
/** A joined cue holds this many lines at most. */
export const MAX_JOINED_LINES = 4;

/** Lines in order, each once (by their words, spaces trimmed). */
const unique = (lines: readonly string[]) => {
  const seen = new Set<string>();
  return lines.filter((line) => {
    const key = line.trim();
    if (!key || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

/** Whether `next` joins a run ending at `end` whose last cue is `last`. */
export const joinsRun = (end: number, last: Cue, next: Cue): boolean => {
  if (next.startMs < end) return true;
  const quick =
    last.endMs - last.startMs < SHORT_CUE_MS || next.endMs - next.startMs < SHORT_CUE_MS;
  return next.startMs - end < JOIN_GAP_MS && quick;
};

/** `cues` with those close together joined (see the module's words); sorted by start. */
export const joinCloseCues = (cues: readonly Cue[]): Cue[] => {
  const sorted = [...cues].sort((a, b) => a.startMs - b.startMs || a.endMs - b.endMs);
  const out: Cue[] = [];
  let run: Cue[] = [];
  let runStart = 0;
  let runEnd = 0;
  let runLines: string[] = [];
  const flush = () => {
    if (run.length === 1) out.push(run[0]!);
    else if (run.length > 1) out.push({ startMs: runStart, endMs: runEnd, lines: runLines });
    run = [];
  };
  for (const cue of sorted) {
    const last = run.at(-1);
    if (last) {
      const end = Math.max(runEnd, cue.endMs);
      const lines = unique([...runLines, ...cue.lines]);
      if (
        joinsRun(runEnd, last, cue) &&
        end - runStart <= MAX_JOINED_MS &&
        lines.length <= MAX_JOINED_LINES
      ) {
        run.push(cue);
        runEnd = end;
        runLines = lines;
        continue;
      }
      flush();
    }
    run = [cue];
    runStart = cue.startMs;
    runEnd = cue.endMs;
    runLines = unique(cue.lines);
  }
  flush();
  return out;
};
