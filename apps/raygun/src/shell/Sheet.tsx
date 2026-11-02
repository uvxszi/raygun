import { X } from 'lucide-react';
import { useEffect, useId, useRef } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Popover } from './Popover';
import { useWideScreen } from './useWideScreen';

type SheetProps = {
  title: string;
  onClose: () => void;
  children: ReactNode;
  /**
   * The button that opened it: on wide windows the sheet is then a popover beside that button
   * (M46.7) instead of a dialog over the whole window.
   */
  anchor?: HTMLElement | null;
  /** The popover's width in px. */
  popoverWidth?: number;
};

/**
 * A bottom sheet (a centred dialog on wide screens, or a popover beside the button that opened
 * it) for short tasks: typing an address, naming a playlist, picking a folder. Inputs live here or
 * in the top bar, never at the end of a list.
 */
export const Sheet: FC<SheetProps> = ({ anchor, popoverWidth, ...props }) => {
  const wide = useWideScreen();
  if (anchor && wide) {
    return (
      <Popover anchor={anchor} label={props.title} onClose={props.onClose} width={popoverWidth}>
        <h2 className="text-muted px-3 pt-1 pb-2 text-xs font-semibold tracking-wide uppercase">
          {props.title}
        </h2>
        {props.children}
      </Popover>
    );
  }
  return <DialogSheet {...props} />;
};

const DialogSheet: FC<Omit<SheetProps, 'anchor' | 'popoverWidth'>> = ({
  title,
  onClose,
  children,
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  // Closing on pointer down would let the tap's click land on whatever is under the sheet.
  const pressedBackdrop = useRef(false);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', escape);
    return () => document.removeEventListener('keydown', escape);
  }, [onClose]);

  return (
    <div
      className="motion-fade fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center"
      onPointerDown={(event) => {
        pressedBackdrop.current = event.target === event.currentTarget;
      }}
      onClick={(event) => {
        if (pressedBackdrop.current && event.target === event.currentTarget) onClose();
        pressedBackdrop.current = false;
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="bg-surface motion-rise flex max-h-[85vh] w-full max-w-md flex-col gap-4 overflow-y-auto rounded-t-3xl px-5 pt-5 pb-[calc(1.25rem+env(safe-area-inset-bottom))] sm:rounded-3xl"
      >
        <div className="flex items-center justify-between gap-3">
          <h2 id={titleId} className="text-lg font-semibold">
            {title}
          </h2>
          <button
            type="button"
            aria-label={t('watch.close')}
            onClick={onClose}
            className="hover:bg-background -mr-2 grid size-10 place-items-center rounded-full"
          >
            <X aria-hidden size={20} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
};
