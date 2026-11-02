import { useEffect, useState } from 'react';
import type { FC } from 'react';

/** How long the pointer rests on a button before its name shows. */
const DELAY_MS = 450;

type Tip = { text: string; x: number; y: number; above: boolean };

/**
 * Names icon buttons on hover (M46.7): any button or link whose label isn't already its visible
 * text shows that label in a small tip after a moment, for the mouse only (touch has long
 * presses). Elements with a `title` keep the browser's own tip.
 */
export const HoverLabel: FC = () => {
  const [tip, setTip] = useState<Tip | null>(null);
  useEffect(() => {
    let timer: number | undefined;
    let current: Element | null = null;
    const hide = () => {
      window.clearTimeout(timer);
      current = null;
      setTip(null);
    };
    const over = (event: PointerEvent) => {
      if (event.pointerType !== 'mouse') return;
      const element =
        event.target instanceof Element
          ? event.target.closest('button[aria-label], a[aria-label], [role="switch"][aria-label]')
          : null;
      if (element === current) return;
      window.clearTimeout(timer);
      setTip(null);
      current = element;
      if (!element || element.hasAttribute('title')) return;
      const label = element.getAttribute('aria-label')?.trim() ?? '';
      const text = element.textContent?.trim() ?? '';
      if (!label || text === label) return;
      timer = window.setTimeout(() => {
        const box = element.getBoundingClientRect();
        const above = box.bottom + 40 > window.innerHeight;
        setTip({
          text: label,
          x: Math.min(Math.max(box.left + box.width / 2, 60), window.innerWidth - 60),
          y: above ? box.top - 6 : box.bottom + 6,
          above,
        });
      }, DELAY_MS);
    };
    document.addEventListener('pointerover', over);
    document.addEventListener('pointerdown', hide, true);
    document.addEventListener('keydown', hide, true);
    document.addEventListener('scroll', hide, true);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('pointerover', over);
      document.removeEventListener('pointerdown', hide, true);
      document.removeEventListener('keydown', hide, true);
      document.removeEventListener('scroll', hide, true);
    };
  }, []);
  if (!tip) return null;
  return (
    <span
      role="tooltip"
      className="bg-foreground text-background pointer-events-none fixed z-[90] max-w-xs -translate-x-1/2 rounded-md px-2 py-1 text-xs font-medium whitespace-nowrap shadow-lg"
      style={{
        left: tip.x,
        top: tip.y,
        transform: `translate(-50%, ${tip.above ? '-100%' : '0'})`,
      }}
    >
      {tip.text}
    </span>
  );
};
