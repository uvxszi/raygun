import { useCallback, useId, useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { cssColor, hexOf, parseHex, withAlpha } from './playerOptions';

type ColorOption = { value: number; label: string };

const Swatch: FC<{ color: number; className?: string }> = ({ color, className = 'size-5' }) => (
  <span
    aria-hidden
    // A checked backing shows how see-through a colour is; a ring shows black on dark.
    className={`ring-foreground/30 relative inline-block shrink-0 overflow-hidden rounded-full ring-1 ${className}`}
    style={{
      backgroundImage: 'conic-gradient(#bbb 25%, #fff 0 50%, #bbb 0 75%, #fff 0)',
      backgroundSize: '8px 8px',
    }}
  >
    <span className="absolute inset-0" style={{ backgroundColor: cssColor(color) }} />
  </span>
);

/**
 * A ref for an input whose native `change` (let go, picked) keeps its value, not each `input` on
 * the way. The inputs show only once Custom is picked, so the listener follows the element.
 */
const useCommit = (onCommit: (value: string) => void) => {
  const latest = useRef(onCommit);
  useLayoutEffect(() => {
    latest.current = onCommit;
  });
  return useCallback((element: HTMLInputElement | null) => {
    if (!element) return;
    const listener = () => latest.current(element.value);
    element.addEventListener('change', listener);
    return () => element.removeEventListener('change', listener);
  }, []);
};

/**
 * A colour preference (the subtitles' text, edge and background): the row shows the colour in
 * use as a swatch beside its name. Its sheet offers each colour with its swatch, and one of the
 * viewer's own: picked, or typed as #RRGGBB, with its opacity where it may be see-through.
 */
export const PrefColor: FC<{
  title: string;
  /** 0xAARRGGBB. */
  value: number;
  colors: ColorOption[];
  /** The colour may be see-through: an opacity slider. */
  opacity?: boolean;
  onChange: (value: number) => void;
  disabled?: boolean;
}> = ({ title, value, colors, opacity = false, onChange, disabled }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const open = anchor !== null;
  const close = () => setAnchor(null);
  const current = colors.find((color) => color.value === value);
  const [ownChosen, setOwnChosen] = useState(false);
  const own = ownChosen || !current;
  // What shows while a colour is picked or typed, before it's kept.
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const [hex, setHex] = useState<string | null>(null);
  const hexId = useId();
  // A colour of one's own starts opaque, or half see-through for a background that was none.
  const alpha = (shown >>> 24) & 0xff;
  const keep = (next: number) => {
    setDraft(null);
    if (next !== value) onChange(next >>> 0);
  };
  const picker = useCommit((text) => {
    const rgb = parseHex(text);
    if (rgb !== null) keep(withAlpha(rgb, opacity ? alpha || 0x80 : 0xff));
  });
  const slider = useCommit((text) =>
    keep(withAlpha(shown, Math.round((Number(text) / 100) * 255))),
  );
  const typed = () => {
    if (hex === null) return;
    const rgb = parseHex(hex);
    if (rgb !== null) keep(withAlpha(rgb, opacity ? alpha || 0x80 : 0xff));
    setHex(null);
  };

  return (
    <li data-pref={title}>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={disabled}
        onClick={(event) => setAnchor(open ? null : event.currentTarget)}
        className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-1 py-3 text-left disabled:opacity-60"
      >
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{title}</span>
          <span className="text-muted flex items-center gap-2 text-sm">
            <Swatch color={value} className="size-4" />
            <span className="text-foreground/80">
              {current?.label ??
                `${t('settings.player.customColor')} · ${hexOf(value).toUpperCase()}`}
            </span>
          </span>
        </span>
      </button>
      {open && (
        <Sheet title={title} anchor={anchor} onClose={close}>
          <div role="radiogroup" aria-label={title} className="-mx-2 flex flex-col">
            {[...colors, null].map((color) => {
              const selected = color ? !own && color.value === value : own;
              const label = color?.label ?? t('settings.player.colors.custom');
              return (
                <button
                  key={color?.value ?? 'custom'}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  onClick={() => {
                    if (color) {
                      setOwnChosen(false);
                      keep(color.value);
                      close();
                    } else {
                      setOwnChosen(true);
                    }
                  }}
                  className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left"
                >
                  <span
                    aria-hidden
                    className={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${selected ? 'border-primary' : 'border-muted'}`}
                  >
                    {selected && <span className="bg-primary size-2.5 rounded-full" />}
                  </span>
                  {color ? (
                    <Swatch color={color.value} />
                  ) : (
                    <span
                      aria-hidden
                      className="size-5 shrink-0 rounded-full"
                      style={{
                        background:
                          'conic-gradient(#f55, #fb4, #5d5, #4cd, #58f, #a6f, #f5a, #f55)',
                      }}
                    />
                  )}
                  <span>{label}</span>
                </button>
              );
            })}
          </div>
          {own && (
            <div className="flex flex-col gap-3 px-1 pt-2 pb-1">
              <div className="flex items-center gap-3">
                <Swatch color={shown} className="size-9" />
                <input
                  ref={picker}
                  type="color"
                  aria-label={t('settings.player.colorPicker')}
                  value={hexOf(shown)}
                  onChange={(event) => {
                    const rgb = parseHex(event.target.value);
                    if (rgb !== null) setDraft(withAlpha(rgb, opacity ? alpha || 0x80 : 0xff));
                  }}
                  className="h-9 w-14 cursor-pointer rounded-lg bg-transparent"
                />
                <label htmlFor={hexId} className="sr-only">
                  {t('settings.player.colorCode')}
                </label>
                <input
                  id={hexId}
                  type="text"
                  spellCheck={false}
                  value={hex ?? hexOf(shown)}
                  onChange={(event) => setHex(event.target.value)}
                  onBlur={typed}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') (event.target as HTMLInputElement).blur();
                  }}
                  className="bg-surface w-28 rounded-lg px-2 py-1.5 font-mono text-sm uppercase outline-none focus:ring-1"
                />
              </div>
              {opacity && (
                <label className="flex items-center gap-3 text-sm">
                  <span className="w-20 shrink-0">{t('settings.player.opacity')}</span>
                  <input
                    ref={slider}
                    type="range"
                    min={0}
                    max={100}
                    value={Math.round(((shown >>> 24) & 0xff) / 2.55)}
                    onChange={(event) =>
                      setDraft(
                        withAlpha(shown, Math.round((Number(event.target.value) / 100) * 255)),
                      )
                    }
                    className="accent-primary w-full"
                  />
                </label>
              )}
            </div>
          )}
        </Sheet>
      )}
    </li>
  );
};
