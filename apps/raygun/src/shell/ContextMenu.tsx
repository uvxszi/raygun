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
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { Artwork } from '../library/Artwork';
import { useCloseOnBack } from '../player/useCloseOnBack';
import { menuFromEvent, useContextMenuStore } from './contextMenuStore';
import type { MenuItem } from './contextMenuStore';
import { useIsDesktop } from './useIsDesktop';
import { toggleWindowFullscreen } from './useWindowFullscreen';

/** A touch screen: menus come up from the bottom as a sheet, with rows a finger can hit. */
const coarsePointer = () => window.matchMedia?.('(pointer: coarse)').matches ?? false;

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => undefined);

/** An address only this app can open: its own pages, or its proxy on this device. */
const internalAddress = (address: string) => {
  try {
    const url = new URL(address);
    return (
      url.origin === window.location.origin ||
      ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname) ||
      !/^https?:$/.test(url.protocol)
    );
  } catch {
    return true;
  }
};

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
  // Links out of the app; Raygun's own pages (album, artist…) have addresses only it can open.
  const link = target?.closest('a[href]');
  if (link instanceof HTMLAnchorElement && !internalAddress(link.href)) {
    if (items.length) items.push({ kind: 'separator' });
    items.push(
      { label: t('menu.copyLink'), icon: LinkIcon, onSelect: () => copy(link.href) },
      {
        label: t('menu.openLink'),
        icon: ExternalLink,
        onSelect: () => window.open(link.href, '_blank', 'noopener'),
      },
    );
  }
  // Covers come through Raygun's own image proxy (loopback): their addresses mean nothing elsewhere.
  const image = target instanceof HTMLImageElement ? target : null;
  if (image?.src && !internalAddress(image.src)) {
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
 * images; the page adds Back, Forward, Reload and (on desktop) full screen. The webview's menu
 * never shows.
 */
export const ContextMenuHost: FC = () => {
  const { t } = useTranslation();
  const router = useRouter();
  const navigate = useNavigate();
  const open = useContextMenuStore((state) => state.open);
  const show = useContextMenuStore((state) => state.show);
  const close = useContextMenuStore((state) => state.close);
  const menu = useRef<HTMLDivElement>(null);
  // A touch that moved or scrolled the list is a scroll, never a tap on the row under it.
  const press = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  // Closing on the backdrop's click alone would close a drag that started in the menu.
  const pressedBackdrop = useRef(false);
  // A phone app is full screen already: only a desktop window toggles it.
  const desktop = useIsDesktop();
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  const [active, setActive] = useState(-1);
  // An item that asks first (M46.45): the menu becomes its question.
  const [confirming, setConfirming] = useState<Extract<MenuItem, { onSelect: unknown }> | null>(
    null,
  );

  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      // Our menu's own right clicks (and long presses on the sheet's backdrop) do nothing.
      if (
        menu.current?.contains(event.target as Node) ||
        (event.target as Element | null)?.closest?.('[data-menu-backdrop]')
      ) {
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
      const touch = coarsePointer();
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
              ...(desktop
                ? [
                    {
                      label: t('menu.fullscreen'),
                      icon: Maximize,
                      hint: 'F11',
                      onSelect: () => void toggleWindowFullscreen(),
                    },
                  ]
                : []),
            ];
      const groups = [own, spot, page].filter((group) => group.length > 0);
      const items = groups.flatMap((group, index) =>
        index === 0 ? group : [{ kind: 'separator' } as MenuItem, ...group],
      );
      setActive(-1);
      setConfirming(null);
      setPlace(null);
      show(event.clientX, event.clientY, items);
    };
    window.addEventListener('contextmenu', onContextMenu);
    return () => window.removeEventListener('contextmenu', onContextMenu);
  }, [navigate, router, show, t, desktop]);

  // On a touch screen the menu is a sheet from the bottom, and Back closes it.
  const sheet = useMemo(() => open !== null && coarsePointer(), [open]);
  useCloseOnBack(sheet, close);

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
      // The sheet's backdrop closes it on its click, so the tap doesn't land on the page.
      if (sheet) return;
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
  }, [open, close, sheet]);

  if (!open) return null;
  const choosable = open.items
    .map((item, index) => ({ item, index }))
    .filter(
      ({ item }) => (item.kind ?? 'item') === 'item' && !('disabled' in item && item.disabled),
    );
  const pick = (item: MenuItem) => {
    if ((item.kind ?? 'item') !== 'item' || !('onSelect' in item)) return;
    if (item.confirm && confirming !== item) {
      setConfirming(item);
      return;
    }
    close();
    item.onSelect();
  };
  const step = (by: number) => {
    const at = choosable.findIndex(({ index }) => index === active);
    const next = choosable[(at + by + choosable.length) % choosable.length];
    if (next) setActive(next.index);
  };

  const moved = () => press.current?.moved === true;
  const renderItem = (item: MenuItem, index: number) => {
    if (item.kind === 'separator') {
      return (
        <div key={index} role="separator" className="bg-foreground/10 mx-2 my-1 h-px shrink-0" />
      );
    }
    if (item.kind === 'heading') {
      return sheet ? (
        <div key={index} className="flex min-w-0 shrink-0 items-center gap-3 px-4 pt-2 pb-3">
          {item.artworkUrl !== undefined && (
            <Artwork
              url={item.artworkUrl}
              className="size-[clamp(3rem,14vw,3.5rem)] shrink-0 rounded-md"
              iconSize={22}
            />
          )}
          <span className="flex min-w-0 flex-col">
            <span className="truncate text-[clamp(1rem,4.4vw,1.125rem)] font-semibold">
              {item.label}
            </span>
            {item.subtitle && (
              <span className="text-muted truncate text-[clamp(0.875rem,3.8vw,1rem)]">
                {item.subtitle}
              </span>
            )}
          </span>
        </div>
      ) : (
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
        onClick={(event) => {
          // Keyboard presses have no pointer (detail 0) and always choose.
          if (sheet && event.detail !== 0 && moved()) return;
          pick(item);
        }}
        className={`flex shrink-0 items-center text-left disabled:opacity-40 ${
          sheet
            ? 'min-h-[max(3rem,7dvh)] gap-4 rounded-xl px-4 py-2 text-[clamp(1rem,4.3vw,1.125rem)]'
            : 'gap-3 rounded-lg px-3 py-1.5 text-sm'
        } ${index === active ? 'bg-primary/15 text-foreground' : ''} ${
          item.danger ? 'text-primary' : ''
        }`}
      >
        <span className={`grid shrink-0 place-items-center ${sheet ? 'size-6' : 'size-4'}`}>
          {Icon && (
            <Icon
              aria-hidden
              size={sheet ? 22 : 15}
              className={index === active ? 'text-primary' : 'text-muted'}
            />
          )}
        </span>
        <span className="min-w-0 flex-1 truncate">{item.label}</span>
        {item.hint && !sheet && (
          <span className="text-muted shrink-0 pl-4 text-xs">{item.hint}</span>
        )}
      </button>
    );
  };
  // On a sheet the headings it starts with (cover, title) stay put while the rows scroll.
  const leading =
    sheet && !confirming ? open.items.findIndex((item) => item.kind !== 'heading') : 0;
  const headerCount = leading < 0 ? open.items.length : leading;
  const rows = open.items.map((item, index) => ({ item, index })).slice(headerCount);

  const panel = (
    <div
      ref={menu}
      role="menu"
      aria-label={t('menu.title')}
      tabIndex={-1}
      data-testid="context-menu"
      data-sheet={sheet ? '' : undefined}
      onKeyDown={(event) => {
        if (event.key === 'Escape') close();
        else if (event.key === 'ArrowDown') step(1);
        else if (event.key === 'ArrowUp') step(-1);
        else if (event.key === 'Enter' && active >= 0) pick(open.items[active] as MenuItem);
        else return;
        event.preventDefault();
      }}
      onPointerDown={(event) => {
        press.current = { x: event.clientX, y: event.clientY, moved: false };
      }}
      onPointerMove={(event) => {
        const at = press.current;
        if (at && !at.moved && Math.hypot(event.clientX - at.x, event.clientY - at.y) > 10) {
          at.moved = true;
        }
      }}
      style={sheet ? undefined : { left: place?.left ?? open.x, top: place?.top ?? open.y }}
      className={
        sheet
          ? 'bg-surface motion-rise flex max-h-[85dvh] w-full flex-col overflow-hidden rounded-t-3xl pt-2 outline-none sm:max-w-md'
          : 'bg-surface motion-pop fixed z-[90] flex max-h-[80vh] min-w-56 flex-col overflow-y-auto rounded-xl p-1.5 shadow-2xl ring-1 shadow-black/40 ring-white/10 outline-none'
      }
    >
      {confirming ? (
        <div
          role="alertdialog"
          aria-label={confirming.confirm}
          className={`flex flex-col gap-3 ${
            sheet ? 'px-4 pt-2 pb-[calc(1rem+env(safe-area-inset-bottom))]' : 'max-w-72 p-2'
          }`}
        >
          <p className={sheet ? 'text-[clamp(1rem,4.3vw,1.125rem)]' : 'text-sm'}>
            {confirming.confirm}
          </p>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              autoFocus
              onClick={close}
              className={`hover:bg-background rounded-full px-3 ${sheet ? 'min-h-12 px-5 text-base' : 'py-1.5 text-sm'}`}
            >
              {t('collections.cancel')}
            </button>
            <button
              type="button"
              onClick={() => pick(confirming)}
              className={`bg-primary text-background rounded-full px-4 font-semibold ${sheet ? 'min-h-12 px-6 text-base' : 'py-1.5 text-sm'}`}
            >
              {confirming.label}
            </button>
          </div>
        </div>
      ) : sheet ? (
        <>
          {open.items.slice(0, headerCount).map((item, index) => renderItem(item, index))}
          <div
            data-testid="context-menu-rows"
            onScroll={() => {
              if (press.current) press.current.moved = true;
            }}
            className="flex min-h-0 flex-1 touch-pan-y flex-col overflow-y-auto overscroll-contain px-2 pb-[calc(0.75rem+env(safe-area-inset-bottom))] [-webkit-overflow-scrolling:touch]"
          >
            {rows.map(({ item, index }) => renderItem(item, index))}
          </div>
        </>
      ) : (
        open.items.map((item, index) => renderItem(item, index))
      )}
    </div>
  );

  return createPortal(
    sheet ? (
      <div
        data-menu-backdrop=""
        className="motion-fade fixed inset-0 z-[90] flex items-end justify-center bg-black/60"
        onPointerDown={(event) => {
          pressedBackdrop.current = event.target === event.currentTarget;
        }}
        onClick={(event) => {
          if (pressedBackdrop.current && event.target === event.currentTarget) close();
          pressedBackdrop.current = false;
        }}
      >
        {panel}
      </div>
    ) : (
      panel
    ),
    document.body,
  );
};
