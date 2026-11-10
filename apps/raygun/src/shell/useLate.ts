import { useEffect, useState } from 'react';

/**
 * Whether `on` has lasted `ms`: a spinner shows only for an answer that's slow, never as a flash
 * before one that comes from what the app kept.
 */
export const useLate = (on: boolean, ms = 400) => {
  const [late, setLate] = useState(false);
  const [was, setWas] = useState(on);
  if (was !== on) {
    setWas(on);
    if (!on) setLate(false);
  }
  useEffect(() => {
    if (!on) return;
    const timer = setTimeout(() => setLate(true), ms);
    return () => clearTimeout(timer);
  }, [on, ms]);
  return on && late;
};
