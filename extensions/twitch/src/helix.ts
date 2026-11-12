// Twitch's official API (Helix): an app access token from the person's own registered
// application (the client credentials flow, https://dev.twitch.tv/docs/authentication/), then
// requests with its Client ID. A key Twitch refuses says so with `needsSetup`.
import { needsSetup, query } from '@raygun/sdk';

import type { HelixPage } from './parse';

export const API = 'https://api.twitch.tv/helix';
export const TOKEN = 'https://id.twitch.tv/oauth2/token';

const SETUP =
  "Add the Client ID and Client Secret of an application you registered at dev.twitch.tv/console in this source's settings";
const REFUSED =
  "Twitch didn't accept the Client ID and Client Secret: check them in this source's settings";

/** Where the token is kept between runs, with the Client ID it was made for. */
const KEPT = 'appToken';

type Token = { clientId: string; value: string; expiresAt: number };

let token: Token | null = null;

const setting = (key: string) => {
  const value = raygun.settings.get(key);
  return typeof value === 'string' ? value.trim() : '';
};

const credentials = () => {
  const clientId = setting('clientId');
  const clientSecret = setting('clientSecret');
  if (!clientId || !clientSecret) needsSetup(SETUP);
  return { clientId, clientSecret };
};

const usable = (kept: Token | null, clientId: string): kept is Token =>
  kept !== null && kept.clientId === clientId && kept.expiresAt - Date.now() > 60_000;

/** The app access token: kept while it lasts, else a new one. */
const appToken = async (fresh: boolean): Promise<string> => {
  const { clientId, clientSecret } = credentials();
  if (!fresh) {
    if (usable(token, clientId)) return token.value;
    const kept = raygun.storage.get<Token>(KEPT);
    if (usable(kept, clientId)) {
      token = kept;
      return kept.value;
    }
  }
  const response = await raygun.http.fetch(TOKEN, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: query({
      client_id: clientId,
      client_secret: clientSecret,
      grant_type: 'client_credentials',
    }),
  });
  if ([400, 401, 403].includes(response.status)) needsSetup(REFUSED);
  if (!response.ok) throw new Error(`Twitch's sign-in answered HTTP ${response.status}`);
  const answer = await response.json<{ access_token?: string; expires_in?: number }>();
  if (!answer.access_token) throw new Error("Twitch's sign-in gave no token");
  token = {
    clientId,
    value: answer.access_token,
    expiresAt: Date.now() + (answer.expires_in ?? 3600) * 1000,
  };
  try {
    raygun.storage.set(KEPT, token);
  } catch {
    // Kept for this run only.
  }
  return token.value;
};

type Params = Record<string, string | number | boolean | readonly string[] | null | undefined>;

/** One Helix request; an expired or revoked token is replaced once. */
export const helix = async <T>(path: string, params: Params): Promise<HelixPage<T>> => {
  const { clientId } = credentials();
  const url = `${API}/${path}?${query(params)}`;
  const send = async (fresh: boolean) =>
    raygun.http.fetch(url, {
      headers: { 'Client-Id': clientId, Authorization: `Bearer ${await appToken(fresh)}` },
    });
  let response = await send(false);
  if (response.status === 401) response = await send(true);
  if (response.status === 401 || response.status === 403) needsSetup(REFUSED);
  if (response.status === 429)
    throw new Error('Twitch is limiting requests: try again in a minute');
  if (!response.ok) throw new Error(`Twitch answered HTTP ${response.status} for ${path}`);
  return response.json<HelixPage<T>>();
};
