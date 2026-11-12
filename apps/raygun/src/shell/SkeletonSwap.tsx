import { useState } from 'react';
import type { FC, ReactNode } from 'react';

import { picturesFade } from './imageMemory';

/**
 * `skeleton` while what it stands for loads (`ready` false), then `children` in its place, with
 * the skeleton fading out over them: its placeholders have the cards' shapes, so the cards show
 * through as it goes (a cross-fade, not a jump). Only where things may move; what's there from
 * the start shows at once. The fade is a CSS transition from its first style (styles.css
 * `.skeleton-leaving`); nothing runs per frame, and without the transition it's simply clear.
 */
export const SkeletonSwap: FC<{ ready: boolean; skeleton: ReactNode; children: ReactNode }> = ({
  ready,
  skeleton,
  children,
}) => {
  const [was, setWas] = useState(ready);
  const [leaving, setLeaving] = useState(false);
  if (ready !== was) {
    setWas(ready);
    setLeaving(ready && picturesFade());
  }
  return (
    <div className="relative">
      {ready ? children : skeleton}
      {ready && leaving && (
        <div
          aria-hidden
          data-testid="skeleton-leaving"
          className="skeleton-leaving"
          onTransitionEnd={(event) => {
            if (event.target === event.currentTarget) setLeaving(false);
          }}
        >
          {skeleton}
        </div>
      )}
    </div>
  );
};
