import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useRef, useState } from 'react';
import type { ChangeEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';

/** Slices sent at a time: big enough to be quick, small enough for one IPC message. */
const CHUNK_BYTES = 2 * 1024 * 1024;

const toBase64 = (bytes: Uint8Array) => {
  let binary = '';
  const step = 0x8000;
  for (let start = 0; start < bytes.length; start += step) {
    binary += String.fromCharCode(...bytes.subarray(start, start + step));
  }
  return btoa(binary);
};

/** Sends a file to Rust in slices; returns the upload's id. */
export const uploadFile = async (file: File, onProgress?: (sent: number) => void) => {
  const id = await unwrapResult(commands.uploadBegin());
  try {
    for (let start = 0; start < file.size; start += CHUNK_BYTES) {
      const slice = new Uint8Array(await file.slice(start, start + CHUNK_BYTES).arrayBuffer());
      await unwrapResult(commands.uploadAppend(id, toBase64(slice)));
      onProgress?.(Math.min(file.size, start + CHUNK_BYTES));
    }
  } catch (error) {
    void commands.uploadCancel(id);
    throw error;
  }
  return id;
};

/**
 * Library → Manga → Import comics (M46): picks CBZ or ZIP files, sends each in slices and imports
 * it as a chapter of a local series, then opens the series.
 */
export const useImportComics = () => {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const importFiles = useMutation({
    mutationFn: async (files: File[]) => {
      let last = null;
      for (const [index, file] of files.entries()) {
        setProgress({ done: index, total: files.length });
        const id = await uploadFile(file);
        last = await unwrapResult(commands.mangaImportComic(id, file.name));
      }
      return last;
    },
    onSettled: () => {
      setProgress(null);
      void queryClient.invalidateQueries({ queryKey: ['manga'] });
    },
    onSuccess: (title) => {
      if (title) {
        void navigate({
          to: '/manga/title',
          search: { source: title.sourceId, id: title.remoteId },
        });
      }
    },
  });

  const onPick = (event: ChangeEvent<HTMLInputElement>) => {
    const files = [...(event.target.files ?? [])];
    event.target.value = '';
    if (files.length > 0) importFiles.mutate(files);
  };

  return {
    pick: () => inputRef.current?.click(),
    importFiles,
    progress,
    input: (
      <input
        ref={inputRef}
        type="file"
        multiple
        accept=".cbz,.zip,application/zip,application/vnd.comicbook+zip,application/x-cbz"
        aria-label={t('manga.comicFiles')}
        onChange={onPick}
        className="hidden"
      />
    ),
  };
};
