import type { AiModel } from '@raygun/bindings';

/** What an on-device model is for: the part of Settings → AI it serves. */
export type ModelUse = 'translation' | 'transcription' | 'textReading' | 'comicReader' | 'unused';

/** Models list by their use, in the screen's order. */
const ORDER: ModelUse[] = ['translation', 'transcription', 'textReading', 'comicReader', 'unused'];

export const modelUse = (model: AiModel): ModelUse => {
  switch (model.kind) {
    case 'translate':
      return 'translation';
    case 'speech':
      return 'transcription';
    case 'ocr':
      return 'textReading';
    case 'upscale':
    case 'restore':
      // The comic reader runs Anime4K's on the graphics card; ncnn's have no runner yet.
      return model.engine === 'anime4k' ? 'comicReader' : 'unused';
    default:
      return 'unused';
  }
};

/**
 * The models to list, by use (the catalogue's order within one): every one Raygun runs, and the
 * others only while downloaded, to remove them.
 */
export const listedModels = (models: AiModel[]): AiModel[] =>
  models
    .filter((model) => model.installed || modelUse(model) !== 'unused')
    .sort((first, second) => ORDER.indexOf(modelUse(first)) - ORDER.indexOf(modelUse(second)));

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
