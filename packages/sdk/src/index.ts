import './global';
import type { FetchInit, Provider } from './types';

export type * from './types';

/** Type-checks a provider against its kind's contract; returns it unchanged. */
export const defineProvider = <P extends Provider>(provider: P): P => provider;

/** Registers a provider with the app. Call it at the top level of the extension. */
export const registerProvider = (provider: Provider): void => {
  raygun.providers.register(provider);
};

/** Fetches and parses JSON; throws on a non-2xx status. */
export const fetchJson = async <T = unknown>(url: string, init?: FetchInit): Promise<T> => {
  const response = await raygun.http.fetch(url, init);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} from ${url}`);
  }
  return response.json<T>();
};
