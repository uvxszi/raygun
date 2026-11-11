// Entry of the shim bundle that goes in front of every converted Nuclear plugin. It takes Raygun's
// natives for plugins, installs the web polyfills (`fetch` too: plugins written for a webview call
// it, and it goes through `raygun.http.fetch` like `api.Http.fetch`) and exposes `run` (see
// run.ts) as the global `__nuclearShim`.
import { pluginFetch } from './api';
import { createCrypto } from './crypto';
import { createDomParser } from './dom';
import { takeNatives } from './native';
import { installPolyfills } from './polyfills';
import { run } from './run';

const scope = globalThis as Record<string, unknown>;
const natives = takeNatives(scope);
installPolyfills(scope);
const provide = (name: string, value: unknown) => {
  if (scope[name] === undefined) scope[name] = value;
};
provide('fetch', pluginFetch);
if (natives) {
  provide('DOMParser', createDomParser(natives));
  provide('crypto', createCrypto(natives));
}

export { run };
