import {
  ALPHA_MAX,
  FIND_COLS,
  FIND_ROWS,
  MIN_SAMPLES,
  OverlayFinder,
  restorePixel,
  sampleLayout,
  shaderBox,
  watermarkContrast,
  watermarkFeather,
  WatermarkEstimator,
} from './watermarkMath';

/** The same numbers as Android's (a small linear congruential generator). */
const random = (seed: number) => {
  let state = seed;
  return () => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state / 2_147_483_648;
  };
};

describe('the watermark remover as numbers', () => {
  it('lays the box and a ring around it on one grid, inside the picture', () => {
    const middle = sampleLayout({ x: 4_000, y: 4_000, width: 1_000, height: 500 }, 1920, 1080);
    expect(middle.innerCols).toBe(48);
    expect(middle.innerRows).toBe(14);
    expect(middle.innerLeft).toBe(24);
    expect(middle.innerTop).toBe(7);
    expect(middle.cols).toBe(96);
    expect(middle.rows).toBe(28);
    expect(middle.rect.left).toBeCloseTo(768 - 96, 5);

    // A corner box keeps its ring to the picture.
    const corner = sampleLayout({ x: 0, y: 0, width: 1_000, height: 1_000 }, 1920, 1080);
    expect(corner.innerLeft).toBe(0);
    expect(corner.innerTop).toBe(0);
    expect(corner.cols).toBe(corner.innerCols + Math.ceil(corner.innerCols / 2));
    expect(corner.rect.left).toBe(0);
    expect(corner.rect.top).toBe(0);
  });

  it('finds the overlay’s opacity and colour from frames that change under it', () => {
    const layout = sampleLayout({ x: 4_000, y: 4_000, width: 1_000, height: 1_000 }, 640, 360);
    const estimator = new WatermarkEstimator(layout);
    const next = random(7);
    const { cols, rows, innerLeft, innerTop, innerCols, innerRows } = layout;
    // The overlay: white at half opacity over the box's middle.
    const inLogo = (col: number, row: number) =>
      col >= innerLeft + innerCols / 4 &&
      col < innerLeft + (innerCols * 3) / 4 &&
      row >= innerTop + innerRows / 4 &&
      row < innerTop + (innerRows * 3) / 4;
    for (let frame = 0; frame < 200; frame += 1) {
      const pixels = new Uint8Array(cols * rows * 4);
      for (let cell = 0; cell < cols * rows; cell += 1) {
        const video = next();
        const shown = inLogo(cell % cols, Math.floor(cell / cols)) ? 0.5 + 0.5 * video : video;
        const byte = Math.round(shown * 255);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      estimator.add(pixels);
      if (frame === MIN_SAMPLES - 2) expect(estimator.estimate()).toBeNull();
    }
    const mask = estimator.estimate();
    expect(mask).not.toBeNull();
    const at = (col: number, row: number) => (row * innerCols + col) * 4;
    const centre = at(Math.floor(innerCols / 2), Math.floor(innerRows / 2));
    expect((mask?.data[centre + 3] ?? 0) / 255).toBeGreaterThan(0.4);
    expect((mask?.data[centre + 3] ?? 0) / 255).toBeLessThan(0.6);
    // The colour is stored times the opacity.
    const alpha = (mask?.data[centre + 3] ?? 1) / 255;
    expect((mask?.data[centre] ?? 0) / 255 / alpha).toBeGreaterThan(0.85);
    // The box's corner has no overlay.
    expect(mask?.data[at(0, 0) + 3]).toBe(0);
  });

  it('waits for a picture that changes', () => {
    const layout = sampleLayout({ x: 4_000, y: 4_000, width: 1_000, height: 1_000 }, 640, 360);
    const estimator = new WatermarkEstimator(layout);
    const still = new Uint8Array(layout.cols * layout.rows * 4).fill(128);
    for (let frame = 0; frame < 60; frame += 1) estimator.add(still);
    expect(estimator.estimate()).toBeNull();
  });

  it('finds a still logo in a corner of a moving picture', () => {
    const finder = new OverlayFinder();
    const next = random(11);
    // A logo's letters: still bright strokes at the top right.
    const logo = (col: number, row: number) =>
      col >= 52 && col <= 58 && row >= 3 && row <= 6 && (col + row) % 2 === 0;
    for (let frame = 0; frame < 48; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        const col = cell % FIND_COLS;
        const row = Math.floor(cell / FIND_COLS);
        const byte = logo(col, row) ? 240 : Math.round(next() * 200);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      finder.add(pixels);
    }
    const box = finder.find();
    expect(box).not.toBeNull();
    // Columns 51–59 and rows 2–7 with a cell of margin.
    expect(box?.x).toBe(Math.round((51 / 64) * 10_000));
    expect(box?.y).toBe(Math.round((2 / 36) * 10_000));
    expect((box?.x ?? 0) + (box?.width ?? 0)).toBe(Math.round((60 / 64) * 10_000));
    expect((box?.y ?? 0) + (box?.height ?? 0)).toBe(Math.round((8 / 36) * 10_000));
  });

  it('takes a letterbox for no logo', () => {
    const finder = new OverlayFinder();
    const next = random(5);
    for (let frame = 0; frame < 48; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        const byte = cell < FIND_COLS * 4 ? 0 : Math.round(next() * 200);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      finder.add(pixels);
    }
    expect(finder.find()).toBeNull();
  });

  it('finds nothing in a picture that hardly changes', () => {
    const finder = new OverlayFinder();
    const flat = new Uint8Array(FIND_COLS * FIND_ROWS * 4).fill(90);
    for (let frame = 0; frame < 48; frame += 1) finder.add(flat);
    expect(finder.find()).toBeNull();
  });

  it('maps the box and controls onto the shader’s numbers', () => {
    expect(shaderBox({ x: 1_000, y: 2_000, width: 3_000, height: 1_000 })).toEqual([
      0.1, 0.7, 0.4, 0.8,
    ]);
    expect(watermarkContrast(100)).toBe(2);
    expect(watermarkContrast(-100)).toBe(0.5);
    expect(watermarkFeather(50)).toBe(0.25);
  });

  /** Three held shots of different scenery with strong edges (a little noise), a logo or not. */
  const shots = (withLogo: boolean) => {
    const finder = new OverlayFinder();
    const next = random(17);
    const logo = (col: number, row: number) =>
      col >= 50 && col <= 58 && row >= 3 && row <= 6 && (col + row) % 2 === 0;
    const blocks = Array.from({ length: 22 * 12 }, () => next());
    for (let shot = 0; shot < 3; shot += 1) {
      // Scenery: blocks of 3 × 3 cells at their own levels, held for the whole shot; each shot's
      // level of a block is another third of the way round, so the shots differ.
      const levels = blocks.map((block) => 0.1 + 0.8 * ((block + shot / 3) % 1));
      for (let frame = 0; frame < 30; frame += 1) {
        const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
        for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
          const col = cell % FIND_COLS;
          const row = Math.floor(cell / FIND_COLS);
          const scenery =
            (levels[Math.floor(row / 3) * 22 + Math.floor(col / 3)] ?? 0) + (next() - 0.5) * 0.02;
          const value = withLogo && logo(col, row) ? 0.95 : scenery;
          const byte = Math.round(Math.min(1, Math.max(0, value)) * 255);
          pixels.set([byte, byte, byte, 255], cell * 4);
        }
        finder.add(pixels);
      }
    }
    return finder;
  };

  it('takes a uniform patch that looks alike in every shot (grass) for no overlay', () => {
    const finder = new OverlayFinder();
    const next = random(23);
    const grass = (col: number, row: number) => col >= 16 && col <= 23 && row >= 28;
    for (let frame = 0; frame < 90; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        const value = grass(cell % FIND_COLS, Math.floor(cell / FIND_COLS))
          ? 0.35 + (next() - 0.5) * 0.01
          : next();
        const byte = Math.round(value * 255);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      finder.add(pixels);
    }
    expect(finder.cuts).toBeGreaterThanOrEqual(2);
    expect(finder.verdict()).toEqual({ kind: 'none' });
  });

  it('takes scenery held still within shots for no overlay', () => {
    const finder = shots(false);
    expect(finder.cuts).toBe(2);
    expect(finder.ready).toBe(true);
    expect(finder.verdict()).toEqual({ kind: 'none' });
  });

  it('finds a logo that stays the same across scene changes', () => {
    const finder = shots(true);
    const box = finder.find();
    expect(box).not.toBeNull();
    expect((box?.x ?? 0) / 10_000).toBeLessThanOrEqual(50 / 64);
    expect(((box?.x ?? 0) + (box?.width ?? 0)) / 10_000).toBeGreaterThanOrEqual(59 / 64);
  });

  it('waits for scene changes in one held shot, and can’t tell without them', () => {
    const finder = new OverlayFinder();
    const next = random(19);
    const levels = Array.from({ length: 22 * 12 }, () => 0.1 + next() * 0.8);
    for (let frame = 0; frame < 120; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        const level =
          levels[
            Math.floor(Math.floor(cell / FIND_COLS) / 3) * 22 + Math.floor((cell % FIND_COLS) / 3)
          ] ?? 0;
        const byte = Math.round((level + (next() - 0.5) * 0.02) * 255);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      finder.add(pixels);
    }
    expect(finder.ready).toBe(false);
    expect(finder.verdict()).toEqual({ kind: 'unsure' });
  });

  it('finds nothing in moving video without an overlay, and can’t tell in a still one', () => {
    const finder = new OverlayFinder();
    const next = random(3);
    for (let frame = 0; frame < 48; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        pixels.set([next() * 255, next() * 255, next() * 255, 255].map(Math.round), cell * 4);
      }
      finder.add(pixels);
    }
    expect(finder.verdict()).toEqual({ kind: 'none' });

    const still = new OverlayFinder();
    const flat = new Uint8Array(FIND_COLS * FIND_ROWS * 4).fill(90);
    for (let frame = 0; frame < 48; frame += 1) still.add(flat);
    expect(still.verdict()).toEqual({ kind: 'unsure' });
  });

  it('finds a still text block, and takes half-clear white text out without tinting the picture', () => {
    // Coloured video changing every frame, a caption over the bottom left.
    const opacity = 0.4;
    const inBlock = (col: number, row: number) => col >= 4 && col <= 14 && row >= 29 && row <= 32;
    const finder = new OverlayFinder();
    const next = random(13);
    for (let frame = 0; frame < 48; frame += 1) {
      const pixels = new Uint8Array(FIND_COLS * FIND_ROWS * 4);
      for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
        const col = cell % FIND_COLS;
        const block = inBlock(col, Math.floor(cell / FIND_COLS));
        // A caption: a dark box that stays, its letters white at 80 % (every other column).
        const video = block ? [0.1, 0.08, 0.06] : [next(), next(), next()];
        const letter = block && col % 2 === 0 ? 0.8 : 0;
        const shown = video.map((value) => letter + (1 - letter) * value);
        pixels.set([...shown.map((value) => Math.round(value * 255)), 255], cell * 4);
      }
      finder.add(pixels);
    }
    const verdict = finder.verdict();
    expect(verdict.kind).toBe('found');
    const box = verdict.kind === 'found' ? verdict.box : null;
    expect((box?.x ?? 0) / 10_000).toBeLessThanOrEqual(4 / 64);
    expect(((box?.x ?? 0) + (box?.width ?? 0)) / 10_000).toBeGreaterThanOrEqual(15 / 64);
    expect((box?.y ?? 0) / 10_000).toBeLessThanOrEqual(29 / 36);

    // The remover learns it from frames whose video changes under it, then restores colours.
    const layout = sampleLayout({ x: 4_000, y: 4_000, width: 1_000, height: 1_000 }, 640, 360);
    const estimator = new WatermarkEstimator(layout);
    const { cols, rows, innerLeft, innerTop, innerCols, innerRows } = layout;
    const covered = (col: number, row: number) =>
      col >= innerLeft &&
      col < innerLeft + innerCols &&
      row >= innerTop &&
      row < innerTop + innerRows;
    for (let frame = 0; frame < 300; frame += 1) {
      const pixels = new Uint8Array(cols * rows * 4);
      for (let cell = 0; cell < cols * rows; cell += 1) {
        const video = [next(), next(), next()];
        const over = covered(cell % cols, Math.floor(cell / cols));
        const shown = video.map((value) => (over ? opacity + (1 - opacity) * value : value));
        pixels.set([...shown.map((value) => Math.round(value * 255)), 255], cell * 4);
      }
      estimator.add(pixels);
    }
    const mask = estimator.estimate();
    expect(mask).not.toBeNull();
    const cell = (Math.floor(innerRows / 2) * innerCols + Math.floor(innerCols / 2)) * 4;
    const texel = Array.from(mask?.data.subarray(cell, cell + 4) ?? []);
    expect((texel[3] ?? 0) / 255).toBeLessThanOrEqual(ALPHA_MAX + 0.01);
    for (const video of [
      [0.6, 0.3, 0.2],
      [0.1, 0.5, 0.8],
      [0.9, 0.9, 0.2],
    ]) {
      const shown = video.map((value) => opacity + (1 - opacity) * value);
      const restored = restorePixel(shown, texel);
      restored.forEach((value, channel) => {
        expect(Math.abs(value - (video[channel] ?? 0))).toBeLessThan(0.1);
      });
      // The channels keep their order: no hue shift.
      const order = (rgb: number[]) => [0, 1, 2].sort((a, b) => (rgb[b] ?? 0) - (rgb[a] ?? 0));
      if (new Set(video).size === 3) expect(order(restored)).toEqual(order(video));
    }
  });

  it('masks only the mark itself, never still picture without edges of its own', () => {
    // As Android's: a box half over a still, flat part of the picture (a dark band) and half over
    // moving picture with a logo's strokes in it.
    const layout = sampleLayout({ x: 4_000, y: 4_000, width: 1_000, height: 1_000 }, 640, 360);
    const estimator = new WatermarkEstimator(layout);
    const next = random(5);
    const { cols, rows, innerLeft, innerTop, innerCols, innerRows } = layout;
    const stroke = (col: number, row: number) =>
      col >= innerLeft + Math.floor(innerCols / 2) + 2 &&
      col < innerLeft + innerCols - 2 &&
      row >= innerTop + 2 &&
      row < innerTop + innerRows - 2 &&
      col % 3 === 0;
    const band = (col: number) => col < innerLeft + Math.floor(innerCols / 2);
    for (let frame = 0; frame < 200; frame += 1) {
      const pixels = new Uint8Array(cols * rows * 4);
      for (let cell = 0; cell < cols * rows; cell += 1) {
        const col = cell % cols;
        const row = Math.floor(cell / cols);
        const video = next();
        const inside =
          col >= innerLeft &&
          row >= innerTop &&
          col < innerLeft + innerCols &&
          row < innerTop + innerRows;
        const shown = inside && band(col) ? 0.1 : stroke(col, row) ? 0.6 + 0.4 * video : video;
        const byte = Math.trunc(shown * 255);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      estimator.add(pixels);
    }
    const mask = estimator.estimate();
    expect(mask).not.toBeNull();
    const alphaAt = (col: number, row: number) => mask?.data[(row * innerCols + col) * 4 + 3] ?? -1;
    // The still band has no mark in it: untouched.
    expect(alphaAt(2, Math.floor(innerRows / 2))).toBe(0);
    // A stroke is masked.
    let strokeCol = Math.floor(innerCols / 2) + 2;
    while ((strokeCol + innerLeft) % 3 !== 0) strokeCol += 1;
    expect(alphaAt(strokeCol, Math.floor(innerRows / 2))).toBeGreaterThan(0);
    // Nothing found yet in a box over still picture alone: no mask, so nothing changes.
    const still = new WatermarkEstimator(layout);
    for (let frame = 0; frame < 60; frame += 1) {
      const pixels = new Uint8Array(cols * rows * 4);
      for (let cell = 0; cell < cols * rows; cell += 1) {
        const col = cell % cols;
        const row = Math.floor(cell / cols);
        const inside =
          col >= innerLeft &&
          row >= innerTop &&
          col < innerLeft + innerCols &&
          row < innerTop + innerRows;
        const byte = inside ? 128 : Math.trunc(next() * 255);
        pixels.set([byte, byte, byte, 255], cell * 4);
      }
      still.add(pixels);
    }
    const flat = still.estimate();
    expect(flat === null || flat.data.every((value, index) => index % 4 !== 3 || value === 0)).toBe(
      true,
    );
  });
});
