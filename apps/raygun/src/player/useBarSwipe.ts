import type { QueueItem } from '@raygun/bindings';
import { animate, useMotionValue } from 'motion/react';
import type { AnimationPlaybackControls } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';

import { comeIn, swipeAction, throwDuration, velocityTracker } from './swipe';
import type { SwipeAction } from './swipe';

/** A press moving this far (px) picks the axis it follows. */
const AXIS_SLOP = 8;
/** How much of the finger's way up or down the bar follows (it springs back). */
const ELASTIC_UP = 0.25;
const ELASTIC_DOWN = 0.5;
/** Where things settle: the bar after an up or down drag, the content as it comes back in. */
const SETTLE = { type: 'spring', stiffness: 420, damping: 34 } as const;
const COME_IN = { type: 'spring', stiffness: 360, damping: 30 } as const;
/** Reduced motion's fades (s). */
const FADE = 0.1;
/** The song that came in shows this long (ms) at most while the player hasn't got to it. */
const LANDED_HOLD_MS = 3000;

/** What the bar shows over the playing item while a swipe throws it out and brings one in. */
type Shown = { item: QueueItem; from: string; coming: QueueItem; landed: boolean };

type Press = {
  pointerId: number;
  x: number;
  y: number;
  axis: 'x' | 'y' | null;
  tracker: ReturnType<typeof velocityTracker>;
};

type Options = {
  item: QueueItem;
  /** The song a sideways swipe brings in; null brings the same song back. */
  incoming: (direction: 'next' | 'previous') => QueueItem | null;
  /** What a let-go does (the same as before: change the song, open or close the player). */
  act: (action: SwipeAction) => void;
};

const motionLevel = () => document.documentElement.dataset.motion ?? 'lively';

/**
 * The phone player bar's swipes. Up and down move the whole bar a little (it springs back; up
 * opens the full player, down closes it); sideways the bar's song (cover and names) follows the
 * finger, and a swipe throws it off in its direction with the finger's speed while the song that
 * comes next (or before) arrives from the other side and settles with a spring; with none, the
 * same song comes back. The song coming in is read from the queue at once, so it shows before the
 * player reports it. Only motion values move per frame (transforms, no React state per move);
 * reduced motion fades instead, off just swaps.
 */
export const useBarSwipe = ({ item, incoming, act }: Options) => {
  const barY = useMotionValue(0);
  const contentX = useMotionValue(0);
  const contentOpacity = useMotionValue(1);
  const [shown, setShown] = useState<Shown | null>(null);
  /** Counts the songs a swipe brought in: each is a card of its own, not a change of artwork. */
  const [card, setCard] = useState(0);
  const press = useRef<Press | null>(null);
  const dragged = useRef(false);
  const running = useRef<AnimationPlaybackControls[]>([]);
  /** Each swipe's turn: a newer one stops what an older one still had to do. */
  const turn = useRef(0);
  const width = useRef(0);

  // The player has got to the song that came in (or moved somewhere else): show it as it is.
  if (
    shown &&
    ((shown.landed && item.id === shown.coming.id) ||
      (item.id !== shown.from && item.id !== shown.coming.id))
  ) {
    setShown(null);
  }

  useEffect(() => {
    if (!shown?.landed) return;
    const timer = setTimeout(() => setShown(null), LANDED_HOLD_MS);
    return () => clearTimeout(timer);
  }, [shown]);

  useEffect(
    () => () => {
      for (const each of running.current) each.stop();
    },
    [],
  );

  const run = (controls: AnimationPlaybackControls) => {
    running.current.push(controls);
    return controls.finished;
  };

  const stopAll = () => {
    for (const each of running.current) each.stop();
    running.current = [];
  };

  const throwOut = async (direction: 'next' | 'previous', speed: number) => {
    const mine = ++turn.current;
    const coming = incoming(direction) ?? item;
    setShown({ item, from: item.id, coming, landed: false });
    const land = () => {
      setShown({ item: coming, from: item.id, coming, landed: true });
      if (coming.id !== item.id) setCard((count) => count + 1);
    };
    const level = motionLevel();
    if (level === 'off') {
      contentX.set(0);
      contentOpacity.set(1);
      land();
      return;
    }
    if (level === 'reduced') {
      await run(animate(contentOpacity, 0, { duration: FADE }));
      if (mine !== turn.current) return;
      contentX.set(0);
      land();
      await run(animate(contentOpacity, 1, { duration: FADE }));
      return;
    }
    const entry = comeIn(direction, speed, width.current || 400);
    const off = -entry.from;
    await run(
      animate(contentX, off, {
        duration: throwDuration(off - contentX.get(), speed),
        ease: 'linear',
      }),
    );
    if (mine !== turn.current) return;
    land();
    // On the other side by a jump: a `set` would count the leap across the bar as the value's
    // own motion (thousands of px/s away from its place), and the spring would fling the song far
    // past the bar and bring it back from the side it was thrown to.
    contentX.jump(entry.from);
    await run(animate(contentX, 0, { ...COME_IN, velocity: entry.velocity }));
  };

  const springBack = () => {
    const level = motionLevel();
    if (level === 'off') {
      barY.set(0);
      contentX.set(0);
      return;
    }
    const timing = level === 'reduced' ? { duration: FADE } : SETTLE;
    void run(animate(barY, 0, timing));
    void run(animate(contentX, 0, timing));
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    if (event.pointerType === 'mouse' && event.button !== 0) return;
    dragged.current = false;
    const tracker = velocityTracker();
    tracker.add(event.clientX, event.clientY, event.timeStamp);
    press.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      axis: null,
      tracker,
    };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
    const held = press.current;
    if (!held || held.pointerId !== event.pointerId) return;
    held.tracker.add(event.clientX, event.clientY, event.timeStamp);
    const dx = event.clientX - held.x;
    const dy = event.clientY - held.y;
    if (!held.axis) {
      if (Math.hypot(dx, dy) < AXIS_SLOP) return;
      held.axis = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y';
      dragged.current = true;
      width.current = event.currentTarget.offsetWidth;
      // From here the bar has the pointer: its buttons get no click from this press.
      event.currentTarget.setPointerCapture?.(event.pointerId);
      stopAll();
      // A swipe caught mid-flight goes on from where its content is.
      if (shown && !shown.landed) {
        turn.current++;
        setShown({ ...shown, item: shown.coming, landed: true });
        if (shown.coming.id !== shown.item.id) setCard((count) => count + 1);
      }
    }
    if (held.axis === 'x') contentX.set(dx);
    else barY.set(dy * (dy < 0 ? ELASTIC_UP : ELASTIC_DOWN));
  };

  const finish = (event: ReactPointerEvent<HTMLElement>, cancelled: boolean) => {
    const held = press.current;
    if (!held || held.pointerId !== event.pointerId) return;
    press.current = null;
    if (!held.axis) return;
    const velocity = held.tracker.velocity();
    const offset = { x: event.clientX - held.x, y: event.clientY - held.y };
    const action = cancelled
      ? null
      : held.axis === 'x'
        ? swipeAction({ x: offset.x, y: 0 }, { x: velocity.x, y: 0 })
        : swipeAction({ x: 0, y: offset.y }, { x: 0, y: velocity.y });
    if (action === 'next' || action === 'previous') {
      act(action);
      void throwOut(action, velocity.x);
      return;
    }
    springBack();
    if (action) act(action);
  };

  return {
    barY,
    contentX,
    contentOpacity,
    /** The song the bar shows: the playing one, or the one a swipe throws out or brings in. */
    shownItem: shown?.item ?? item,
    /** Changes when a swipe brings a song in (a key: its artwork is drawn afresh, not faded). */
    card,
    /** Whether the last press moved (a drag, not a tap). */
    dragged,
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp: (event: ReactPointerEvent<HTMLElement>) => finish(event, false),
      onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => finish(event, true),
    },
  };
};
