import type { LucideIcon } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { afterLayersClose } from './backLayers';
import { Popover } from './Popover';

/** One line of a quick access item's menu. */
export type QuickMenuItem =
  | {
      kind?: 'item';
      label: string;
      icon?: LucideIcon;
      /** A key hint, as other menus have; not shown here. */
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      /** Asked first, in the menu's place ("Remove “Road trip” from your library?"). */
      confirm?: string;
      onSelect: () => void;
    }
  | { kind: 'separator' }
  | { kind: 'heading'; label: string; subtitle?: string | null; artworkUrl?: string | null };

type Choice = Extract<QuickMenuItem, { onSelect: unknown }>;

/**
 * The menu of a quick access item (a right click on desktop, a hold or ⋮ in the phone's drawer),
 * beside it. An item that asks first turns the menu into its question, with Cancel.
 */
export const QuickAccessMenu: FC<{
  anchor: HTMLElement;
  label: string;
  items: QuickMenuItem[];
  onClose: () => void;
}> = ({ anchor, label, items, onClose }) => {
  const { t } = useTranslation();
  const [asking, setAsking] = useState<Choice | null>(null);
  const list = useRef<HTMLDivElement>(null);

  // The first item takes the focus once the popover has, so the arrow keys work at once.
  useEffect(() => {
    if (asking) return;
    const timer = window.setTimeout(() =>
      list.current
        ?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')
        ?.focus({ preventScroll: true }),
    );
    return () => window.clearTimeout(timer);
  }, [asking]);

  const choose = (item: Choice, confirmed = false) => {
    if (item.confirm && !confirmed) {
      setAsking(item);
      return;
    }
    onClose();
    // After the menu's history entry is gone (phones): a page it opens then stays open.
    afterLayersClose(item.onSelect);
  };

  // Up and down move between the items, as in other menus.
  const step = (event: KeyboardEvent) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const buttons = [
      ...(list.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ??
        []),
    ];
    if (buttons.length === 0) return;
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === 'ArrowDown'
        ? (at + 1) % buttons.length
        : (at - 1 + buttons.length) % buttons.length;
    buttons[at < 0 ? 0 : next]?.focus();
  };

  return (
    <Popover anchor={anchor} label={label} onClose={onClose} width={264}>
      {asking ? (
        <div role="alertdialog" aria-label={asking.confirm} className="flex flex-col gap-3 p-2">
          <p className="text-sm">{asking.confirm}</p>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              autoFocus
              onClick={onClose}
              className="hover:bg-background rounded-full px-3 py-1.5 text-sm"
            >
              {t('collections.cancel')}
            </button>
            <button
              type="button"
              onClick={() => choose(asking, true)}
              className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold"
            >
              {asking.label}
            </button>
          </div>
        </div>
      ) : (
        <div
          ref={list}
          role="menu"
          aria-label={label}
          data-testid="quick-access-menu"
          onKeyDown={step}
          className="flex flex-col"
        >
          {items.map((item, index) => {
            if (item.kind === 'separator') {
              return (
                <div
                  key={index}
                  role="separator"
                  className="bg-foreground/10 mx-2 my-1 h-px shrink-0"
                />
              );
            }
            if (item.kind === 'heading') {
              return (
                <div
                  key={index}
                  className="text-muted truncate px-3 pt-1.5 pb-1 text-xs font-semibold"
                >
                  {item.label}
                </div>
              );
            }
            const Icon = item.icon;
            return (
              <button
                key={index}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                onClick={() => choose(item)}
                className={`hover:bg-background focus-visible:bg-background flex min-h-10 shrink-0 items-center gap-3 rounded-lg px-3 py-2 text-left text-sm outline-none disabled:opacity-40 pointer-coarse:min-h-12 ${
                  item.danger ? 'text-primary' : ''
                }`}
              >
                <span className="grid size-4 shrink-0 place-items-center">
                  {Icon && <Icon aria-hidden size={15} className="text-muted" />}
                </span>
                <span className="min-w-0 flex-1 truncate">{item.label}</span>
              </button>
            );
          })}
        </div>
      )}
    </Popover>
  );
};
