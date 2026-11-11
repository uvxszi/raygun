import { render, screen } from '@testing-library/react';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { Popover } from './Popover';
import { safeInsets } from './safeArea';
import { Sheet } from './Sheet';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

/** A phone on its side: the cutout on the left, the navigation bar on the right. */
const LANDSCAPE = { top: 24, right: 48, bottom: 0, left: 32 };

/** As Android's activity tells the page (MainActivity's SafeArea). */
const setSafeArea = (area: typeof LANDSCAPE) => {
  for (const [side, value] of Object.entries(area)) {
    document.documentElement.style.setProperty(`--safe-${side}`, `${value}px`);
  }
};

describe('The safe area', () => {
  afterEach(() => {
    for (const side of ['top', 'right', 'bottom', 'left']) {
      document.documentElement.style.removeProperty(`--safe-${side}`);
    }
  });

  it('is read as Android sets it, and is nothing elsewhere', () => {
    expect(safeInsets()).toEqual({ top: 0, right: 0, bottom: 0, left: 0 });
    setSafeArea(LANDSCAPE);
    expect(safeInsets()).toEqual(LANDSCAPE);
  });

  it('keeps the shell’s pages, bars and tabs clear of a cutout and a navigation bar at the sides', async () => {
    AppShellWrapper.seedCore();
    await AppShellWrapper.mount();
    const page = screen.getByTestId('page-content');
    // The row holding the rail, the page, the player bar and the tabs.
    const row = page.closest('[class*="pl-[var(--safe-left)]"]');
    expect(row).not.toBeNull();
    expect(row?.className).toContain('pr-[var(--safe-right)]');
    expect(row).toContainElement(screen.getByTestId('bottom-tabs'));
    // Under the status bar too.
    expect(page.className).toContain('pt-[var(--safe-top)]');
  });

  it('keeps sheets clear of the status bar and the sides, their body clear of the bottom', () => {
    render(
      <Sheet title="Sort" onClose={vi.fn()}>
        <p>Choices</p>
      </Sheet>,
    );
    const dialog = screen.getByRole('dialog', { name: 'Sort' });
    const backdrop = dialog.parentElement;
    for (const side of [
      'pt-[var(--safe-top)]',
      'pr-[var(--safe-right)]',
      'pl-[var(--safe-left)]',
    ]) {
      expect(backdrop?.className).toContain(side);
    }
    expect(screen.getByTestId('sheet-body').className).toContain('var(--safe-bottom)');
  });

  it('keeps popovers inside it', () => {
    setSafeArea(LANDSCAPE);
    const anchor = document.createElement('button');
    document.body.append(anchor);
    render(
      <Popover anchor={anchor} label="Sort" onClose={vi.fn()}>
        <p>Choices</p>
      </Popover>,
    );
    // The button at the window's top left (jsdom's boxes are empty): beside the cutout, under
    // the status bar.
    const popover = screen.getByTestId('popover');
    expect(popover.style.left).toBe(`${LANDSCAPE.left + 8}px`);
    expect(popover.style.top).toBe(`${LANDSCAPE.top + 8}px`);
    anchor.remove();
  });
});
