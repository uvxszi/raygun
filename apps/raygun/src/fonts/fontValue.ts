import type { CSSProperties } from 'react';

import { quoteFamily, useInstalledFamily } from './systemFonts';

/** A font picker's value: one of Raygun's own choices, or a family of the device's. */
export type FontValue = { kind: 'own'; id: string } | { kind: 'family'; family: string };

/**
 * One of Raygun's own choices, listed first: Default, or the reader's Serif and Sans. `css` is
 * what else draws it besides its family (subtitles' Condensed and Small capitals).
 */
export type OwnFont = { id: string; label: string; stack: string; css?: CSSProperties };

/** The chosen font's name and how to show it: in its own font where this device has it. */
export const useFontLabel = (own: OwnFont[], value: FontValue, fallback: string) => {
  const family = useInstalledFamily(value.kind === 'family' ? value.family : undefined);
  if (value.kind === 'family') {
    return {
      label: value.family,
      stack: family ? `${quoteFamily(family)}, ${fallback}` : fallback,
      css: undefined,
    };
  }
  const option = own.find((item) => item.id === value.id) ?? own[0];
  return { label: option?.label ?? '', stack: option?.stack ?? fallback, css: option?.css };
};
