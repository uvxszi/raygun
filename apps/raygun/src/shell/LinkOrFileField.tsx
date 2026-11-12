import { commands } from '@raygun/bindings';
import type { PickTarget } from '@raygun/bindings';
import { FileIcon, FolderIcon, FolderOpen, LoaderCircle } from 'lucide-react';
import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import type { ClipboardEvent, FC } from 'react';
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
 * The button beside a field that picks a file or a folder; for `both` a menu under it asks
 * which, closed by a press anywhere else. Why a pick failed goes to `onProblem` (`null` as one
 * starts), for under the field.
 */
const PickButton: FC<{
  pick: LinkOrFilePick;
  disabled?: boolean;
  onPicked: (path: string) => void;
  onProblem: (problem: string | null) => void;
}> = ({ pick, disabled, onPicked, onProblem }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [picking, setPicking] = useState(false);
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
    onProblem(null);
    setPicking(true);
    try {
      const picked = await unwrapResult(commands.filesPick(target));
      if (picked) onPicked(picked.path);
    } catch (failure) {
      onProblem(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setPicking(false);
    }
  };

  const label = t(
    pick === 'both' ? 'picker.choose' : pick === 'folder' ? 'picker.folder' : 'picker.file',
  );
  const item = 'hover:bg-foreground/10 flex items-center gap-3 px-4 py-2.5 text-left text-sm';

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        aria-label={label}
        title={label}
        aria-haspopup={pick === 'both' ? 'menu' : undefined}
        aria-expanded={pick === 'both' ? open : undefined}
        aria-controls={open ? menuId : undefined}
        disabled={disabled || picking}
        onClick={() => (pick === 'both' ? setOpen(!open) : void choose(pick))}
        className="bg-background text-muted hover:text-foreground grid size-11 place-items-center rounded-xl disabled:opacity-50"
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
  );
};

const PickProblem: FC<{ problem: string | null }> = ({ problem }) =>
  problem ? (
    <p role="alert" className="text-muted px-1 text-xs wrap-anywhere">
      {problem}
    </p>
  ) : null;

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
  const [problem, setProblem] = useState<string | null>(null);
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <TextField
          {...field}
          disabled={disabled}
          onChange={(value) => {
            setProblem(null);
            onChange(value);
          }}
          className={`flex-1 ${className}`}
        />
        <PickButton
          pick={pick}
          disabled={disabled}
          onProblem={setProblem}
          onPicked={(path) => {
            onChange(path);
            onPicked?.(path);
          }}
        />
      </div>
      <PickProblem problem={problem} />
    </div>
  );
};

type LinkOrFileLinesProps = {
  value: string;
  onChange: (value: string) => void;
  pick?: LinkOrFilePick;
  /** With what was picked; the caller decides what the field then holds. */
  onPicked: (path: string) => void;
  rows?: number;
  placeholder?: string;
  'aria-label': string;
  disabled?: boolean;
  autoFocus?: boolean;
};

/**
 * Links or paths one a line (a list typed or pasted), with the picker beside them. Each paste
 * ends its line (unless the text pasted already does), so the caret waits at the start of a new
 * one for the next.
 */
export const LinkOrFileLines: FC<LinkOrFileLinesProps> = ({
  value,
  onChange,
  pick = 'file',
  onPicked,
  rows = 6,
  disabled,
  ...field
}) => {
  const [problem, setProblem] = useState<string | null>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  // Where the caret goes once a paste is in the field.
  const caret = useRef<number | null>(null);

  useLayoutEffect(() => {
    const at = caret.current;
    if (at === null || !area.current) return;
    caret.current = null;
    area.current.setSelectionRange(at, at);
  }, [value]);

  const paste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = event.clipboardData.getData('text/plain') || event.clipboardData.getData('text');
    if (!pasted) return;
    const text = pasted.replace(/\r\n?/g, '\n');
    const inserted = text.endsWith('\n') ? text : `${text}\n`;
    event.preventDefault();
    const target = event.currentTarget;
    const start = target.selectionStart;
    const end = target.selectionEnd;
    caret.current = start + inserted.length;
    setProblem(null);
    onChange(target.value.slice(0, start) + inserted + target.value.slice(end));
  };

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-start gap-2">
        <textarea
          {...field}
          ref={area}
          rows={rows}
          value={value}
          disabled={disabled}
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          onChange={(event) => {
            setProblem(null);
            onChange(event.target.value);
          }}
          onPaste={paste}
          className="bg-background placeholder:text-muted min-w-0 flex-1 rounded-xl px-4 py-3 text-sm wrap-anywhere outline-none"
        />
        <PickButton pick={pick} disabled={disabled} onProblem={setProblem} onPicked={onPicked} />
      </div>
      <PickProblem problem={problem} />
    </div>
  );
};
