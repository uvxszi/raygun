import { useEffect, useRef, useState } from 'react';

/**
 * Whether an element has come near the window yet (M46.7): rows load when they're about to be
 * seen, not all at once. Once true it stays true. Without IntersectionObserver (tests) it's true.
 */
export const useInView = <T extends Element>(margin = '400px') => {
  const ref = useRef<T>(null);
  const [seen, setSeen] = useState(() => typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    const element = ref.current;
    if (seen || !element) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setSeen(true);
          observer.disconnect();
        }
      },
      { rootMargin: margin },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [seen, margin]);
  return [ref, seen] as const;
};
