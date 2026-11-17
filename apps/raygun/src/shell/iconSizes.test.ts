import { afterEach, describe, expect, it } from 'vitest';

import {
  ICON_SIZES,
  followDevicePixels,
  iconLength,
  iconSizeCss,
  installIconSizes,
} from './iconSizes';

describe('icon sizes', () => {
  afterEach(() => {
    document.getElementById('raygun-icon-sizes')?.remove();
    document.documentElement.style.removeProperty('--device-px');
  });

  it('scales a size by the icon unit, then rounds it to device pixels', () => {
    expect(iconLength(22)).toEqual({
      scaled: 'calc(22 * var(--icon-unit, 1px))',
      rounded: 'round(nearest, calc(22 * var(--icon-unit, 1px)), var(--device-px, 1px))',
    });
  });

  it('gives every whole size a rule, with a plain length before the rounded one', () => {
    expect(ICON_SIZES[0]).toBe(6);
    expect(ICON_SIZES.at(-1)).toBe(128);
    const css = iconSizeCss([16, 22]);
    expect(css.startsWith('@layer base {')).toBe(true);
    const rule = css.split('\n').find((line) => line.includes('[width="22"]'))!;
    const plain = rule.indexOf('width: calc(22 * var(--icon-unit, 1px))');
    const rounded = rule.indexOf('width: round(nearest, calc(22');
    expect(plain).toBeGreaterThan(-1);
    expect(rounded).toBeGreaterThan(plain);
    expect(rule).toContain('height: round(nearest, calc(22');
    // Other sizes follow typed attr() where the engine reads it.
    expect(css).toContain('attr(width type(<number>), 24)');
  });

  it('keeps icons whole inside rows and buttons', () => {
    expect(iconSizeCss([])).toContain(
      'svg.lucide { flex-shrink: 0; max-width: 100%; max-height: 100%; }',
    );
  });

  it('adds the stylesheet once', () => {
    installIconSizes();
    installIconSizes();
    expect(document.querySelectorAll('#raygun-icon-sizes')).toHaveLength(1);
  });

  it('sets one device pixel from the pixel ratio', () => {
    const before = window.devicePixelRatio;
    Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: 2.5 });
    const stop = followDevicePixels();
    expect(document.documentElement.style.getPropertyValue('--device-px')).toBe('0.4px');
    stop();
    Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: before });
  });
});
