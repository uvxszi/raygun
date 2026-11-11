/**
 * Subtitle files for the webview's video player (M46.4): SubRip, WebVTT and ASS/SSA, read into
 * cues the player draws itself (styled from Settings → Player → Subtitles, with a delay).
 */

export type Cue = { startMs: number; endMs: number; lines: string[] };

/** `00:01:02,345`, `01:02.345`, `0:01:02.34` (ASS centiseconds) → milliseconds. */
export const parseTimestamp = (text: string): number | null => {
  const match = /^\s*(?:(\d+):)?(\d{1,2}):(\d{1,2})(?:[.,](\d{1,3}))?\s*$/.exec(text);
  if (!match) return null;
  const [, hours, minutes, seconds, fraction = '0'] = match;
  const millis = Number(fraction.padEnd(3, '0'));
  return ((Number(hours ?? 0) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1000 + millis;
};

/** Markup the player doesn't style itself: `<i>`, `<font>`, `{\an8}`… */
const plain = (line: string) =>
  line
    .replace(/<[^>]*>/g, '')
    .replace(/\{\\[^}]*\}/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&nbsp;/g, ' ')
    .trim();

/** A byte order mark some subtitle files start with. */
const BOM = new RegExp(`^${String.fromCharCode(0xfeff)}`);

const blocks = (text: string) =>
  text
    .replace(BOM, '')
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/);

/** SubRip and WebVTT: blocks with a `start --> end` line, then the text. */
const parseTimed = (text: string): Cue[] => {
  const cues: Cue[] = [];
  for (const block of blocks(text)) {
    const lines = block.split('\n');
    const timing = lines.findIndex((line) => line.includes('-->'));
    if (timing < 0) continue;
    const [from = '', rest = ''] = lines[timing]!.split('-->');
    // WebVTT puts cue settings after the end time.
    const to = rest.trim().split(/\s+/)[0] ?? '';
    const startMs = parseTimestamp(from);
    const endMs = parseTimestamp(to);
    if (startMs === null || endMs === null || endMs <= startMs) continue;
    const body = lines
      .slice(timing + 1)
      .map(plain)
      .filter(Boolean);
    if (body.length > 0) cues.push({ startMs, endMs, lines: body });
  }
  return cues;
};

/** ASS/SSA: `Dialogue:` lines read with the `[Events]` section's `Format:` line. */
const parseAss = (text: string): Cue[] => {
  const cues: Cue[] = [];
  let fields = [
    'layer',
    'start',
    'end',
    'style',
    'name',
    'marginl',
    'marginr',
    'marginv',
    'effect',
    'text',
  ];
  for (const raw of text.replace(/\r\n?/g, '\n').split('\n')) {
    const line = raw.trim();
    if (/^format\s*:/i.test(line)) {
      fields = line
        .slice(line.indexOf(':') + 1)
        .split(',')
        .map((field) => field.trim().toLowerCase());
      continue;
    }
    if (!/^dialogue\s*:/i.test(line)) continue;
    const values = line.slice(line.indexOf(':') + 1).split(',');
    // The text is last and may hold commas.
    const textIndex = fields.indexOf('text');
    const parts = [...values.slice(0, textIndex), values.slice(textIndex).join(',')];
    const value = (name: string) => parts[fields.indexOf(name)] ?? '';
    const startMs = parseTimestamp(value('start'));
    const endMs = parseTimestamp(value('end'));
    if (startMs === null || endMs === null || endMs <= startMs) continue;
    const body = value('text')
      .split(/\\N|\\n/)
      .map(plain)
      .filter(Boolean);
    if (body.length > 0) cues.push({ startMs, endMs, lines: body });
  }
  return cues.sort((a, b) => a.startMs - b.startMs);
};

/** Reads a subtitle file; `format` (`srt`, `vtt`, `ass`…) is a hint, the content decides. */
export const parseSubtitles = (text: string, format?: string | null): Cue[] => {
  const ass =
    /^\s*\[script info\]/i.test(text.replace(BOM, '')) || /^(ass|ssa)$/i.test(format ?? '');
  if (ass && /dialogue\s*:/i.test(text)) return parseAss(text);
  return parseTimed(text).sort((a, b) => a.startMs - b.startMs);
};

/** The lines showing at `timeMs`. */
export const cuesAt = (cues: Cue[], timeMs: number): string[] =>
  cues.filter((cue) => cue.startMs <= timeMs && timeMs < cue.endMs).flatMap((cue) => cue.lines);

/**
 * The lines of the cue showing at `timeMs` in a track made as it plays (AI subtitles, a
 * translation): the last cue to start, until it ends. One shows at a time: a stretch heard again
 * (a window that met a line cut at the end of the one before) brings a cue over one already
 * there, and both would show, one under the other.
 */
export const latestCueAt = (cues: Cue[], timeMs: number): string[] => {
  let latest: Cue | null = null;
  for (const cue of cues) {
    if (cue.startMs <= timeMs && (!latest || cue.startMs >= latest.startMs)) latest = cue;
  }
  return latest && timeMs < latest.endMs ? latest.lines : [];
};
