import { Check, X } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

type Props = { title: string; onClose: () => void; children: ReactNode };

/** A side panel over the picture: episodes, sources, speed, tracks, quality, aspect. */
export const PlayerPanel: FC<Props> = ({ title, onClose, children }) => {
  const { t } = useTranslation();
  return (
    <section
      aria-label={title}
      onClick={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
      className="absolute inset-y-3 right-3 z-20 flex w-80 max-w-[calc(100%-1.5rem)] flex-col overflow-hidden rounded-2xl bg-neutral-900/95 shadow-2xl ring-1 ring-white/10 backdrop-blur"
    >
      <header className="flex items-center justify-between gap-2 px-4 pt-3 pb-2">
        <h2 className="truncate text-base font-semibold">{title}</h2>
        <button
          type="button"
          aria-label={t('desktopPlayer.closePanel')}
          onClick={onClose}
          className="grid size-9 place-items-center rounded-full hover:bg-white/10"
        >
          <X aria-hidden size={18} />
        </button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-3">{children}</div>
    </section>
  );
};

type OptionProps = {
  label: string;
  hint?: string | null;
  selected?: boolean;
  disabled?: boolean;
  onSelect: () => void;
};

/** One choice in a panel; the chosen one has a tick. */
export const PanelOption: FC<OptionProps> = ({
  label,
  hint,
  selected = false,
  disabled,
  onSelect,
}) => (
  <button
    type="button"
    role="menuitemradio"
    aria-checked={selected}
    disabled={disabled}
    onClick={onSelect}
    className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm hover:bg-white/10 disabled:opacity-40 ${selected ? 'text-primary' : ''}`}
  >
    <span className="flex min-w-0 flex-1 flex-col">
      <span className="truncate">{label}</span>
      {hint && <span className="truncate text-xs text-white/55">{hint}</span>}
    </span>
    {selected && <Check aria-hidden size={16} className="shrink-0" />}
  </button>
);

/** A small heading inside a panel. */
export const PanelHeading: FC<{ children: ReactNode }> = ({ children }) => (
  <h3 className="px-3 pt-3 pb-1 text-xs font-semibold tracking-wide text-white/55 uppercase">
    {children}
  </h3>
);
