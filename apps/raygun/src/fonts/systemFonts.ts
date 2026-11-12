import { commands } from '@raygun/bindings';
import type { FontPrefs, SystemFont } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { usePlatform } from '../lib/usePlatform';

export const FONTS_KEY = ['fonts', 'installed'] as const;

export const FONT_DEFAULTS: FontPrefs = { interface: '', reading: '', subtitles: '' };

/** Raygun's own interface font: the system's. */
export const INTERFACE_STACK = 'system-ui, sans-serif';

/** The book reader's own typefaces (Settings → Reader), which a device font goes in front of. */
export const READER_STACKS = {
  serif: 'Georgia, "Noto Serif", "Times New Roman", serif',
  sans: 'system-ui, "Noto Sans", sans-serif',
} as const;

/** The font families installed on this device (Rust reads them once). */
export const useSystemFonts = (enabled = true) =>
  useQuery({
    queryKey: FONTS_KEY,
    queryFn: () => unwrapResult(commands.fontsList(false)),
    staleTime: Infinity,
    enabled,
  });

/** A family as `font-family` takes it: quoted, so "Segoe UI" or "Noto Serif CJK" stay whole. */
export const quoteFamily = (family: string) => `"${family.replace(/["\\]/g, '')}"`;

const registered = new Set<string>();

/**
 * Makes a family's files known to the webview (Android's, which finds few installed families by
 * name): one `FontFace` per file, loaded once text uses it.
 */
export const registerFaces = (font: SystemFont) => {
  if (font.faces.length === 0 || typeof FontFace === 'undefined' || !document.fonts) return;
  for (const face of font.faces) {
    if (registered.has(face.url)) continue;
    registered.add(face.url);
    const weight =
      face.weightMin === face.weightMax
        ? String(face.weightMin)
        : `${face.weightMin} ${face.weightMax}`;
    try {
      document.fonts.add(
        new FontFace(font.family, `url("${face.url}")`, {
          weight,
          style: face.italic ? 'italic' : 'normal',
          display: 'swap',
        }),
      );
    } catch {
      // A face the webview refuses: the family's other files still draw.
    }
  }
};

/** Whether this webview finds installed families by name (desktop), or needs their files. */
const findsByName = (platform: string | undefined) =>
  platform !== undefined && platform !== 'android';

/**
 * A family picked from the device's fonts as this device can draw it: by name on desktop, from
 * its files on Android once the list has it. Null for the default and for a family this device
 * doesn't have (picked on another, or removed since), which then falls back to the default.
 */
export const useInstalledFamily = (family: string | undefined): string | null => {
  const platform = usePlatform();
  const wanted = family?.trim() || null;
  const byName = findsByName(platform);
  const fonts = useSystemFonts(platform !== undefined && !byName && wanted !== null);
  const font = byName ? undefined : fonts.data?.find((item) => item.family === wanted);
  useEffect(() => {
    if (font) registerFaces(font);
  }, [font]);
  if (!wanted || platform === undefined) return null;
  return byName || font ? wanted : null;
};

/** The interface font (Settings → Appearance) as `--font-ui` on the root, which body text uses. */
export const useApplyFonts = (prefs: FontPrefs | undefined) => {
  const family = useInstalledFamily(prefs?.interface);
  useEffect(() => {
    const root = document.documentElement;
    if (family) root.style.setProperty('--font-ui', `${quoteFamily(family)}, ${INTERFACE_STACK}`);
    else root.style.removeProperty('--font-ui');
  }, [family]);
};
