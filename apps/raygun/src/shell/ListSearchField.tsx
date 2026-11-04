import { Search } from 'lucide-react';
import type { FC } from 'react';

import { TextField } from './TextField';

/** The field over a long list that narrows it (M46.35). */
export const ListSearchField: FC<{
  value: string;
  onChange: (value: string) => void;
  label: string;
  className?: string;
}> = ({ value, onChange, label, className = '' }) => (
  <TextField
    type="search"
    value={value}
    onChange={onChange}
    icon={Search}
    iconSize={15}
    placeholder={label}
    aria-label={label}
    autoComplete="off"
    className={`bg-surface rounded-full py-0.5 pr-0.5 pl-3 ${className}`}
    inputClassName="py-1 text-sm"
  />
);
