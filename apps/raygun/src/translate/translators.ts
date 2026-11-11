import type { AiProvider } from '@raygun/bindings';

import { useAiProviders } from '../ai/useAi';

/** The providers from Settings → AI that translate (a Whisper server only hears speech). */
export const useTranslatingProviders = (): AiProvider[] =>
  (useAiProviders().data ?? []).filter((provider) => provider.kind !== 'whisperServer');

/** The provider a translator names (`ai:<provider id>`), when it's one. */
export const providerOf = (translator: string, providers: readonly AiProvider[]) =>
  translator.startsWith('ai:')
    ? providers.find((provider) => provider.id === translator.slice('ai:'.length))
    : undefined;

/** Whether a translator is Google's: what it translates carries Google's attribution. */
export const isGoogle = (translator: string, providers: readonly AiProvider[]) =>
  providerOf(translator, providers)?.kind === 'google';
