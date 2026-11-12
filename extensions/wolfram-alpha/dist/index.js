// dev.raygun.wolfram-alpha 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var needsSetup = (reason) => {
    const host = raygun;
    if (typeof host.needsSetup === "function") host.needsSetup(reason);
    throw new Error(reason);
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");

  // src/index.ts
  var API = "https://api.wolframalpha.com/v2/query";
  var SITE = "https://www.wolframalpha.com";
  var MOST_ROWS = 12;
  var appId = () => {
    const typed = raygun.settings.get("appId");
    const id = typeof typed === "string" ? typed.trim() : "";
    if (!id) {
      return needsSetup(
        "Wolfram|Alpha needs your own AppID: create one at developer.wolframalpha.com and enter it in this source's settings"
      );
    }
    return id;
  };
  var textOf = (pod) => (pod.subpods ?? []).map((subpod) => typeof subpod.plaintext === "string" ? subpod.plaintext.trim() : "").filter((text) => text !== "").join("\n");
  var pageOf = (words) => `${SITE}/input?${query({ i: words })}`;
  registerProvider(
    defineProvider({
      kind: "answers",
      id: "answers",
      name: "Wolfram|Alpha",
      async answer({ query: words }) {
        const id = appId();
        const units = raygun.settings.get("units") === "nonmetric" ? "nonmetric" : "metric";
        const response = await raygun.http.fetch(
          `${API}?${query({ appid: id, input: words, output: "json", format: "plaintext", units })}`
        );
        if (response.status === 401 || response.status === 403) {
          return needsSetup("Wolfram|Alpha refused the AppID: check it in this source's settings");
        }
        if (!response.ok) throw new Error(`HTTP ${response.status} from Wolfram|Alpha`);
        const result = (await response.json()).queryresult;
        const error = result?.error;
        if (error && typeof error === "object") {
          const message = typeof error.msg === "string" ? error.msg : "an error";
          if (/appid/i.test(message)) {
            return needsSetup(
              `Wolfram|Alpha refused the AppID (${message}): check it in this source's settings`
            );
          }
          throw new Error(`Wolfram|Alpha answered with ${message}`);
        }
        if (!result?.success) return { answers: [] };
        const pods = (result.pods ?? []).filter((pod) => !pod.error && textOf(pod) !== "");
        const input = pods.find((pod) => pod.id === "Input");
        const main = pods.find((pod) => pod.primary) ?? pods.find((pod) => pod.id === "Result") ?? pods.find((pod) => pod !== input);
        if (!main) return { answers: [] };
        const page = pageOf(words);
        const answer = {
          interpretation: input ? textOf(input) : null,
          result: textOf(main),
          rows: pods.filter((pod) => pod !== input && pod !== main).slice(0, MOST_ROWS).map((pod) => ({ label: pod.title?.trim() || "More", value: textOf(pod) })),
          url: page,
          credits: [{ name: "Wolfram|Alpha", url: page }]
        };
        return { answers: [answer] };
      }
    })
  );
})();
