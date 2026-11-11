/**
 * Presses a hold of an element's own acts on (a cover's save options, a move to the top, picking,
 * a menu that opens on a long press). The labels a held icon button shows (shell/ControlLabels)
 * stay away from them: those holds keep doing what they do, alone.
 */
const claimed = new WeakSet<Event>();

type Press = Event | { nativeEvent: Event };

/** Claims a press for the element's own hold. Call it from the press's `pointerdown`. */
export const claimHold = (press: Press) => {
  claimed.add('nativeEvent' in press ? press.nativeEvent : press);
};

/** Whether something claimed this `pointerdown` for a hold of its own. */
export const holdClaimed = (press: Event) => claimed.has(press);
