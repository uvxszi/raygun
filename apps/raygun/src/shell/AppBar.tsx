import type { WindowFrame } from '@raygun/bindings';
import { Link, useNavigate, useRouter, useRouterState } from '@tanstack/react-router';
import type { RecentSearch } from '@raygun/bindings';
import {
  ChevronLeft,
  ChevronRight,
  Copy,
  History,
  House,
  Inbox,
  Minus,
  Search,
  Square,
  X,
  RotateCw,
} from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useId, useRef, useState } from 'react';
import type { FC, RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { NewPlaylistForm } from '../collections/PlaylistsView';
import { useImportComics } from '../manga/useImportComics';
import { useImportEpub } from '../novel/useImportEpub';
import { SEARCH_DEBOUNCE_MS } from '../pages/BrowsePage';
import { IncognitoBadge } from '../privacy/IncognitoBadge';
import { kindOf, useRecentSearches, useRememberSearch } from '../search/useRecentSearches';
import { isSearchType } from '../search/searchTypes';
import type { SearchType } from '../search/searchTypes';
import { TextField } from './TextField';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { AppMenu } from './AppMenu';
import { MascotButton } from './MascotButton';
import { SmartChipsButton } from './SmartChipsButton';
import { closeWindow, minimizeWindow, toggleMaximizeWindow, useWindowState } from './appWindow';
import { backFromLayers, useDesktopBack, useLayersOpen } from './backLayers';
import { useAppMenuGroups } from './useAppMenuGroups';
import { useHistoryNav } from './useHistoryNav';
import { BarPopoverButtons } from './BarPopovers';

type BrowseSearch = { tab?: string; q?: string; type?: string };

const roundButton =
  'grid size-9 shrink-0 place-items-center rounded-full text-muted hover:text-foreground disabled:opacity-30 disabled:hover:text-muted';

/**
 * The search field in the middle of the bar: typing searches everything (Browse → Search), and
 * it follows the address, so Back shows what was searched. The button at its end opens Browse.
 * Focused while empty, it lists the searches made lately: the arrow keys move through them,
 * Enter searches again. Escape clears what's typed, then leaves the field.
 */
const AppSearch: FC<{ inputRef: RefObject<HTMLInputElement | null> }> = ({ inputRef }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useRouterState({ select: (state) => state.location });
  const search = location.search as BrowseSearch;
  const searching = location.pathname === '/browse' && search.tab === 'search';
  const q = searching ? (search.q ?? '') : '';
  const kind = searching && isSearchType(search.type) ? search.type : 'all';
  const remember = useRememberSearch();
  const recent = useRecentSearches();
  const [input, setInput] = useState(q);
  // The address's query last seen, and the one this field last searched for.
  const [shown, setShown] = useState(q);
  const [sent, setSent] = useState(q);
  const [focused, setFocused] = useState(false);
  const [picked, setPicked] = useState(-1);
  const listId = useId();
  if (q !== shown) {
    setShown(q);
    // A search changed elsewhere (Back, a link) shows here; this field's own search arriving
    // leaves what's been typed since alone.
    if (q !== sent) {
      setSent(q);
      setInput(q);
    }
  }

  const go = (text: string, type?: SearchType) => {
    const next = text.trim();
    if (next === q && (!type || type === kind)) return;
    setSent(next);
    void navigate({
      to: '/browse',
      search: {
        ...(searching ? search : {}),
        tab: 'search',
        q: next || undefined,
        ...(type ? { type: type === 'all' ? undefined : type, view: undefined } : {}),
      },
      // Refining a search keeps one history entry; starting one from elsewhere adds one.
      replace: searching,
    });
  };

  useEffect(() => {
    if (input.trim() === q) return;
    const timer = setTimeout(() => go(input), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  // The searches made lately, while the field is focused and empty.
  const kept = focused && input === '' ? (recent.data ?? []).slice(0, 8) : [];
  const choose = (choice: RecentSearch) => {
    const type = kindOf(choice);
    setInput(choice.query);
    setPicked(-1);
    remember(choice.query, type);
    go(choice.query, type);
  };

  return (
    <div className="relative flex h-11 max-w-[480px] min-w-0 flex-1">
      <TextField
        ref={inputRef}
        type="search"
        value={input}
        onChange={(value) => {
          setInput(value);
          setPicked(-1);
        }}
        onFocus={() => setFocused(true)}
        onBlur={() => {
          setFocused(false);
          setPicked(-1);
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            if (kept.length === 0) return;
            event.preventDefault();
            // Down past the last, or up past the first, comes back to what's typed.
            const step = event.key === 'ArrowDown' ? 1 : -1;
            setPicked((at) => {
              const next = at + step;
              if (next >= kept.length) return -1;
              return next < -1 ? kept.length - 1 : next;
            });
            return;
          }
          if (event.key === 'Enter') {
            const choice = kept[picked];
            if (choice) {
              choose(choice);
              return;
            }
            remember(input, kind);
            go(input);
          }
          if (event.key === 'Escape') event.currentTarget.blur();
        }}
        icon={Search}
        iconSize={20}
        autoComplete="off"
        aria-controls={kept.length > 0 ? listId : undefined}
        aria-activedescendant={picked >= 0 ? `${listId}-${picked}` : undefined}
        placeholder={t('appBar.searchPlaceholder')}
        aria-label={t('search.title')}
        className="bg-surface focus-within:ring-foreground/60 hover:ring-foreground/20 w-full gap-3 rounded-full pr-1.5 pl-4 ring-1 ring-transparent"
        inputClassName="h-full"
        trailing={
          <>
            <span aria-hidden className="bg-foreground/20 h-6 w-px shrink-0" />
            <Link
              to="/browse"
              search={{ tab: 'discover' }}
              aria-label={t('appBar.browse')}
              data-hint={t('appBar.browse')}
              className={roundButton}
            >
              <Inbox aria-hidden size={20} />
            </Link>
          </>
        }
      />
      {kept.length > 0 && (
        <ul
          id={listId}
          role="listbox"
          aria-label={t('search.recent.title')}
          className="bg-surface motion-pop absolute top-full right-0 left-0 z-50 mt-1 flex flex-col rounded-2xl py-1.5 shadow-xl shadow-black/30"
        >
          {kept.map((choice, index) => {
            const type = kindOf(choice);
            return (
              <li
                key={`${choice.kind}:${choice.query}`}
                id={`${listId}-${index}`}
                role="option"
                aria-selected={index === picked}
                // Chosen on press, before the field loses focus and the list goes.
                onMouseDown={(event) => {
                  event.preventDefault();
                  choose(choice);
                }}
                className={`flex cursor-pointer items-center gap-3 px-4 py-2 ${index === picked ? 'bg-foreground/10' : 'hover:bg-foreground/5'}`}
              >
                <History aria-hidden size={16} className="text-muted shrink-0" />
                <span className="truncate">{choice.query}</span>
                {type !== 'all' && (
                  <span className="text-muted ml-auto shrink-0 text-xs">
                    {t(`search.type.${type}`)}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};

/** Minimize, maximize or restore, and close, drawn as Windows draws them. */
const WindowButtons: FC = () => {
  const { t } = useTranslation();
  const { maximized } = useWindowState();
  const button = 'grid h-full w-11 place-items-center text-muted hover:text-foreground';
  return (
    <div className="-mr-2 ml-1 flex h-full self-stretch">
      <button
        type="button"
        aria-label={t('appBar.minimize')}
        onClick={() => void minimizeWindow()}
        className={`${button} hover:bg-foreground/10`}
      >
        <Minus aria-hidden size={16} />
      </button>
      <button
        type="button"
        aria-label={t(maximized ? 'appBar.restore' : 'appBar.maximize')}
        onClick={() => void toggleMaximizeWindow()}
        className={`${button} hover:bg-foreground/10`}
      >
        {maximized ? <Copy aria-hidden size={13} /> : <Square aria-hidden size={13} />}
      </button>
      <button
        type="button"
        aria-label={t('appBar.close')}
        onClick={() => void closeWindow()}
        className={`${button} hover:bg-[#c42b1c] hover:text-white`}
      >
        <X aria-hidden size={17} />
      </button>
    </div>
  );
};

/**
 * The desktop app's top bar (Spotify's, M46.9): the ⋯ menu, Back and Forward, Home, the search
 * field with Browse at its end, Updates and Downloads, and with Raygun's own frame the window's
 * buttons. Empty parts of it move the window; a double click maximizes it.
 */
export const AppBar: FC<{ frame: WindowFrame; onShortcuts: () => void; immersive: boolean }> = ({
  frame,
  onShortcuts,
  immersive,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const nav = useHistoryNav();
  // Back closes what's open over the page first (a menu, a sheet, the full player): the bar's
  // button, the mouse's and Alt+Left alike.
  const layersOpen = useLayersOpen();
  const back = () => backFromLayers(nav.back);
  useDesktopBack(nav.back, nav.forward);
  const router = useRouter();
  const client = useQueryClient();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const searchRef = useRef<HTMLInputElement>(null);
  const [creating, setCreating] = useState(false);
  const comics = useImportComics();
  const epub = useImportEpub();
  const zoomNow = settings.data?.zoom ?? 100;
  const zoom = (by: number | null) => {
    const next = by === null ? 100 : Math.min(200, Math.max(50, zoomNow + by));
    if (next !== zoomNow) updateSetting.mutate({ key: 'zoom', value: next });
  };
  const focusSearch = () => {
    searchRef.current?.focus();
    searchRef.current?.select();
  };
  const { groups, snapshot } = useAppMenuGroups({
    newPlaylist: () => setCreating(true),
    importComics: comics.pick,
    importBook: epub.pick,
    focusSearch,
    zoom,
    shortcuts: onShortcuts,
  });

  // The menu's shortcuts, everywhere on desktop.
  const keys = useRef({ focusSearch, zoom, navigate, immersive });
  useEffect(() => {
    keys.current = { focusSearch, zoom, navigate, immersive };
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey || event.defaultPrevented) return;
      const run = keys.current;
      // The readers zoom their pages with the same keys (M46.7).
      if (run.immersive && ['=', '+', '-', '_', '0'].includes(event.key)) return;
      const act = (() => {
        switch (event.key) {
          case 'k':
          case 'K':
            return run.focusSearch;
          case 'n':
          case 'N':
            return () => setCreating(true);
          case ',':
            return () => void run.navigate({ to: '/settings' });
          case '=':
          case '+':
            return () => run.zoom(10);
          case '-':
          case '_':
            return () => run.zoom(-10);
          case '0':
            return () => run.zoom(null);
          default:
            return null;
        }
      })();
      if (!act) return;
      event.preventDefault();
      act();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  // Ctrl+scroll zooms too (M46.24), a step per wheel notch; the readers zoom their pages with it.
  useEffect(() => {
    let pending = 0;
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey || keys.current.immersive) return;
      event.preventDefault();
      pending += event.deltaY;
      if (Math.abs(pending) < 50) return;
      keys.current.zoom(pending < 0 ? 10 : -10);
      pending = 0;
    };
    window.addEventListener('wheel', onWheel, { passive: false });
    return () => window.removeEventListener('wheel', onWheel);
  }, []);

  const importing = comics.importFiles.isPending || epub.importBook.isPending;
  const importError = comics.importFiles.error ?? epub.importBook.error;

  return (
    <header
      data-tauri-drag-region="deep"
      data-testid="app-bar"
      className="bg-background relative z-50 flex h-16 shrink-0 items-center gap-1 pl-2 select-none"
    >
      <AppMenu groups={groups} onOpen={snapshot} />
      <button
        type="button"
        aria-label={t('menu.back')}
        data-hint={t('menu.back')}
        disabled={!nav.canGoBack && !layersOpen}
        onClick={back}
        className={roundButton}
      >
        <ChevronLeft aria-hidden size={22} />
      </button>
      <button
        type="button"
        aria-label={t('menu.forward')}
        data-hint={t('menu.forward')}
        disabled={!nav.canGoForward}
        onClick={nav.forward}
        className={roundButton}
      >
        <ChevronRight aria-hidden size={22} />
      </button>
      <button
        type="button"
        aria-label={t('menu.reload')}
        data-hint={t('menu.reload')}
        onClick={() => {
          void router.invalidate();
          void client.invalidateQueries();
        }}
        className={roundButton}
      >
        <RotateCw aria-hidden size={18} />
      </button>
      <div className="flex min-w-0 flex-1 items-center justify-center gap-2 px-2">
        <Link
          to="/"
          aria-label={t('nav.home')}
          data-hint={t('nav.home')}
          activeOptions={{ exact: true }}
          className="bg-surface text-muted hover:text-foreground data-[status=active]:text-foreground grid size-11 shrink-0 place-items-center rounded-full"
        >
          <House aria-hidden size={21} />
        </Link>
        <AppSearch inputRef={searchRef} />
      </div>
      <IncognitoBadge labelled className="mr-1" />
      <MascotButton className={roundButton} iconSize={20} />
      <SmartChipsButton className={roundButton} iconSize={20} />
      <BarPopoverButtons className={roundButton} />
      {frame === 'app' ? <WindowButtons /> : <span className="w-2" />}
      {creating && <NewPlaylistForm onDone={() => setCreating(false)} />}
      {comics.input}
      {epub.input}
      {(importing || importError) && (
        <p
          role={importError ? 'alert' : 'status'}
          className="bg-surface absolute top-full left-1/2 z-50 mt-1 -translate-x-1/2 rounded-full px-4 py-1.5 text-sm shadow-lg"
        >
          {importError ? importError.message : t('appBar.importing')}
        </p>
      )}
    </header>
  );
};
