/** Raygun's version, from the app's package (vite.pages.config.ts). */
declare const __RAYGUN_VERSION__: string;

declare module 'virtual:raygun-web-answers' {
  import type { CommandAnswers } from './core/fallback';

  const answers: CommandAnswers;
  export default answers;
}
