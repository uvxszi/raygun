import { ArrowDown, ArrowUp, ImagePlus, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { PictureInspector } from './PictureInspector';
import { cropPicture, rotatePicture } from './pictureCrop';
import type { CropBox } from './pictureCrop';
import { sortByTaken } from './screenshotOrder';

/**
 * The screenshots to read, in order (M46.40): numbered thumbnails sorted by when each was taken,
 * moved with up and down, removed, or added to. Reading goes in this order, so a long list
 * screenshotted while scrolling comes out as it was. A thumbnail opens the screenshot to look at
 * closely, crop to the part with the songs or turn upright before anything is read (v2.0.1).
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
  // The screenshot open in the inspector, by its place in the queue.
  const [open, setOpen] = useState<number | null>(null);
  const [working, setWorking] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const replace = async (index: number, change: (file: File) => Promise<File>) => {
    const file = files[index];
    if (!file || working) return;
    setWorking(true);
    setProblem(null);
    try {
      const changed = await change(file);
      // Keeps the screenshot's time, so it stays in its place if the list is sorted again.
      const kept = new File([changed], changed.name, {
        type: changed.type,
        lastModified: file.lastModified,
      });
      onChange(files.map((other, at) => (at === index ? kept : other)));
    } catch (error) {
      setProblem(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(false);
    }
  };
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
            <button
              type="button"
              aria-label={t('import.openShot', { name: file.name })}
              disabled={disabled}
              onClick={() => setOpen(index)}
              className="shrink-0 rounded"
            >
              <img src={previews[index]} alt="" className="h-14 w-10 rounded object-cover" />
            </button>
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
      {problem && <SourceIssue>{problem}</SourceIssue>}
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
      {open !== null && files[open] && previews[open] && (
        <PictureInspector
          // A cropped or turned screenshot opens anew: unzoomed, not cropping.
          key={previews[open]}
          src={previews[open] ?? ''}
          title={t('import.shotTitle', { number: open + 1 })}
          onClose={() => setOpen(null)}
          reading={working}
          cropLabel={t('import.keepCrop')}
          onCrop={(box: CropBox) => void replace(open, (file) => cropPicture(file, box, file.name))}
          onRotate={() => void replace(open, rotatePicture)}
        />
      )}
    </div>
  );
};
