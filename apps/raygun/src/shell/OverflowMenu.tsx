import { useNavigate } from '@tanstack/react-router';
import { EllipsisVertical } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { openMenuAt } from './contextMenuStore';
import type { MenuItem } from './contextMenuStore';
import { useBackLayer } from './backLayers';

export type OverflowItem = {
  label: string;
  icon?: LucideIcon;
  onSelect?: () => void;
  /** Navigates instead of running `onSelect`. */
  to?: string;
  search?: Record<string, unknown>;
  /** Asked first, in the item's place: a second tap does it ("Delete 12 downloads"). */
  confirm?: string;
};

/** A touch screen: the menu is the shell's menu sheet, full width with rows a finger can hit. */
const coarsePointer = () => window.matchMedia?.('(pointer: coarse)').matches ?? false;

/**
 * The top bar's ⋮ menu: actions that don't earn an icon of their own. In a list, `label` names
 * what the menu is for.
 */
export const OverflowMenu: FC<{ items: OverflowItem[]; label?: string }> = ({ items, label }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [asking, setAsking] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  // Back and Escape close it before anything under it.
  useBackLayer(open, () => setOpen(false));

  useEffect(() => {
    if (!open) return;
    const close = (event: Event) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);

  // `confirmed`: the touch sheet already asked.
  const select = (item: OverflowItem, confirmed = false) => {
    if (item.confirm && !confirmed && asking !== item.label) {
      setAsking(item.label);
      return;
    }
    setAsking(null);
    setOpen(false);
    if (item.to) {
      void navigate({ to: item.to, search: item.search });
    } else {
      item.onSelect?.();
    }
  };

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        aria-label={label ?? t('nav.moreActions')}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={(event) => {
          setAsking(null);
          if (coarsePointer()) {
            const menu: MenuItem[] = items.map((item) => ({
              label: item.label,
              icon: item.icon,
              confirm: item.confirm,
              onSelect: () => select(item, true),
            }));
            openMenuAt(event.currentTarget, menu);
            return;
          }
          setOpen(!open);
        }}
        className="hover:bg-surface grid size-11 place-items-center rounded-full"
      >
        <EllipsisVertical aria-hidden size={22} />
      </button>
      {open && (
        <ul
          id={menuId}
          role="menu"
          className="bg-surface absolute top-12 right-0 z-40 flex min-w-56 flex-col rounded-xl py-2 shadow-xl"
        >
          {items.map((item) => {
            const askingHere = asking === item.label && Boolean(item.confirm);
            return (
              <li key={item.label} role="none">
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => select(item)}
                  className={`hover:bg-background flex min-h-11 w-full items-center gap-3 px-4 py-3 text-left text-sm pointer-coarse:min-h-12 ${askingHere ? 'text-primary font-semibold' : ''}`}
                >
                  {item.icon && (
                    <item.icon aria-hidden size={18} className={askingHere ? '' : 'text-muted'} />
                  )}
                  {askingHere ? item.confirm : item.label}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};
