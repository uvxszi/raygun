import type { FC } from 'react';

import { ModelsSection } from '../ai/ModelsSection';
import { ProvidersSection } from '../ai/ProvidersSection';
import { TextReadingSection } from '../ai/TextReadingSection';
import { TranscriptionSection } from '../ai/TranscriptionSection';
import { TranslationSection } from '../ai/TranslationSection';
import { McpCard } from '../McpCard';

/**
 * Settings → AI, by what it's for: translation, transcription and text reading (each with where
 * it's used), then the online providers and the on-device models they run on, and AI assistants.
 * Each group has an anchor (`/settings/ai#transcription`, `#models`…).
 */
export const AiSettings: FC = () => (
  <>
    <TranslationSection />
    <TranscriptionSection />
    <TextReadingSection />
    <ProvidersSection />
    <ModelsSection />
    <McpCard />
  </>
);
