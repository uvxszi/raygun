import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';

import { HOLD_MS } from '../select/scopeContext';
import { controlOf, keepOnScreen, KEPT_TITLE, labelFor, placeLabel } from './labelRules';
import type { LabelPlace } from './labelRules';
import { holdClaimed } from './holdClaims';
import { safeInsets } from './safeArea';
import { bottomBars } from './toastStore';

/** How long the mouse rests on a control before its name shows, ms. */
export const HOVER_MS = 500;
/** How long a finger holds a control before its name shows, ms: the app's hold (picking). */
export const LABEL_HOLD_MS = HOLD_MS;
/** A held finger moving this far (px) is a scroll or a drag: no name. */
const SLOP = 10;
/** A held control's name stays this long (ms) after the finger lifts. */
export const LINGER_MS = 1500;
/** After a hold showed a name, how long (ms) the click its release may make is waited for. */
const CLICK_MS = 400;
/** Marks a label given for the hover from a title, taken off again after it. */
const NAMED = 'data-label-named';

type Shown = { text: string; place: LabelPlace; id: number };

type Press = {
  down: PointerEvent;
  control: HTMLElement;
  timer: number;
  /** The name is showing for it: its click is no tap. */
  shown: boolean;
};

const screenNow = () => ({
  width: window.innerWidth,
  height: window.innerHeight,
  safe: safeInsets(),
});

/** While the mouse is on it, the control's title rests: the label shows instead of the tip. */
const restTitle = (control: HTMLElement) => {
  const title = control.getAttribute('title');
  if (title === null) return;
  control.setAttribute(KEPT_TITLE, title);
  control.removeAttribute('title');
  if (!control.hasAttribute('aria-label')) {
    control.setAttribute('aria-label', title);
    control.setAttribute(NAMED, '');
  }
};

const giveTitleBack = (control: HTMLElement) => {
  const title = control.getAttribute(KEPT_TITLE);
  if (title === null) return;
  control.removeAttribute(KEPT_TITLE);
  if (!control.hasAttribute('title')) control.setAttribute('title', title);
  if (control.hasAttribute(NAMED)) {
    control.removeAttribute(NAMED);
    if (control.getAttribute('aria-label') === title) control.removeAttribute('aria-label');
  }
};

/**
 * Names icon buttons, app-wide (every button, link, tab or switch whose name, its `aria-label` or
 * `title`, isn't the words it shows). The mouse resting on one shows its name in a small bubble
 * after a moment (instead of the browser's own tip); on a touch screen holding one shows it, above
 * the finger (else below it, else as a toast at the bottom), and that release doesn't press it.
 * Holds a control's own code acts on (shell/holdClaims: covers, move buttons, picking, quick
 * access's menu) keep doing that and show no name.
 */
export const ControlLabels: FC = () => {
  const [shown, setShown] = useState<Shown | null>(null);
  const bubble = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    let count = 0;
    let hovered: HTMLElement | null = null;
    let hoverTimer: number | undefined;
    let press: Press | null = null;
    let lingerTimer: number | undefined;
    // The control whose name shows.
    let labelled: HTMLElement | null = null;
    // The click a hold's release may still make: swallowed, the hold was for the name.
    let swallow: { control: HTMLElement; until: number } | null = null;

    const show = (control: HTMLElement, text: string, by: 'pointer' | 'touch', y?: number) => {
      window.clearTimeout(lingerTimer);
      labelled = control;
      const place = placeLabel(control.getBoundingClientRect(), screenNow(), by, y);
      setShown({ text, place, id: ++count });
    };
    const hide = () => {
      window.clearTimeout(lingerTimer);
      if (!labelled) return;
      labelled = null;
      setShown(null);
    };
    const leave = () => {
      window.clearTimeout(hoverTimer);
      if (hovered) giveTitleBack(hovered);
      hovered = null;
    };
    const endPress = () => {
      if (press) window.clearTimeout(press.timer);
      press = null;
    };

    const over = (event: PointerEvent) => {
      if (event.pointerType !== 'mouse') return;
      const control = controlOf(event.target);
      if (control === hovered) return;
      leave();
      hide();
      const text = control && labelFor(control, 'pointer');
      if (!control || !text) return;
      hovered = control;
      restTitle(control);
      hoverTimer = window.setTimeout(() => {
        if (hovered === control && control.isConnected) show(control, text, 'pointer');
      }, HOVER_MS);
    };
    const out = (event: PointerEvent) => {
      // Off the window altogether.
      if (event.pointerType === 'mouse' && event.relatedTarget === null) {
        leave();
        hide();
      }
    };

    const down = (event: PointerEvent) => {
      leave();
      endPress();
      hide();
      swallow = null;
      if (event.pointerType === 'mouse') return;
      const control = controlOf(event.target);
      const text = control && labelFor(control, 'touch');
      if (!control || !text) return;
      const timer = window.setTimeout(() => {
        if (press?.down !== event) return;
        // Its own hold acts on this press (a cover, a move button, picking): no name.
        if (holdClaimed(event) || !control.isConnected) {
          press = null;
          return;
        }
        press.shown = true;
        navigator.vibrate?.(10);
        show(control, text, 'touch', event.clientY);
      }, LABEL_HOLD_MS);
      press = { down: event, control, timer, shown: false };
    };
    const move = (event: PointerEvent) => {
      if (!press || event.pointerId !== press.down.pointerId) return;
      const moved = Math.hypot(
        event.clientX - press.down.clientX,
        event.clientY - press.down.clientY,
      );
      if (moved <= SLOP) return;
      // A scroll or a drag after all: the name goes, and nothing is swallowed.
      const named = press.shown;
      endPress();
      if (named) hide();
    };
    const lift = (event: PointerEvent) => {
      if (!press || event.pointerId !== press.down.pointerId) return;
      const { shown: named, control } = press;
      endPress();
      if (!named) return;
      if (event.type === 'pointerup') swallow = { control, until: event.timeStamp + CLICK_MS };
      lingerTimer = window.setTimeout(hide, LINGER_MS);
    };
    const click = (event: MouseEvent) => {
      const kept = swallow;
      if (!kept) return;
      swallow = null;
      if (event.timeStamp > kept.until) return;
      if (!(event.target instanceof Node) || !kept.control.contains(event.target)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    // While a finger holds a named control, the webview's own long press does nothing: no menu,
    // and its link or picture doesn't start dragging. A hold claimed by the control's own code
    // is that code's to answer.
    const held = (event: Event) => {
      if (!press || holdClaimed(press.down)) return;
      if (event.target instanceof Node && press.control.contains(event.target)) {
        event.preventDefault();
      }
    };
    const quiet = () => {
      leave();
      endPress();
      hide();
    };
    // A scroll moves the controls in it away from their names; others (lyrics gliding beside
    // the button) leave them be.
    const scrolled = (event: Event) => {
      const area = event.target;
      if (!(area instanceof Node)) return;
      const moves = (control: HTMLElement | null | undefined) =>
        control != null && (area === document || area.contains(control));
      if (moves(hovered) || moves(press?.control) || moves(labelled)) quiet();
    };

    document.addEventListener('pointerover', over, true);
    document.addEventListener('pointerout', out, true);
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('pointermove', move, true);
    document.addEventListener('pointerup', lift, true);
    document.addEventListener('pointercancel', lift, true);
    document.addEventListener('click', click, true);
    document.addEventListener('contextmenu', held, true);
    document.addEventListener('dragstart', held, true);
    document.addEventListener('keydown', quiet, true);
    document.addEventListener('scroll', scrolled, true);
    window.addEventListener('blur', quiet);
    return () => {
      leave();
      endPress();
      window.clearTimeout(lingerTimer);
      document.removeEventListener('pointerover', over, true);
      document.removeEventListener('pointerout', out, true);
      document.removeEventListener('pointerdown', down, true);
      document.removeEventListener('pointermove', move, true);
      document.removeEventListener('pointerup', lift, true);
      document.removeEventListener('pointercancel', lift, true);
      document.removeEventListener('click', click, true);
      document.removeEventListener('contextmenu', held, true);
      document.removeEventListener('dragstart', held, true);
      document.removeEventListener('keydown', quiet, true);
      document.removeEventListener('scroll', scrolled, true);
      window.removeEventListener('blur', quiet);
    };
  }, []);

  // Once its width is known, a bubble near an edge moves in to stay on screen.
  useLayoutEffect(() => {
    const element = bubble.current;
    if (!element || shown?.place.kind !== 'bubble') return;
    const box = element.getBoundingClientRect();
    const shift = keepOnScreen(box.left, box.right, screenNow());
    if (shift !== 0) element.style.marginLeft = `${shift}px`;
  }, [shown]);

  if (!shown) return null;
  const { place } = shown;
  if (place.kind === 'toast') {
    const bottom = bottomBars();
    return (
      <div
        className={`pointer-events-none fixed inset-x-0 z-[100] flex justify-center pr-[max(1rem,var(--safe-right))] pl-[max(1rem,var(--safe-left))] ${bottom > 0 ? 'pb-3' : 'pb-[calc(0.75rem+var(--safe-bottom))]'}`}
        style={{ bottom }}
      >
        <span
          key={shown.id}
          role="tooltip"
          data-place="toast"
          className="bg-foreground text-background motion-fade max-w-md rounded-xl px-4 py-2 text-sm font-medium shadow-xl shadow-black/40"
        >
          {shown.text}
        </span>
      </div>
    );
  }
  return (
    <span
      key={shown.id}
      ref={bubble}
      role="tooltip"
      data-place="bubble"
      className="bg-foreground text-background motion-fade pointer-events-none fixed z-[100] w-max max-w-[min(20rem,calc(100vw-1rem))] rounded-md px-2 py-1 text-center text-xs font-medium shadow-lg"
      style={{
        left: place.x,
        top: place.y,
        transform: `translate(-50%, ${place.above ? '-100%' : '0'})`,
      }}
    >
      {shown.text}
    </span>
  );
};
