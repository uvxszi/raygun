import {
  Outlet,
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from '@tanstack/react-router';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useLayoutEffect, useRef } from 'react';
import type { FC } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { create } from 'zustand';

import { forgetScrollPositions, useScrollMemory } from './scrollMemory';

const ROW_PX = 100;
const VIEW_PX = 500;

/** How many rows the list page draws: fewer stands for a page still loading. */
const useRows = create<{ rows: number }>(() => ({ rows: 50 }));

/** jsdom has no layout: `<main>` scrolls as far as its rows reach. */
const giveLayout = (element: HTMLElement) => {
  let top = 0;
  const height = () => element.querySelectorAll('li').length * ROW_PX;
  Object.defineProperty(element, 'clientHeight', { configurable: true, get: () => VIEW_PX });
  Object.defineProperty(element, 'scrollHeight', { configurable: true, get: height });
  Object.defineProperty(element, 'scrollTop', {
    configurable: true,
    get: () => top,
    set: (value: number) => {
      top = Math.max(0, Math.min(value, height() - VIEW_PX));
    },
  });
  element.scrollTo = ((_left: number, value: number) => {
    element.scrollTop = value;
  }) as typeof element.scrollTo;
};

const Shell: FC = () => {
  const mainRef = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    if (mainRef.current) giveLayout(mainRef.current);
  }, []);
  useScrollMemory(mainRef, (pathname) => pathname === '/read');
  return (
    <main ref={mainRef} data-testid="page-content">
      <Outlet />
    </main>
  );
};

const ListPage: FC = () => {
  const rows = useRows((store) => store.rows);
  return (
    <ul aria-label="list">
      {Array.from({ length: rows }, (_, index) => (
        <li key={index}>Row {index}</li>
      ))}
    </ul>
  );
};

const DetailPage: FC = () => (
  <ul aria-label="detail">
    {Array.from({ length: 40 }, (_, index) => (
      <li key={index}>Detail {index}</li>
    ))}
  </ul>
);

const setup = async () => {
  const root = createRootRoute({ component: Shell });
  const router = createRouter({
    routeTree: root.addChildren([
      createRoute({ getParentRoute: () => root, path: '/list', component: ListPage }),
      createRoute({ getParentRoute: () => root, path: '/detail', component: DetailPage }),
    ]),
    history: createMemoryHistory({ initialEntries: ['/list'] }),
  });
  render(<RouterProvider router={router} />);
  await screen.findByRole('list', { name: 'list' });
  const main = screen.getByTestId('page-content');
  act(() => {
    main.scrollTop = 2000;
  });
  await act(() => router.navigate({ to: '/detail' }));
  await screen.findByRole('list', { name: 'detail' });
  return { router, main };
};

afterEach(() => {
  forgetScrollPositions();
  useRows.setState({ rows: 50 });
});

describe('useScrollMemory', () => {
  it('starts a new page at the top and returns to the spot on Back', async () => {
    const { router, main } = await setup();
    await waitFor(() => expect(main.scrollTop).toBe(0));

    act(() => {
      main.scrollTop = 900;
    });
    act(() => router.history.back());
    await screen.findByRole('list', { name: 'list' });
    await waitFor(() => expect(main.scrollTop).toBe(2000));

    // Forward returns to where the other page was.
    act(() => router.history.forward());
    await screen.findByRole('list', { name: 'detail' });
    await waitFor(() => expect(main.scrollTop).toBe(900));
  });

  it('keeps scrolling down while the page grows after Back', async () => {
    const { router, main } = await setup();
    act(() => useRows.setState({ rows: 3 }));
    act(() => router.history.back());
    await screen.findByRole('list', { name: 'list' });
    expect(main.scrollTop).toBe(0);

    act(() => useRows.setState({ rows: 12 }));
    await waitFor(() => expect(main.scrollTop).toBe(12 * ROW_PX - VIEW_PX));
    act(() => useRows.setState({ rows: 60 }));
    await waitFor(() => expect(main.scrollTop).toBe(2000));
  });

  it('stops once the person scrolls themselves', async () => {
    const user = userEvent.setup();
    const { router, main } = await setup();
    act(() => useRows.setState({ rows: 8 }));
    act(() => router.history.back());
    await screen.findByRole('list', { name: 'list' });
    await waitFor(() => expect(main.scrollTop).toBe(8 * ROW_PX - VIEW_PX));

    await user.keyboard('{ArrowUp}');
    act(() => {
      main.scrollTop = 100;
    });
    act(() => useRows.setState({ rows: 60 }));
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(main.scrollTop).toBe(100);
  });
});
