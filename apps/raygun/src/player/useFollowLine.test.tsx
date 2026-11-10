import { render } from '@testing-library/react';
import { useRef } from 'react';
import type { FC } from 'react';

import { useFollowLine } from './useFollowLine';

const glides = vi.hoisted(() => ({ made: 0, moves: [] as number[] }));

vi.mock('./lyricsGlide', () => ({
  createGlide: () => {
    glides.made += 1;
    return { to: (top: number) => glides.moves.push(top), stop: () => {} };
  },
}));

/** Lines rendered in the same commit as their scroll box, as the phone's lyrics open. */
const Lyrics: FC<{ current: number }> = ({ current }) => {
  const box = useRef<HTMLDivElement>(null);
  return (
    <div ref={box}>
      <Lines container={box} current={current} />
    </div>
  );
};

const Lines: FC<{ container: React.RefObject<HTMLDivElement | null>; current: number }> = ({
  container,
  current,
}) => {
  const { lineRef, listRef } = useFollowLine(container, current, 'song');
  return (
    <ol ref={listRef}>
      {[0, 1, 2].map((index) => (
        <li key={index}>
          <button type="button" ref={index === current ? lineRef : undefined}>
            {index}
          </button>
        </li>
      ))}
    </ol>
  );
};

describe('following the line', () => {
  beforeEach(() => {
    glides.made = 0;
    glides.moves = [];
  });

  it('follows lines that show together with their box', () => {
    const { rerender } = render(<Lyrics current={0} />);
    // Placed once the box has its ref, then moved on with each line.
    expect(glides.made).toBe(1);
    const placed = glides.moves.length;
    expect(placed).toBeGreaterThan(0);
    rerender(<Lyrics current={1} />);
    rerender(<Lyrics current={2} />);
    expect(glides.moves.length).toBe(placed + 2);
    expect(glides.made).toBe(1);
  });
});
