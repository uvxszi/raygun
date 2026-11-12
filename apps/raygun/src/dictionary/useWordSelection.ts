import { useCallback, useEffect, useEffectEvent, useRef } from 'react';
import type { RefObject } from 'react';

import { textOffsetOf } from '../novel/readAloud/textOffset';

/** Where a selection is on screen, in the window's coordinates. */
export type SelectionBox = { top: number; bottom: number; left: number; right: number };

/** Words selected in a book's text. */
export type WordSelection = {
  /** As selected, spaces run together. */
  text: string;
  box: SelectionBox;
  /** The block (`data-block`) the selection starts in, when it's in one. */
  block: number | null;
  /** Where in that block's text the selection starts (UTF-16), for reading aloud from there. */
  offset?: number | null;
};

/**
 * How long the selection must rest before it's looked up: a phone's handles move it as they're
 * dragged, and a long press selects one word before a drag makes it more.
 */
export const SETTLE_MS = 350;

/** The longest selection offered for a lookup; longer ones are copied or translated. */
export const MAX_LOOKUP_CHARS = 80;

/** Whether a selection is short enough to look up: a word or a few. */
export const isLookupText = (text: string) =>
  text.length <= MAX_LOOKUP_CHARS && text.split(/\s+/).length <= 6 && /\p{L}/u.test(text);

/** The selection now, when it lies inside `root` and has text. */
export const readSelection = (root: HTMLElement | null): WordSelection | null => {
  const selection = typeof window === 'undefined' ? null : window.getSelection();
  if (!root || !selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return null;
  const text = selection.toString().replace(/\s+/g, ' ').trim();
  if (!text) return null;
  const rect = range.getBoundingClientRect?.();
  const start =
    range.startContainer.nodeType === Node.ELEMENT_NODE
      ? (range.startContainer as Element)
      : range.startContainer.parentElement;
  const blockElement = start?.closest<HTMLElement>('[data-block]');
  const block = blockElement?.dataset.block;
  return {
    text,
    box: {
      top: rect?.top ?? 0,
      bottom: rect?.bottom ?? 0,
      left: rect?.left ?? 0,
      right: rect?.right ?? 0,
    },
    block: block === undefined ? null : Number(block),
    offset: blockElement
      ? textOffsetOf(blockElement, range.startContainer, range.startOffset)
      : null,
  };
};

/**
 * Follows the document's selection inside `root` while `enabled`: `onSelect` hears each new
 * selection once it rests (at once when a mouse lets go). A selection that goes away tells
 * nothing: what was looked up stays until it's closed. Returns a function that forgets the last
 * selection heard (the lookup closed), so the same words selected again are heard again.
 */
export const useWordSelection = (
  root: RefObject<HTMLElement | null>,
  enabled: boolean,
  onSelect: (selection: WordSelection) => void,
) => {
  const last = useRef('');
  const report = useEffectEvent(() => {
    const found = readSelection(root.current);
    if (!found) {
      last.current = '';
      return;
    }
    const key = `${found.block}:${found.text}`;
    if (key === last.current) return;
    last.current = key;
    onSelect(found);
  });
  useEffect(() => {
    if (!enabled) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const settle = () => {
      clearTimeout(timer);
      timer = setTimeout(report, SETTLE_MS);
    };
    // A mouse or a pen lets go where it means to: no need to wait.
    const onPointerUp = (event: PointerEvent) => {
      if (event.pointerType === 'touch') return;
      clearTimeout(timer);
      timer = setTimeout(report, 0);
    };
    document.addEventListener('selectionchange', settle);
    document.addEventListener('pointerup', onPointerUp);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('selectionchange', settle);
      document.removeEventListener('pointerup', onPointerUp);
    };
  }, [enabled]);
  return useCallback(() => {
    last.current = '';
  }, []);
};

/** Whether the reader's text holds a selection now (a tap then only ends it). */
export const hasSelectionIn = (root: HTMLElement | null) => readSelection(root) !== null;

/** Lets go of the document's selection. */
export const clearSelection = () => {
  window.getSelection()?.removeAllRanges();
};
