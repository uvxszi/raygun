import { render, screen } from '@testing-library/react';

import { DEFAULT_SUBTITLE_STYLE } from '../settings/playerOptions';
import { pictureBox } from './pictureBox';
import { SubtitleLines } from './SubtitleLines';
import { subtitleSlide } from './subtitlePlacement';

// A 16:9 film filling a 1000×563 player: the lines sit 45 px up its foot, under the control bar.
const BOX = pictureBox(1000, 563, 1920, 1080, 'fit');

describe('subtitle lines over the picture', () => {
  it('slide out of the controls’ way and back, keeping their size', () => {
    const { rerender } = render(
      <SubtitleLines lines={['Hello there']} style={DEFAULT_SUBTITLE_STYLE} lifted box={BOX} />,
    );
    const lines = screen.getByTestId('subtitle-lines');
    const size = screen.getByText('Hello there').style.fontSize;
    // Up above the bar (110 px), moved rather than placed again.
    const rise = 110 - (563 - BOX.top - BOX.height + BOX.height * 0.08);
    expect(Number(lines.dataset.slide)).toBeCloseTo(-rise, 3);
    const moved = /^translateY\((-[\d.]+)px\)$/.exec(lines.style.transform);
    expect(Number(moved?.[1])).toBeCloseTo(-rise, 3);
    const bottom = lines.style.bottom;

    rerender(
      <SubtitleLines
        lines={['Hello there']}
        style={DEFAULT_SUBTITLE_STYLE}
        lifted={false}
        box={BOX}
      />,
    );
    expect(lines.dataset.slide).toBe('0');
    expect(lines.style.transform).toBe('');
    // Same place, same size: only the slide changed.
    expect(lines.style.bottom).toBe(bottom);
    expect(screen.getByText('Hello there').style.fontSize).toBe(size);
  });

  it('moves the second subtitles below the top bar while it shows', () => {
    expect(subtitleSlide(BOX, DEFAULT_SUBTITLE_STYLE, true, 'top')).toBeCloseTo(
      64 - (BOX.top + BOX.height * 0.06),
      3,
    );
    expect(subtitleSlide(BOX, DEFAULT_SUBTITLE_STYLE, false, 'top')).toBe(0);
    // A tall window's letterboxed film starts low enough already.
    const tall = pictureBox(1000, 2000, 1920, 800, 'fit');
    expect(subtitleSlide(tall, DEFAULT_SUBTITLE_STYLE, true, 'top')).toBe(0);
    expect(subtitleSlide(tall, DEFAULT_SUBTITLE_STYLE, true, 'bottom')).toBe(0);
  });

  it('wraps lines within the chosen share of the picture’s width, in rounded boxes', () => {
    render(
      <SubtitleLines
        lines={['A long line that wraps']}
        style={{ ...DEFAULT_SUBTITLE_STYLE, maxWidth: 60, backgroundColor: 0xff000000 }}
        lifted={false}
        box={BOX}
      />,
    );
    const lines = screen.getByTestId('subtitle-lines');
    expect(lines).toHaveStyle({ width: '1000px', paddingLeft: '200px', paddingRight: '200px' });
    expect(screen.getByText('A long line that wraps')).toHaveStyle({
      backgroundColor: 'rgb(0, 0, 0)',
      borderRadius: '0.25em',
    });
  });
});
