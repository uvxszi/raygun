import { ChevronRight, Ellipsis } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, KeyboardEvent as ReactKeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useBackLayer } from './backLayers';

/** One line of an app menu group (M46.9). */
export type AppMenuItem =
  | {
      kind?: 'item';
      label: string;
      /** A key hint shown on the right, e.g. "Ctrl+N". */
      hint?: string;
      disabled?: boolean;
      onSelect: () => void;
    }
  | { kind: 'separator' };

/** File, Edit, View…: a menu bar entry whose items open beside it. */
export type AppMenuGroup = { id: string; label: string; items: AppMenuItem[] };

/** Moves focus between a menu's enabled items. */
const moveFocus = (menu: HTMLElement | null, by: number) => {
  const items = [
    ...(menu?.querySelectorAll<HTMLButtonElement>(
      ':scope > * > [role="menuitem"]:not(:disabled)',
    ) ?? []),
  ];
  if (!items.length) return;
  const at = items.indexOf(document.activeElement as HTMLButtonElement);
  items[(at + by + items.length) % items.length]?.focus();
};

const itemClass =
  'flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm outline-none hover:bg-foreground/10 focus-visible:bg-foreground/10 disabled:opacity-40 disabled:hover:bg-transparent';

/**
 * The app bar's ⋯ menu (Spotify's on Windows, M46.9): File, Edit, View, Playback and Help, each
 * opening its items beside it on hover, click or the arrow keys. Escape steps back out.
 */
export const AppMenu: FC<{ groups: AppMenuGroup[]; onOpen?: () => void }> = ({
  groups,
  onOpen,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const top = useRef<HTMLDivElement>(null);
  const subs = useRef<Record<string, HTMLDivElement | null>>({});

  const close = (refocus = false) => {
    setOpen(false);
    setActive(null);
    if (refocus) button.current?.focus();
  };
  // Back closes it; Escape steps out of a submenu first, so the menu takes Escape itself.
  useBackLayer(open, () => close(), { escape: false });

  useEffect(() => {
    if (!open) return;
    const away = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) close();
    };
    const gone = () => close();
    document.addEventListener('pointerdown', away);
    window.addEventListener('blur', gone);
    window.addEventListener('resize', gone);
    return () => {
      document.removeEventListener('pointerdown', away);
      window.removeEventListener('blur', gone);
      window.removeEventListener('resize', gone);
    };
  }, [open]);

  // Opening from the keyboard lands on the first entry; a submenu on its first item.
  useEffect(() => {
    if (open && !active) moveFocus(top.current, 1);
  }, [open, active]);

  const openGroup = (id: string, focusFirst: boolean) => {
    setActive(id);
    if (focusFirst) requestAnimationFrame(() => moveFocus(subs.current[id] ?? null, 1));
  };

  const onTopKey = (event: ReactKeyboardEvent, id: string) => {
    if (event.key === 'ArrowRight' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openGroup(id, true);
    }
  };

  const onMenuKey = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const inSub = active !== null && subs.current[active]?.contains(document.activeElement);
    if (event.key === 'Escape' || (event.key === 'ArrowLeft' && inSub)) {
      event.preventDefault();
      event.stopPropagation();
      if (inSub && active) {
        const group = active;
        setActive(null);
        top.current?.querySelector<HTMLButtonElement>(`[data-group="${group}"]`)?.focus();
      } else close(true);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const menu = inSub && active ? (subs.current[active] ?? null) : top.current;
      moveFocus(menu, event.key === 'ArrowDown' ? 1 : -1);
    }
  };

  return (
    <div ref={root} className="relative">
      <button
        ref={button}
        type="button"
        aria-label={t('appBar.menu')}
        aria-haspopup="menu"
        aria-expanded={open}
        data-hint={t('appBar.menu')}
        onClick={() => {
          if (open) close();
          else {
            onOpen?.();
            setOpen(true);
          }
        }}
        className="text-muted hover:text-foreground grid size-9 place-items-center rounded-full"
      >
        <Ellipsis aria-hidden size={20} />
      </button>
      {open && (
        <div
          ref={top}
          role="menu"
          aria-label={t('appBar.menu')}
          onKeyDown={onMenuKey}
          className="bg-surface motion-pop absolute top-full left-0 z-[90] mt-1 flex w-52 flex-col rounded-lg p-1 shadow-2xl ring-1 shadow-black/40 ring-white/10"
        >
          {groups.map((group) => (
            <div key={group.id} className="relative">
              <button
                type="button"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={active === group.id}
                data-group={group.id}
                onMouseEnter={() => openGroup(group.id, false)}
                onClick={() => openGroup(group.id, true)}
                onKeyDown={(event) => onTopKey(event, group.id)}
                className={`${itemClass} ${active === group.id ? 'bg-foreground/10' : ''}`}
              >
                <span className="flex-1">{group.label}</span>
                <ChevronRight aria-hidden size={16} className="text-muted" />
              </button>
              {active === group.id && (
                <div
                  ref={(element) => {
                    subs.current[group.id] = element;
                  }}
                  role="menu"
                  aria-label={group.label}
                  className="bg-surface motion-pop absolute top-0 left-full z-[91] ml-1 flex w-64 flex-col rounded-lg p-1 shadow-2xl ring-1 shadow-black/40 ring-white/10"
                >
                  {group.items.map((item, index) =>
                    item.kind === 'separator' ? (
                      <div
                        key={index}
                        role="separator"
                        className="bg-foreground/10 mx-2 my-1 h-px"
                      />
                    ) : (
                      <div key={index}>
                        <button
                          type="button"
                          role="menuitem"
                          disabled={item.disabled}
                          onClick={() => {
                            close();
                            item.onSelect();
                          }}
                          className={itemClass}
                        >
                          <span className="min-w-0 flex-1 truncate">{item.label}</span>
                          {item.hint && (
                            <span className="text-muted shrink-0 pl-4 text-xs">{item.hint}</span>
                          )}
                        </button>
                      </div>
                    ),
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
