import { commands } from '@raygun/bindings';
import type { SystemFont } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, RefreshCw, Search } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { useListSearch } from '../shell/useListSearch';
import { useFontLabel } from './fontValue';
import type { FontValue, OwnFont } from './fontValue';
import { FONTS_KEY, quoteFamily, registerFaces, useSystemFonts } from './systemFonts';

type PickerProps = {
  title: string;
  own: OwnFont[];
  value: FontValue;
  /** What a device font falls back to, in its preview as where it's used. */
  fallback: string;
  onPick: (value: FontValue) => void;
};

/** Device fonts beyond this many get a search field. */
const SEARCH_FROM = 8;

/**
 * A name in its own font, once it comes near the screen: a font loads when its name shows, so a
 * long list doesn't load every font at once.
 */
const FontName: FC<{ label: string; stack: string; font?: SystemFont }> = ({
  label,
  stack,
  font,
}) => {
  const ref = useRef<HTMLSpanElement>(null);
  const [seen, setSeen] = useState(() => typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    const node = ref.current;
    if (seen || !node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) setSeen(true);
      },
      { rootMargin: '25%' },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [seen]);
  useEffect(() => {
    if (seen && font) registerFaces(font);
  }, [seen, font]);
  return (
    <span ref={ref} className="truncate text-lg" style={seen ? { fontFamily: stack } : undefined}>
      {label}
    </span>
  );
};

const Choice: FC<{
  selected: boolean;
  onSelect: () => void;
  label: string;
  stack: string;
  font?: SystemFont;
  hint?: string;
}> = ({ selected, onSelect, label, stack, font, hint }) => (
  <button
    type="button"
    role="radio"
    aria-checked={selected}
    aria-label={label}
    onClick={onSelect}
    className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-2.5 text-left"
  >
    <span
      aria-hidden
      className={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${selected ? 'border-primary' : 'border-muted'}`}
    >
      {selected && <span className="bg-primary size-2.5 rounded-full" />}
    </span>
    <span className="flex min-w-0 flex-col">
      <FontName label={label} stack={stack} font={font} />
      {hint && <span className="text-muted text-xs">{hint}</span>}
    </span>
  </button>
);

/**
 * Picks a font: Raygun's own choices, then the device's installed families, each name shown in
 * its own font. A long list can be searched, and read again for fonts installed since.
 */
export const FontPickerSheet: FC<
  PickerProps & { anchor?: HTMLElement | null; onClose: () => void }
> = ({ title, own, value, fallback, onPick, anchor, onClose }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const fonts = useSystemFonts();
  const { query, setQuery, filtered } = useListSearch(fonts.data, (font) => [font.family]);
  const [refreshing, setRefreshing] = useState(false);
  const pick = (next: FontValue) => {
    onPick(next);
    onClose();
  };
  const refresh = () => {
    setRefreshing(true);
    void unwrapResult(commands.fontsList(true))
      .then((found) => queryClient.setQueryData(FONTS_KEY, found))
      .catch(() => undefined)
      .finally(() => setRefreshing(false));
  };
  const chosenFamily = value.kind === 'family' ? value.family : null;
  // A family picked on another device, or removed since: shown, so the choice makes sense.
  const missing =
    chosenFamily !== null &&
    fonts.data !== undefined &&
    !fonts.data.some((font) => font.family === chosenFamily);

  return (
    <Sheet title={title} anchor={anchor} onClose={onClose} popoverWidth={340}>
      {(fonts.data?.length ?? 0) > SEARCH_FROM && (
        <TextField
          type="search"
          value={query}
          onChange={setQuery}
          icon={Search}
          placeholder={t('settings.fonts.search')}
          aria-label={t('settings.fonts.search')}
          autoComplete="off"
          className={`${SHEET_FIELD} shrink-0`}
          inputClassName={SHEET_INPUT}
        />
      )}
      <div role="radiogroup" aria-label={title} className="-mx-2 flex flex-col">
        {own.map((option) => (
          <Choice
            key={option.id}
            selected={value.kind === 'own' && value.id === option.id}
            onSelect={() => pick({ kind: 'own', id: option.id })}
            label={option.label}
            stack={option.stack}
          />
        ))}
        <h3 className="text-muted px-3 pt-3 pb-1 text-xs font-semibold">
          {t('settings.fonts.onDevice')}
        </h3>
        {missing && (
          <Choice
            selected
            onSelect={onClose}
            label={chosenFamily}
            stack={fallback}
            hint={t('settings.fonts.missing', { name: chosenFamily })}
          />
        )}
        {fonts.isPending && (
          <p className="text-muted flex items-center gap-2 px-3 py-3 text-sm">
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
            {t('settings.fonts.loading')}
          </p>
        )}
        {fonts.isError && (
          <p className="text-muted px-3 py-3 text-sm">{t('settings.fonts.failed')}</p>
        )}
        {fonts.data?.length === 0 && (
          <p className="text-muted px-3 py-3 text-sm">{t('settings.fonts.none')}</p>
        )}
        {fonts.data && fonts.data.length > 0 && filtered.length === 0 && (
          <p className="text-muted px-3 py-3 text-sm">{t('settings.fonts.noMatch')}</p>
        )}
        {filtered.map((font) => (
          <Choice
            key={font.family}
            selected={chosenFamily === font.family}
            onSelect={() => pick({ kind: 'family', family: font.family })}
            label={font.family}
            stack={`${quoteFamily(font.family)}, ${fallback}`}
            font={font}
          />
        ))}
      </div>
      {fonts.data && (
        <button
          type="button"
          onClick={refresh}
          disabled={refreshing}
          className="text-primary hover:bg-background flex shrink-0 items-center gap-2 self-start rounded-full px-3 py-2 text-sm font-medium disabled:opacity-60"
        >
          <RefreshCw aria-hidden size={16} className={refreshing ? 'animate-spin' : ''} />
          {t('settings.fonts.refresh')}
        </button>
      )}
    </Sheet>
  );
};

/** A font as a settings row: the current one, in its font, and a sheet to pick another. */
export const PrefFont: FC<PickerProps> = (props) => {
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const open = anchor !== null;
  const { label, stack } = useFontLabel(props.own, props.value, props.fallback);
  return (
    <li data-pref={props.title}>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={(event) => setAnchor(open ? null : event.currentTarget)}
        className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-1 py-3 text-left"
      >
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{props.title}</span>
          <span className="text-foreground/80 truncate text-sm" style={{ fontFamily: stack }}>
            {label}
          </span>
        </span>
      </button>
      {open && <FontPickerSheet {...props} anchor={anchor} onClose={() => setAnchor(null)} />}
    </li>
  );
};
