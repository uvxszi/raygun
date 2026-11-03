import { ArrowDown, ArrowUp, ImagePlus, X } from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { sortByTaken } from './screenshotOrder';

/**
 * The screenshots to read, in order (M46.40): numbered thumbnails sorted by when each was taken,
 * moved with up and down, removed, or added to. Reading goes in this order, so a long list
 * screenshotted while scrolling comes out as it was.
 */
export const ScreenshotQueue: FC<{
  files: File[];
  onChange: (files: File[]) => void;
  disabled?: boolean;
}> = ({ files, onChange, disabled = false }) => {
  const { t } = useTranslation();
  const more = useRef<HTMLInputElement>(null);
  const previews = useMemo(() => files.map((file) => URL.createObjectURL(file)), [files]);
  useEffect(() => () => previews.forEach((url) => URL.revokeObjectURL(url)), [previews]);
  const move = (index: number, by: number) => {
    const next = [...files];
    const [file] = next.splice(index, 1);
    if (file) next.splice(index + by, 0, file);
    onChange(next);
  };
  const button =
    'text-muted hover:text-foreground grid size-8 place-items-center rounded-full disabled:opacity-30';
  return (
    <div className="flex flex-col gap-2">
      <ol data-testid="screenshot-queue" className="flex max-h-72 flex-col gap-1 overflow-y-auto">
        {files.map((file, index) => (
          <li
            key={`${file.name}-${file.lastModified}-${index}`}
            className="bg-background flex items-center gap-3 rounded-lg p-1.5"
          >
            <span className="text-muted w-5 text-right text-sm tabular-nums">{index + 1}</span>
            <img src={previews[index]} alt="" className="h-12 w-8 shrink-0 rounded object-cover" />
            <span className="min-w-0 flex-1 truncate text-sm">{file.name}</span>
            <button
              type="button"
              aria-label={t('import.moveUp', { title: file.name })}
              disabled={disabled || index === 0}
              onClick={() => move(index, -1)}
              className={button}
            >
              <ArrowUp aria-hidden size={16} />
            </button>
            <button
              type="button"
              aria-label={t('import.moveDown', { title: file.name })}
              disabled={disabled || index === files.length - 1}
              onClick={() => move(index, 1)}
              className={button}
            >
              <ArrowDown aria-hidden size={16} />
            </button>
            <button
              type="button"
              aria-label={t('import.removeShot', { name: file.name })}
              disabled={disabled}
              onClick={() => onChange(files.filter((_, at) => at !== index))}
              className={button}
            >
              <X aria-hidden size={16} />
            </button>
          </li>
        ))}
      </ol>
      <button
        type="button"
        disabled={disabled}
        onClick={() => more.current?.click()}
        className="text-primary flex items-center gap-1.5 self-start text-sm font-medium disabled:opacity-50"
      >
        <ImagePlus aria-hidden size={16} />
        {t('import.addScreenshots')}
      </button>
      <input
        ref={more}
        type="file"
        accept="image/*"
        multiple
        aria-label={t('import.addScreenshots')}
        className="hidden"
        onChange={(event) => {
          const added = sortByTaken([...(event.target.files ?? [])]);
          if (added.length > 0) onChange([...files, ...added]);
          event.target.value = '';
        }}
      />
    </div>
  );
};
