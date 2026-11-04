import { X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useCallback, useRef } from 'react';
import type { FC, InputHTMLAttributes, KeyboardEvent, ReactNode, Ref } from 'react';
import { useTranslation } from 'react-i18next';

export type TextFieldProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'value' | 'onChange' | 'type' | 'className' | 'children'
> & {
  value: string;
  onChange: (value: string) => void;
  type?: 'text' | 'search' | 'url' | 'email' | 'password' | 'number';
  /** An icon before the text, such as a magnifier. */
  icon?: LucideIcon;
  iconSize?: number;
  /** The box: background, corners, padding and width. */
  className?: string;
  /** The text itself: size, vertical padding. */
  inputClassName?: string;
  /** Escape empties the field before doing anything else; on by default for search fields. */
  clearOnEscape?: boolean;
  /** After the clear button or Escape emptied it. */
  onClear?: () => void;
  /** Shown between the text and the clear button: a spinner, a unit. */
  trailing?: ReactNode;
  ref?: Ref<HTMLInputElement>;
};

/** Sets every ref the field is known by. */
const setRefs = (node: HTMLInputElement | null, refs: (Ref<HTMLInputElement> | undefined)[]) => {
  for (const ref of refs) {
    if (typeof ref === 'function') ref(node);
    else if (ref) ref.current = node;
  }
};

/** The style of a field in a sheet or a form: a filled box on the sheet's surface. */
export const SHEET_FIELD = 'bg-background rounded-xl pl-4 pr-1.5';
export const SHEET_INPUT = 'py-3';

/**
 * A one-line text field with a clear button while it has text (deleting a long address letter by
 * letter on a phone is no fun). Clearing keeps the keyboard up; Escape clears search fields, and
 * with nothing left to clear it does whatever Escape does around the field (close a sheet, leave
 * search).
 */
export const TextField: FC<TextFieldProps> = ({
  value,
  onChange,
  type = 'text',
  icon: Icon,
  iconSize = 16,
  className = '',
  inputClassName = '',
  clearOnEscape,
  onClear,
  trailing,
  onKeyDown,
  ref,
  disabled,
  readOnly,
  ...input
}) => {
  const { t } = useTranslation();
  const own = useRef<HTMLInputElement | null>(null);
  const setRef = useCallback((node: HTMLInputElement | null) => setRefs(node, [own, ref]), [ref]);
  const label = input['aria-label'];
  const clearable = value !== '' && !disabled && !readOnly;
  const escapes = clearOnEscape ?? type === 'search';
  const clear = () => {
    onChange('');
    onClear?.();
    own.current?.focus();
  };
  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape' && escapes && clearable) {
      // This Escape is the field's: nothing around it closes.
      event.preventDefault();
      event.stopPropagation();
      clear();
      return;
    }
    onKeyDown?.(event);
  };
  return (
    <span className={`flex min-w-0 items-center gap-2 ${className}`}>
      {Icon && <Icon aria-hidden size={iconSize} className="text-muted shrink-0" />}
      <input
        {...input}
        ref={setRef}
        type={type}
        value={value}
        disabled={disabled}
        readOnly={readOnly}
        enterKeyHint={input.enterKeyHint ?? (type === 'search' ? 'search' : undefined)}
        // Popovers let the field have this Escape while it has text.
        data-clears-on-escape={escapes ? 'true' : undefined}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={keyDown}
        className={`placeholder:text-muted min-w-0 flex-1 bg-transparent outline-none [&::-webkit-search-cancel-button]:hidden ${inputClassName}`}
      />
      {trailing}
      {clearable && (
        <button
          type="button"
          // Tab goes on to the next field; keys clear with Escape or a select-all.
          tabIndex={-1}
          aria-label={label ? t('common.clearField', { field: label }) : t('common.clear')}
          // The field keeps focus (and a phone its keyboard) while the button is pressed.
          onPointerDown={(event) => event.preventDefault()}
          onMouseDown={(event) => event.preventDefault()}
          onClick={clear}
          className="text-muted hover:text-foreground hover:bg-foreground/10 -my-1 grid size-8 shrink-0 place-items-center rounded-full"
        >
          <X aria-hidden size={16} />
        </button>
      )}
    </span>
  );
};
