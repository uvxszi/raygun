import { useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties, FC } from 'react';

/**
 * Text that slides to show all of itself when it doesn't fit (M46.7): long song titles in the
 * player bar, the full player and the side panel. It rests at each end; with reduced motion it's
 * cut with an ellipsis instead (styles.css).
 */
export const Marquee: FC<{ text: string; className?: string; testId?: string }> = ({
  text,
  className = '',
  testId,
}) => {
  const box = useRef<HTMLSpanElement>(null);
  const inner = useRef<HTMLSpanElement>(null);
  const [overflow, setOverflow] = useState(0);
  useLayoutEffect(() => {
    const measure = () => {
      const outer = box.current;
      const content = inner.current;
      if (outer && content) setOverflow(Math.max(0, content.scrollWidth - outer.clientWidth));
    };
    measure();
    if (typeof ResizeObserver === 'undefined' || !box.current) return;
    const observer = new ResizeObserver(measure);
    observer.observe(box.current);
    return () => observer.disconnect();
  }, [text]);
  const style: CSSProperties | undefined =
    overflow > 0
      ? ({
          '--marquee-shift': `-${overflow}px`,
          // About 30 px a second plus the rests, so a long title can be read as it goes by.
          '--marquee-time': `${Math.max(4, overflow / 30 + 2).toFixed(1)}s`,
        } as CSSProperties)
      : undefined;
  return (
    <span
      ref={box}
      data-testid={testId}
      title={overflow > 0 ? text : undefined}
      className={`block overflow-hidden whitespace-nowrap ${className}`}
    >
      <span
        ref={inner}
        className={overflow > 0 ? 'marquee-run inline-block' : 'inline-block'}
        style={style}
      >
        {text}
      </span>
    </span>
  );
};
