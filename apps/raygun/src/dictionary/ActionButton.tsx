import type { Copy } from 'lucide-react';
import type { FC, ReactNode } from 'react';

/** One of the word panel's actions (Copy, Search in book, Translate…), as a pill. */
export const ActionButton: FC<{
  icon: typeof Copy;
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}> = ({ icon: Icon, onClick, disabled = false, children }) => (
  <button
    type="button"
    onClick={onClick}
    disabled={disabled}
    className="bg-background/60 hover:bg-background inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-medium disabled:opacity-60"
  >
    <Icon aria-hidden size={15} />
    {children}
  </button>
);
