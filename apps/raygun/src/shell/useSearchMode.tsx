import { Search } from 'lucide-react';
import { useState } from 'react';

import { TopBarAction } from './TopBar';
import type { TopBarSearch } from './TopBar';

/**
 * The top bar's search mode for a list (M46.35: every list can be searched): `action` is the
 * search button for the bar's actions, `search` goes to the bar, `query` narrows the list.
 */
export const useSearchMode = (placeholder: string) => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const action = <TopBarAction label={placeholder} icon={Search} onClick={() => setOpen(true)} />;
  const search: TopBarSearch | null = open
    ? {
        value: query,
        onChange: setQuery,
        placeholder,
        onClose: () => {
          setOpen(false);
          setQuery('');
        },
      }
    : null;
  return { query, action, search };
};
