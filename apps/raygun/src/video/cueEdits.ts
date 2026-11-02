import type { Cue } from './subtitles';

/** One subtitle line changed by hand (M46.12): the cue that started at `at`, now these. */
export type CueEdit = { at: number; startMs: number; endMs: number; text: string };

/** Edits per episode and track: `<progress key>|<label>|<language>`. */
export type CueEdits = Record<string, CueEdit[]>;

/** Where an episode's track keeps its edits; the label and language survive reopening. */
export const editKey = (episode: string, label: string, language: string | null) =>
  `${episode}|${label}|${language ?? ''}`;

/** The cues with the person's edits in place, in time order. */
export const applyCueEdits = (cues: Cue[], edits: CueEdit[] | undefined): Cue[] => {
  if (!edits || edits.length === 0) return cues;
  const byStart = new Map(edits.map((edit) => [edit.at, edit]));
  return cues
    .map((cue) => {
      const edit = byStart.get(cue.startMs);
      return edit
        ? { startMs: edit.startMs, endMs: edit.endMs, lines: edit.text.split('\n') }
        : cue;
    })
    .sort((left, right) => left.startMs - right.startMs);
};

/** Replaces (or adds) the edit for the cue that started at `at`; `null` undoes it. */
export const withCueEdit = (
  edits: CueEdit[] | undefined,
  at: number,
  edit: Omit<CueEdit, 'at'> | null,
): CueEdit[] => {
  const others = (edits ?? []).filter((each) => each.at !== at);
  return edit ? [...others, { at, ...edit }] : others;
};
