import { Link } from '@tanstack/react-router';
import type { FC } from 'react';

export type TabItem = {
  id: string;
  label: string;
  /** Where the tab leads; tabs are links so Back and reloads keep the choice. */
  to: string;
  search: Record<string, unknown>;
  testId?: string;
};

/** A row of underlined tabs under the top bar (Mihon's and Aniyomi's), scrolling when wide. */
export const Tabs: FC<{ label: string; items: TabItem[]; active: string }> = ({
  label,
  items,
  active,
}) => (
  <nav
    aria-label={label}
    className="border-surface -mx-4 flex overflow-x-auto border-b px-2 lg:-mx-10 lg:px-6"
  >
    {items.map((item) => {
      const current = item.id === active;
      return (
        <Link
          key={item.id}
          to={item.to}
          search={item.search}
          replace
          data-testid={item.testId}
          aria-current={current ? 'page' : undefined}
          className={`relative shrink-0 px-4 py-3 text-sm font-medium ${current ? 'text-primary' : 'text-muted'}`}
        >
          {item.label}
          {current && (
            <span
              aria-hidden
              className="bg-primary absolute inset-x-3 bottom-0 h-0.5 rounded-full"
            />
          )}
        </Link>
      );
    })}
  </nav>
);
