import { commands } from '@raygun/bindings';
import type { ImportReader } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import type { TFunction } from 'i18next';

import { unwrapResult } from '../lib/commandResult';

export const READERS = ['import', 'readers'] as const;

/** The ways screenshots can be read here, best first. */
export const useImportReaders = () =>
  useQuery({ queryKey: READERS, queryFn: () => unwrapResult(commands.importReaders()) });

/** A reader's name as people know it. */
export const readerName = (reader: ImportReader, t: TFunction) =>
  reader.kind === 'ai'
    ? (reader.name ?? t('import.readers.ai'))
    : t(`import.readers.${reader.kind}`);
