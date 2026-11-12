import type { AnimationEvent } from 'react';

/** The list's own animation (styles.css), which lasts as long as its first screenful's. */
const HOLD = 'raygun-appear-hold';

/**
 * Marks a list whose first screenful has come in: rows drawn later (scrolled to, a page more, a
 * refresh) just show. One handler for the whole list, none per row.
 */
const appeared = (event: AnimationEvent<HTMLElement>) => {
  if (event.target === event.currentTarget && event.animationName === HOLD) {
    event.currentTarget.dataset.appeared = '';
  }
};

/**
 * Props for a list or grid of what a source answered, drawn once it came: its first screenful
 * fades in one after another (`rise`: cards rise a little as they do), once, only where things
 * may move (styles.css `[data-appear]`). Only opacity and transform change, so nothing is laid
 * out again and nothing runs per row.
 */
export const appearItems = (rise = false) => ({
  'data-appear': rise ? 'rise' : '',
  onAnimationEnd: appeared,
});
