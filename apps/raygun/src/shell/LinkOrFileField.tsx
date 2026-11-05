import { commands } from '@raygun/bindings';
import type { PickTarget } from '@raygun/bindings';
import { FileIcon, FolderIcon, FolderOpen, LoaderCircle } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useBackLayer } from './backLayers';
import { TextField } from './TextField';
import type { TextFieldProps } from './TextField';

/** What a field's button picks: a file, a folder, or either (it then asks which). */
export type LinkOrFilePick = PickTarget | 'both';

type LinkOrFileFieldProps = Omit<TextFieldProps, 'trailing'> & {
  pick?: LinkOrFilePick;
  /** After a pick filled the field, with what it now holds. */
  onPicked?: (path: string) => void;
};

/**
 * A field that takes a link or a path, with a button beside it that picks a file or a folder
 * (M46.52), so a path never has to be typed: the system's dialog on desktop, the system picker
 * on Android (Rust copies the item into Raygun's data folder). The pick fills the field.
 */
export const LinkOrFileField: FC<LinkOrFileFieldProps> = ({
  pick = 'file',
  onPicked,
  onChange,
  className = '',
  disabled,
  ...field
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [picking, setPicking] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  useBackLayer(open, () => setOpen(false));

  useEffect(() => {
    if (!open) return;
    const close = (event: Event) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);

  const choose = async (target: PickTarget) => {
    setOpen(false);
    setProblem(null);
    setPicking(true);
    try {
      const picked = await unwrapResult(commands.filesPick(target));
      if (picked) {
        onChange(picked.path);
        onPicked?.(picked.path);
      }
    } catch (failure) {
      setProblem(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setPicking(false);
    }
  };

  const label = t(
    pick === 'both' ? 'picker.choose' : pick === 'folder' ? 'picker.folder' : 'picker.file',
  );
  const item = 'hover:bg-foreground/10 flex items-center gap-3 px-4 py-2.5 text-left text-sm';

  return (
    <div className="flex flex-col gap-1">
      <div ref={rootRef} className="relative flex items-center gap-2">
        <TextField
          {...field}
          disabled={disabled}
          onChange={(value) => {
            setProblem(null);
            onChange(value);
          }}
          className={`flex-1 ${className}`}
        />
        <button
          type="button"
          aria-label={label}
          title={label}
          aria-haspopup={pick === 'both' ? 'menu' : undefined}
          aria-expanded={pick === 'both' ? open : undefined}
          aria-controls={open ? menuId : undefined}
          disabled={disabled || picking}
          onClick={() => (pick === 'both' ? setOpen(!open) : void choose(pick))}
          className="bg-background text-muted hover:text-foreground grid size-11 shrink-0 place-items-center rounded-xl disabled:opacity-50"
        >
          {picking ? (
            <LoaderCircle aria-hidden size={18} className="animate-spin" />
          ) : (
            <FolderOpen aria-hidden size={18} />
          )}
        </button>
        {open && (
          <div
            id={menuId}
            role="menu"
            aria-label={label}
            className="bg-surface absolute top-full right-0 z-20 mt-1 flex min-w-48 flex-col overflow-hidden rounded-xl py-1 shadow-lg"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => void choose('file')}
              className={item}
            >
              <FileIcon aria-hidden size={16} className="text-muted" />
              {t('picker.file')}
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={() => void choose('folder')}
              className={item}
            >
              <FolderIcon aria-hidden size={16} className="text-muted" />
              {t('picker.folder')}
            </button>
          </div>
        )}
      </div>
      {problem && (
        <p role="alert" className="text-muted px-1 text-xs wrap-anywhere">
          {problem}
        </p>
      )}
    </div>
  );
};
