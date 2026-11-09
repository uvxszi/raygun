import { commands } from '@raygun/bindings';
import type {
  AiModel,
  PageTextBlock,
  ReaderAiKind,
  ReaderAiLayer,
  ReaderAiPrefs,
  TextBlock,
} from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useRef, useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { MAX_PIXELS, runShader } from './anime4k';

/** The readers with AI layers: comics, and novels (whose layers translate). */
export type ReaderMedium = 'comic' | 'novel';

const NO_LAYERS: ReaderAiPrefs = { comic: [], novel: [] };

/** What each reader's layers can be. */
export const KINDS_OF: Record<ReaderMedium, ReaderAiKind[]> = {
  comic: ['upscale', 'restore', 'translate'],
  novel: ['translate'],
};

/** A layer with every field given (Rust leaves empty ones out). */
export type Layer = Required<ReaderAiLayer>;

/** A reader's layers, bottom first, and saving them. */
export const useReaderAiLayers = (medium: ReaderMedium) => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const prefs = settings.data?.readerAi ?? NO_LAYERS;
  const layers: Layer[] = (prefs[medium] ?? []).map((layer) => ({
    ...layer,
    model: layer.model ?? '',
    from: layer.from ?? '',
    to: layer.to ?? '',
  }));
  const save = (next: Layer[]) =>
    update.mutate({ key: 'readerAi', value: { ...prefs, [medium]: next } });
  return { layers, save };
};

/** The catalogue's models a layer of `kind` can run: Anime4K's for pictures, translators. */
export const modelsFor = (models: AiModel[], kind: ReaderAiKind) =>
  models.filter((model) =>
    kind === 'translate'
      ? model.kind === 'translate'
      : model.kind === kind && model.engine === 'anime4k',
  );

/** The model a picture layer runs: its own if installed, else the first installed of its kind. */
export const pictureModel = (layer: Layer, models: AiModel[]): AiModel | null => {
  const fitting = modelsFor(models, layer.kind).filter((model) => model.installed);
  return fitting.find((model) => model.id === layer.model) ?? fitting[0] ?? null;
};

/**
 * The models this device has, read while any of a reader's layers is on, and their ids as one
 * key: work that failed for want of a model runs again once one is downloaded.
 */
const useInstalledModels = (on: boolean) => {
  const query = useQuery({
    queryKey: ['ai', 'models'],
    queryFn: () => commands.aiModels(),
    enabled: on,
  });
  const models = query.data ?? [];
  const installed = models
    .filter((model) => model.installed)
    .map((model) => model.id)
    .join(',');
  return { models, installed, ready: query.isFetched };
};

/** A new layer of `kind`, with a fresh id. */
export const newLayer = (kind: ReaderAiKind, language: string): Layer => ({
  id: `${kind}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
  kind,
  enabled: true,
  model: '',
  from: '',
  to: kind === 'translate' ? language : '',
});

// ---- Pictures ----

/** Pictures drawn by layers, by page and layers, as object URLs; the oldest are let go. */
const drawn = new Map<string, string>();
const DRAWN_KEPT = 40;

const keep = (key: string, url: string) => {
  drawn.set(key, url);
  while (drawn.size > DRAWN_KEPT) {
    const [oldest, old] = drawn.entries().next().value as [string, string];
    drawn.delete(oldest);
    URL.revokeObjectURL(old);
  }
};

/** One picture on the graphics card at a time. */
let queue: Promise<unknown> = Promise.resolve();
const inTurn = <T>(work: () => Promise<T>): Promise<T> => {
  const next = queue.then(work, work);
  queue = next.catch(() => undefined);
  return next;
};

const shaderText = (client: QueryClient, id: string) =>
  client.fetchQuery({
    queryKey: ['readerAi', 'shader', id],
    queryFn: () => unwrapResult(commands.aiModelText(id)),
    staleTime: Infinity,
  });

const toBlob = (canvas: HTMLCanvasElement) =>
  new Promise<Blob>((resolve, reject) =>
    canvas.toBlob(
      (blob) => (blob ? resolve(blob) : reject(new Error("The picture couldn't be saved"))),
      'image/jpeg',
      0.93,
    ),
  );

/** Runs the picture layers (model ids, bottom first) on a page; the drawn page's URL. */
const drawPage = async (client: QueryClient, url: string, models: string[]) => {
  const key = `${url}\u001f${models.join(',')}`;
  const known = drawn.get(key);
  if (known) return known;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`The page couldn't be loaded (HTTP ${response.status})`);
  let picture = await createImageBitmap(await response.blob());
  const result = await inTurn(async () => {
    let canvas: HTMLCanvasElement | null = null;
    for (const id of models) {
      // Pages already this large are left as they are.
      if (picture.width * picture.height > MAX_PIXELS) break;
      const shader = await shaderText(client, id);
      canvas = runShader(id, shader, picture);
      picture.close();
      picture = await createImageBitmap(canvas);
    }
    picture.close();
    return canvas;
  });
  if (!result) return null;
  const object = URL.createObjectURL(await toBlob(result));
  keep(key, object);
  return object;
};

/**
 * A comic page with the reader's layers: the picture the picture layers drew (null until it's
 * drawn, or when none is on), and the words the translation layers made, where they are.
 */
export const usePageAi = (url: string) => {
  const client = useQueryClient();
  const { layers } = useReaderAiLayers('comic');
  const on = layers.filter((layer) => layer.enabled);
  // The same list Settings → AI shows, read only while a layer is on.
  const { models, installed, ready } = useInstalledModels(on.length > 0);
  const pictureIds = on
    .filter((layer) => layer.kind !== 'translate')
    .map((layer) => pictureModel(layer, models)?.id)
    .filter((id): id is string => Boolean(id));
  const translations = on.filter((layer) => layer.kind === 'translate');
  const page = url.startsWith('http://127.0.0.1') || url.startsWith('http://localhost');

  const picture = useQuery({
    queryKey: ['readerAi', 'picture', url, pictureIds],
    queryFn: () => drawPage(client, url, pictureIds),
    enabled: page && pictureIds.length > 0,
    staleTime: Infinity,
    gcTime: 10 * 60_000,
    retry: false,
  });
  // With the installed models in the keys, a page that failed for want of the text reader or a
  // translator is read again once it's downloaded (Rust keeps what it read and translated).
  const text = useQuery({
    queryKey: ['readerAi', 'pageText', url, installed],
    queryFn: () => unwrapResult(commands.readerAiPageText(url)),
    enabled: page && translations.length > 0 && ready,
    staleTime: Infinity,
    gcTime: 10 * 60_000,
    retry: false,
  });
  const steps = translations.map(({ model, from, to }) => ({ model, from, to }));
  const translated = useQuery({
    queryKey: ['readerAi', 'pageTranslation', url, steps, installed],
    queryFn: async (): Promise<PageTextBlock[]> => {
      const blocks = text.data ?? [];
      let texts = blocks.map((block) => block.text);
      for (const step of steps) {
        texts = await unwrapResult(commands.readerAiTranslate({ texts, ...step }));
      }
      return blocks.map((block, index) => ({ ...block, text: texts[index] ?? block.text }));
    },
    enabled: text.isSuccess && (text.data?.length ?? 0) > 0,
    staleTime: Infinity,
    gcTime: 10 * 60_000,
    retry: false,
  });
  return {
    src: pictureIds.length > 0 ? (picture.data ?? null) : null,
    blocks: translations.length > 0 ? (translated.data ?? null) : null,
    busy: picture.isFetching || text.isFetching || translated.isFetching,
    error: (picture.error ?? text.error ?? translated.error)?.message ?? null,
  };
};

// ---- Novels ----

/** Paragraphs translated at a time. */
const BATCH = 6;

const translatable = (block: TextBlock): block is TextBlock & { text: string } =>
  (block.kind === 'paragraph' || block.kind === 'heading') && block.text.trim() !== '';

/**
 * A novel chapter's text through its translation layers, a few paragraphs at a time from where
 * the reader is (`from`), then the rest: each block's text once translated.
 */
export const useNovelAi = (chapterKey: string, blocks: TextBlock[], from: number) => {
  const { layers } = useReaderAiLayers('novel');
  const steps = useMemo(
    () =>
      layers
        .filter((layer) => layer.enabled && layer.kind === 'translate')
        .map(({ model, from: source, to }) => ({ model, from: source, to })),
    [layers],
  );
  const stepsKey = JSON.stringify(steps);
  // A translation that failed for want of a model runs again once one is downloaded.
  const { installed, ready } = useInstalledModels(steps.length > 0);
  const [state, setState] = useState<{
    key: string;
    texts: Map<number, string>;
    busy: boolean;
    error: string | null;
  }>({ key: '', texts: new Map(), busy: false, error: null });
  const textKey = `${chapterKey}\u001f${stepsKey}`;
  const runKey = `${textKey}\u001f${installed}`;
  // Where the reader is when a translation starts.
  const start = useRef(from);
  useEffect(() => {
    start.current = from;
  }, [from]);

  useEffect(() => {
    if (steps.length === 0 || !chapterKey || !ready) return;
    let stopped = false;
    const indexes = blocks.flatMap((block, index) => (translatable(block) ? [index] : []));
    // From where the reader is, then what's before it.
    const at = start.current;
    const order = [
      ...indexes.filter((index) => index >= at),
      ...indexes.filter((index) => index < at),
    ];
    const run = async () => {
      // Paragraphs already translated stay while a new model has the rest done again.
      setState((current) => ({
        key: runKey,
        texts: current.key.startsWith(`${textKey}\u001f`) ? current.texts : new Map(),
        busy: true,
        error: null,
      }));
      for (let at = 0; at < order.length && !stopped; at += BATCH) {
        const batch = order.slice(at, at + BATCH);
        let texts = batch.map((index) => (blocks[index] as { text: string }).text);
        try {
          for (const step of steps) {
            texts = await unwrapResult(commands.readerAiTranslate({ texts, ...step }));
          }
        } catch (error) {
          if (!stopped) {
            setState((current) => ({
              ...current,
              busy: false,
              error: error instanceof Error ? error.message : String(error),
            }));
          }
          return;
        }
        if (stopped) return;
        setState((current) => {
          const next = new Map(current.texts);
          batch.forEach((index, place) => next.set(index, texts[place] ?? ''));
          return { ...current, texts: next };
        });
      }
      if (!stopped) setState((current) => ({ ...current, busy: false }));
    };
    void run();
    return () => {
      stopped = true;
    };
    // `steps` follows `stepsKey`; blocks follow the chapter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runKey, ready]);

  const current = state.key === runKey && steps.length > 0;
  return {
    on: steps.length > 0,
    texts: current ? state.texts : new Map<number, string>(),
    busy: current && state.busy,
    error: current ? state.error : null,
  };
};
