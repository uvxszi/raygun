// __NAME__: answers (`answers`) for Search, from Open-Meteo (https://open-meteo.com, CC BY 4.0,
// free for non-commercial use): the weather now in a place, for "weather in Paris" or "Tokyo
// weather". Raygun asks every answers provider on the Answers pill, and in Search → All (with
// `card: true`) for a question nothing else answered; answer `{ answers: [] }` for anything else.
// Start from here for an answer engine of your own.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test weather answer query="weather in Paris" language=en card=false`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { AnswerItem } from '@raygun/sdk';

const GEOCODING = 'https://geocoding-api.open-meteo.com/v1/search';
const FORECAST = 'https://api.open-meteo.com/v1/forecast';

type Places = {
  results?: {
    name: string;
    country?: string;
    admin1?: string;
    latitude: number;
    longitude: number;
  }[];
};

type Forecast = {
  current?: {
    time: string;
    temperature_2m: number;
    apparent_temperature?: number;
    relative_humidity_2m?: number;
    wind_speed_10m?: number;
    weather_code?: number;
  };
  current_units?: Record<string, string>;
};

/** WMO weather codes, as Open-Meteo gives them. */
const CONDITIONS: Record<number, string> = {
  0: 'clear sky',
  1: 'mainly clear',
  2: 'partly cloudy',
  3: 'overcast',
  45: 'fog',
  48: 'rime fog',
  51: 'light drizzle',
  53: 'drizzle',
  55: 'dense drizzle',
  61: 'light rain',
  63: 'rain',
  65: 'heavy rain',
  71: 'light snow',
  73: 'snow',
  75: 'heavy snow',
  80: 'rain showers',
  81: 'rain showers',
  82: 'violent rain showers',
  95: 'thunderstorm',
  96: 'thunderstorm with hail',
  99: 'thunderstorm with hail',
};

/** The place in "weather in Paris", "what's the weather in Paris" or "Paris weather". */
const placeOf = (words: string): string | null => {
  const text = words.trim().replace(/[?!.]+$/, '');
  const asked = /^(?:what(?:'s| is) the )?(?:weather|temperature)(?: now| today)? in (.+)$/i.exec(
    text,
  );
  if (asked?.[1]) return asked[1].trim();
  const named = /^(.+?) (?:weather|temperature)(?: now| today)?$/i.exec(text);
  return named?.[1]?.trim() ?? null;
};

registerProvider(
  defineProvider({
    kind: 'answers',
    id: 'weather',
    name: '__NAME__',

    async answer({ query: words, language }) {
      const place = placeOf(words);
      if (!place) return { answers: [] };
      const places = await fetchJson<Places>(
        `${GEOCODING}?${query({ name: place, count: 1, language, format: 'json' })}`,
      );
      const found = places.results?.[0];
      if (!found) return { answers: [] };
      const current =
        'temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code';
      const forecast = await fetchJson<Forecast>(
        `${FORECAST}?${query({ latitude: found.latitude, longitude: found.longitude, current, timezone: 'auto' })}`,
      );
      const now = forecast.current;
      if (!now) return { answers: [] };
      const unit = (key: string, fallback: string) => forecast.current_units?.[key] ?? fallback;
      const condition = CONDITIONS[now.weather_code ?? -1];
      const rows: { label: string; value: string }[] = [];
      if (now.apparent_temperature !== undefined) {
        rows.push({
          label: 'Feels like',
          value: `${now.apparent_temperature} ${unit('apparent_temperature', '°C')}`,
        });
      }
      if (now.relative_humidity_2m !== undefined) {
        rows.push({
          label: 'Humidity',
          value: `${now.relative_humidity_2m} ${unit('relative_humidity_2m', '%')}`,
        });
      }
      if (now.wind_speed_10m !== undefined) {
        rows.push({
          label: 'Wind',
          value: `${now.wind_speed_10m} ${unit('wind_speed_10m', 'km/h')}`,
        });
      }
      const where = [found.name, found.admin1, found.country].filter(Boolean).join(', ');
      const answer: AnswerItem = {
        kind: 'fact',
        interpretation: `weather now in ${where}`,
        result: `${now.temperature_2m} ${unit('temperature_2m', '°C')}${condition ? `, ${condition}` : ''}`,
        rows,
        url: 'https://open-meteo.com',
        credits: [
          {
            name: 'Open-Meteo',
            url: 'https://open-meteo.com',
            license: 'CC BY 4.0',
            licenseUrl: 'https://creativecommons.org/licenses/by/4.0/',
          },
        ],
        asOf: now.time,
      };
      return { answers: [answer] };
    },
  }),
);
