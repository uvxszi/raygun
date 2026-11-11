import type { AiModel } from '@raygun/bindings';

/** What an on-device model is for: the part of Settings → AI it serves. */
export type ModelUse =
  'translation' | 'transcription' | 'textReading' | 'readAloud' | 'comicReader' | 'unused';

export const modelUse = (model: AiModel): ModelUse => {
  switch (model.kind) {
    case 'translate':
      return 'translation';
    case 'speech':
      return 'transcription';
    case 'ocr':
      return 'textReading';
    // Voices, and the engine that runs them (M46.x: reading aloud).
    case 'tts':
    case 'runtime':
      return 'readAloud';
    case 'upscale':
    case 'restore':
      // The comic reader runs Anime4K's on the graphics card; ncnn's have no runner yet.
      return model.engine === 'anime4k' ? 'comicReader' : 'unused';
    default:
      return 'unused';
  }
};

/** On-device models' pills: every model, those for one use, or the downloaded ones. */
export type ModelFilter =
  | 'all'
  | 'translation'
  | 'transcription'
  | 'textReading'
  | 'readAloud'
  | 'upscaling'
  | 'downloaded';

export const MODEL_FILTERS: ModelFilter[] = [
  'all',
  'translation',
  'transcription',
  'textReading',
  'readAloud',
  'upscaling',
  'downloaded',
];

/** How On-device models are ordered: by name, the largest first, or the downloaded ones first. */
export type ModelSort = 'name' | 'size' | 'downloaded';

export const MODEL_SORTS: ModelSort[] = ['name', 'size', 'downloaded'];

const matches = (model: AiModel, filter: ModelFilter): boolean => {
  switch (filter) {
    case 'all':
      return true;
    case 'downloaded':
      return model.installed;
    // The readers' upscale and restore layers.
    case 'upscaling':
      return model.kind === 'upscale' || model.kind === 'restore';
    default:
      return modelUse(model) === filter;
  }
};

const byName = (first: AiModel, second: AiModel) => first.name.localeCompare(second.name);

const ORDERS: Record<ModelSort, (first: AiModel, second: AiModel) => number> = {
  name: byName,
  size: (first, second) =>
    (second.sizeBytes ?? 0) - (first.sizeBytes ?? 0) || byName(first, second),
  downloaded: (first, second) =>
    Number(second.installed) - Number(first.installed) || byName(first, second),
};

/**
 * The models On-device models lists under `filter`, in `sort`'s order: every one Raygun runs, and
 * the others only while downloaded, to remove them.
 */
export const listedModels = (
  models: AiModel[],
  filter: ModelFilter = 'all',
  sort: ModelSort = 'name',
): AiModel[] =>
  models
    .filter((model) => model.installed || modelUse(model) !== 'unused')
    .filter((model) => matches(model, filter))
    .sort(ORDERS[sort]);

/** The downloaded models, and the bytes they take. */
export const modelStorage = (models: AiModel[]) => {
  const installed = models.filter((model) => model.installed);
  return {
    count: installed.length,
    bytes: installed.reduce((sum, model) => sum + (model.sizeBytes ?? 0), 0),
  };
};

/** How far a model's download is, in whole percent (99 until it's checked and in place). */
export const downloadPercent = (model: AiModel): number | null => {
  const received = model.downloading ?? null;
  if (received === null) return null;
  const size = model.sizeBytes ?? 0;
  return size > 0 ? Math.min(99, Math.floor((received / size) * 100)) : 0;
};
