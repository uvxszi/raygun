import { useNavigate, useRouter } from '@tanstack/react-router';
import {
  ArrowLeft,
  ArrowRight,
  ClipboardPaste,
  Copy,
  ExternalLink,
  Image as ImageIcon,
  Link as LinkIcon,
  Maximize,
  RotateCw,
  Scissors,
  Search,
  TextSelect,
} from 'lucide-react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { menuFromEvent, useContextMenuStore } from './contextMenuStore';
import type { MenuItem } from './contextMenuStore';
import { toggleWindowFullscreen } from './useWindowFullscreen';

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => undefined);

const editable = (target: EventTarget | null): HTMLInputElement | HTMLTextAreaElement | null => {
  if (target instanceof HTMLTextAreaElement) return target;
  if (
    target instanceof HTMLInputElement &&
    !['checkbox', 'radio', 'range', 'color'].includes(target.type)
  ) {
    return target;
  }
  return null;
};

/**
 * What every right click offers for the spot it's on: fields, selected text, links, images.
 * `words` adds the text under the pointer; it's left out when the menu already copies (a song's
 * Copy title and Copy artist would otherwise show twice, M46.36).
 */
const spotItems = (
  event: MouseEvent,
  t: TFunction,
  search: (text: string) => void,
  words: boolean,
): MenuItem[] => {
  const items: MenuItem[] = [];
  const target = event.target as Element | null;
  const field = editable(target);
  const selected = (
    field
      ? field.value.slice(field.selectionStart ?? 0, field.selectionEnd ?? 0)
      : (window.getSelection()?.toString() ?? '')
  ).trim();
  if (field) {
    items.push(
      {
        label: t('menu.cut'),
        icon: Scissors,
        hint: 'Ctrl+X',
        disabled: !selected,
        onSelect: () => {
          copy(selected);
          field.setRangeText('', field.selectionStart ?? 0, field.selectionEnd ?? 0, 'end');
          field.dispatchEvent(new Event('input', { bubbles: true }));
        },
      },
      {
        label: t('menu.copy'),
        icon: Copy,
        hint: 'Ctrl+C',
        disabled: !selected,
        onSelect: () => copy(selected),
      },
      {
        label: t('menu.paste'),
        icon: ClipboardPaste,
        hint: 'Ctrl+V',
        onSelect: () => {
          void navigator.clipboard
            ?.readText()
            .then((text) => {
              field.focus();
              field.setRangeText(text, field.selectionStart ?? 0, field.selectionEnd ?? 0, 'end');
              field.dispatchEvent(new Event('input', { bubbles: true }));
            })
            .catch(() => undefined);
        },
      },
      {
        label: t('menu.selectAll'),
        icon: TextSelect,
        hint: 'Ctrl+A',
        onSelect: () => field.select(),
      },
    );
  } else if (selected) {
    items.push(
      { label: t('menu.copy'), icon: Copy, hint: 'Ctrl+C', onSelect: () => copy(selected) },
      {
        label: t('menu.searchFor', {
          text: selected.length > 28 ? `${selected.slice(0, 28)}…` : selected,
        }),
        icon: Search,
        onSelect: () => search(selected),
      },
    );
  }
  // Text isn't selectable in the app, so the words under the pointer can be copied as they are.
  if (words && !field && !selected) {
    const spot = target?.closest(
      'h1, h2, h3, h4, h5, h6, p, span, a, li, td, th, label, button, dd, dt',
    );
    // As shown: a title and an artist on two lines stay two words apart.
    const text = (
      spot instanceof HTMLElement ? spot.innerText || spot.textContent : spot?.textContent
    )
      ?.replace(/\s+/g, ' ')
      .trim();
    if (text && text.length <= 300) {
      items.push({
        label: t('menu.copyText', {
          text: text.length > 28 ? `${text.slice(0, 28)}…` : text,
        }),
        icon: Copy,
        onSelect: () => copy(text),
      });
    }
  }
  const link = target?.closest('a[href]');
  if (link instanceof HTMLAnchorElement && /^https?:/.test(link.href)) {
    if (items.length) items.push({ kind: 'separator' });
    items.push({ label: t('menu.copyLink'), icon: LinkIcon, onSelect: () => copy(link.href) });
    if (new URL(link.href).origin !== window.location.origin) {
      items.push({
        label: t('menu.openLink'),
        icon: ExternalLink,
        onSelect: () => window.open(link.href, '_blank', 'noopener'),
      });
    }
  }
  const image = target instanceof HTMLImageElement ? target : null;
  if (image?.src) {
    if (items.length) items.push({ kind: 'separator' });
    items.push({
      label: t('menu.copyImageAddress'),
      icon: ImageIcon,
      onSelect: () => copy(image.src),
    });
  }
  return items;
};

/**
 * Raygun's own right-click menu (M46.8), in the theme's colours and accent. Whatever is under the
 * pointer adds its options (a song, a cover, a source); the spot adds fields, text, links and
 * images; the page adds Back, Forward, Reload and full screen. The webview's menu never shows.
 */
export const ContextMenuHost: FC = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const navigate = useNavigate();
  const open = useContextMenuStore((state) => state.open);
  const show = useContextMenuStore((state) => state.show);
  const close = useContextMenuStore((state) => state.close);
  const menu = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  const [active, setActive] = useState(-1);

  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      // Our menu's own right clicks do nothing.
      if (menu.current?.contains(event.target as Node)) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      const own = menuFromEvent(event) ?? [];
      const copies = own.some(
        (item) => (item.kind ?? 'item') === 'item' && 'icon' in item && item.icon === Copy,
      );
      const spot = spotItems(
        event,
        t,
        (text) => void navigate({ to: '/browse', search: { tab: 'search', q: text } as never }),
        !copies,
      );
      // On a touch screen a long press on something shows only its own options: Back, Forward,
      // Reload and full screen are what the page's own buttons do there.
      const touch = window.matchMedia?.('(pointer: coarse)').matches ?? false;
      const page: MenuItem[] =
        touch && own.length > 0
          ? []
          : [
              {
                label: t('menu.back'),
                icon: ArrowLeft,
                hint: 'Alt+←',
                onSelect: () => router.history.back(),
              },
              {
                label: t('menu.forward'),
                icon: ArrowRight,
                hint: 'Alt+→',
                onSelect: () => router.history.forward(),
              },
              { label: t('menu.reload'), icon: RotateCw, onSelect: () => void router.invalidate() },
              {
                label: t('menu.fullscreen'),
                icon: Maximize,
                hint: 'F11',
                onSelect: () => void toggleWindowFullscreen(),
              },
            ];
      const groups = [own, spot, page].filter((group) => group.length > 0);
      const items = groups.flatMap((group, index) =>
        index === 0 ? group : [{ kind: 'separator' } as MenuItem, ...group],
      );
      setActive(-1);
      setPlace(null);
      show(event.clientX, event.clientY, items);
    };
    window.addEventListener('contextmenu', onContextMenu);
    return () => window.removeEventListener('contextmenu', onContextMenu);
  }, [navigate, router, show, t]);

  // Kept on screen: flipped up or left when it would run off the edge.
  useLayoutEffect(() => {
    if (!open || !menu.current) return;
    const { width, height } = menu.current.getBoundingClientRect();
    const left = open.x + width > window.innerWidth - 8 ? Math.max(8, open.x - width) : open.x;
    const top = open.y + height > window.innerHeight - 8 ? Math.max(8, open.y - height) : open.y;
    setPlace({ left, top });
    menu.current.focus({ preventScroll: true });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const away = (event: PointerEvent) => {
      if (!menu.current?.contains(event.target as Node)) close();
    };
    const gone = () => close();
    // Scrolling the page closes the menu; scrolling a long menu itself doesn't (M46.40).
    const scrolled = (event: Event) => {
      if (!menu.current?.contains(event.target as Node)) close();
    };
    window.addEventListener('pointerdown', away, true);
    window.addEventListener('blur', gone);
    window.addEventListener('resize', gone);
    document.addEventListener('scroll', scrolled, true);
    return () => {
      window.removeEventListener('pointerdown', away, true);
      window.removeEventListener('blur', gone);
      window.removeEventListener('resize', gone);
      document.removeEventListener('scroll', scrolled, true);
    };
  }, [open, close]);

  if (!open) return null;
  const choosable = open.items
    .map((item, index) => ({ item, index }))
    .filter(
      ({ item }) => (item.kind ?? 'item') === 'item' && !('disabled' in item && item.disabled),
    );
  const pick = (item: MenuItem) => {
    if ((item.kind ?? 'item') !== 'item' || !('onSelect' in item)) return;
    close();
    item.onSelect();
  };
  const step = (by: number) => {
    const at = choosable.findIndex(({ index }) => index === active);
    const next = choosable[(at + by + choosable.length) % choosable.length];
    if (next) setActive(next.index);
  };

  return createPortal(
    <div
      ref={menu}
      role="menu"
      aria-label={t('menu.title')}
      tabIndex={-1}
      data-testid="context-menu"
      onKeyDown={(event) => {
        if (event.key === 'Escape') close();
        else if (event.key === 'ArrowDown') step(1);
        else if (event.key === 'ArrowUp') step(-1);
        else if (event.key === 'Enter' && active >= 0) pick(open.items[active] as MenuItem);
        else return;
        event.preventDefault();
      }}
      style={{ left: place?.left ?? open.x, top: place?.top ?? open.y }}
      className="bg-surface motion-pop fixed z-[90] flex max-h-[80vh] min-w-56 flex-col overflow-y-auto rounded-xl p-1.5 shadow-2xl ring-1 shadow-black/40 ring-white/10 outline-none"
    >
      {open.items.map((item, index) => {
        if (item.kind === 'separator') {
          return <div key={index} role="separator" className="bg-foreground/10 mx-2 my-1 h-px" />;
        }
        if (item.kind === 'heading') {
          return (
            <div key={index} className="text-muted truncate px-3 pt-1.5 pb-1 text-xs font-semibold">
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
            onMouseEnter={() => setActive(index)}
            onClick={() => pick(item)}
            className={`flex items-center gap-3 rounded-lg px-3 py-1.5 text-left text-sm disabled:opacity-40 ${
              index === active ? 'bg-primary/15 text-foreground' : ''
            } ${item.danger ? 'text-primary' : ''}`}
          >
            <span className="grid size-4 shrink-0 place-items-center">
              {Icon && (
                <Icon
                  aria-hidden
                  size={15}
                  className={index === active ? 'text-primary' : 'text-muted'}
                />
              )}
            </span>
            <span className="min-w-0 flex-1 truncate">{item.label}</span>
            {item.hint && <span className="text-muted shrink-0 pl-4 text-xs">{item.hint}</span>}
          </button>
        );
      })}
    </div>,
    document.body,
  );
};
