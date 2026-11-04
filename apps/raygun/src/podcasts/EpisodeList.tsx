import type { PodcastEpisode } from '@raygun/bindings';
import { useEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';

import { episodeReveal, useRevealIndex } from '../music/reveal';

/** Rows drawn at first, and added each time the list's end comes near. */
const CHUNK = 60;

const episodeName = (episode: PodcastEpisode) => episodeReveal(episode.id);

/**
 * A show's episodes or a book's chapters (M46.48), drawn a chunk at a time as the list's end
 * comes near, so a feed of two thousand episodes opens at once on a phone.
 */
export const EpisodeList: FC<{
  episodes: PodcastEpisode[];
  children: (episode: PodcastEpisode) => ReactNode;
}> = ({ episodes, children }) => {
  const watch = typeof IntersectionObserver !== 'undefined';
  const [shown, setShown] = useState(CHUNK);
  // "Playing from" asked for an episode further down: draw up to it.
  const revealAt = useRevealIndex(episodes, episodeName);
  if (revealAt >= shown) setShown(revealAt + CHUNK);
  const end = useRef<HTMLLIElement>(null);
  const more = watch && shown < episodes.length;
  useEffect(() => {
    const element = end.current;
    if (!more || !element) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) setShown((count) => count + CHUNK);
      },
      { rootMargin: '800px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [more, shown]);
  const visible = watch ? episodes.slice(0, shown) : episodes;
  return (
    <ul className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3">
      {visible.map(children)}
      {more && <li ref={end} aria-hidden className="h-px" />}
    </ul>
  );
};
