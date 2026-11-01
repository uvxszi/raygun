# M34 — Model manager, AI providers, secrets store

**Relevant docs:** ARCHITECTURE §3.2 (layout: `secrets.db`, `models/`), §8 (AI subsystem), §10 (what never syncs).

## Goal
- **The groundwork for Phase 5:**
  - a place to keep API keys safely;
  - a manager for downloadable AI models;
  - a registry of translation providers that the OCR and translation milestones (M36–M38) build on.
- Everything is reachable from a new Settings → AI page.

## Design

### New crate `raygun-ai` (pure logic, no Tauri)
- **`secrets::Vault`:** an encrypted name → value store in `secrets.db`.
  - **File format:** `RGS1` + a 12-byte random nonce + ChaCha20-Poly1305 ciphertext of a JSON map. It is rewritten through a `.part` file and a rename, with a new nonce on every write.
  - **Key:** 32 random bytes, kept in `raygun.db`'s device-local `local_meta` (`ai.secretsKey`).
    - It never syncs and isn't in backups. A copied data folder keeps working (portable mode).
  - **Damaged or unreadable file** (wrong key, damaged file): the vault starts empty, logs a warning and is overwritten on the next save. It doesn't fail the app.
  - **Values never leave Rust,** except the last four characters, shown as a hint.
- **`models`:** the model catalogue and its files.
  - **Built-in catalogue** (`catalog.json`, compiled in): id, name, kind (`upscale` for now), description, licence and licence URL, source page, and files with a URL, SHA-256 and exact size.
    - **Catalogue entries:**
      - Real-CUGAN 2× conservative;
      - Real-CUGAN 2× denoise;
      - Real-CUGAN 4× conservative;
      - waifu2x CUnet 2× (noise 1).
    - **Source:** each entry is pinned to a commit of nihui's MIT ncnn-vulkan repositories.
  - **Installed models:** each is a folder `models/<id>/` holding exactly the catalogue's files at their exact sizes.
  - **Download:**
    - Files stream to `models/<id>.part/`, hashing as they go.
    - Bytes beyond the declared size abort the download, and a wrong size or hash fails it.
    - The folder is renamed into place only when every file checks out. Progress is reported through a callback.
  - **Delete** removes the folder.
- **`translate`:** provider configs and one call, `translate(client, provider, key, texts, from, to) -> Vec<String>`.
  - **Claude** (Messages API over HTTP, as Rust has no official SDK):
    - **Request:** `POST {base}/v1/messages` with `x-api-key` and `anthropic-version: 2026-10-21`.
    - **Settings:** default model `claude-sonnet-5-5` (ARCHITECTURE §8.2), adaptive thinking (the model's default), and no sampling parameters.
    - **Output:** structured JSON (`output_config.format` with a `{translations: string[]}` schema).
    - **Failures:** a `refusal` or `max_tokens` stop reason is an error.
  - **OpenAI-compatible** (also covers local Ollama and LM Studio):
    - `POST {base}/chat/completions`, sending the bearer key only if one is set.
    - The reply's JSON object is read leniently, because local models often wrap it in prose.
  - **DeepL:** `POST {base}/v2/translate` with `DeepL-Auth-Key`.
    - The base is chosen from the key: `:fx` keys use `api-free.deepl.com`.
    - Target codes are upper case, with the `EN-US` and `PT-BR` variants.
  - **Checks:** every provider must return one translation per input, else it's an error. Error messages include the provider's own error text.
  - Batching, glossaries and chapter context are M37.

### App wiring (`raygun-app/src/ai.rs`, `AiService`)
- **Provider list:** device-local JSON in `local_meta` (`ai.providers`), as `{id (UUIDv7), kind, name, baseUrl?, model?}`.
  - A provider's key is the vault entry `provider:<id>`. Deleting a provider deletes its key.
- **Model downloads:**
  - Downloads run one per model at a time (a second request for the same model is refused).
  - Progress goes out as `AiModelEvent { id, receivedBytes, totalBytes, state }` with state `downloading`, `installed` or `failed` (plus the error).
- **Commands:**
  - `ai_providers`;
  - `ai_provider_save(input)`: a `null` key keeps the stored one and `""` removes it;
  - `ai_provider_delete`;
  - `ai_provider_test(id)`: translates a sample sentence from English to Japanese and returns it;
  - `ai_models`, `ai_model_download`, `ai_model_delete`.

### UI: Settings → AI (`/settings/ai`)
- **Settings page:** an "AI" card shows how many translators and models are set up and links to the AI page.
- **Translators:**
  - Add or edit a provider in a form: kind, name, API key (a password field showing "Saved key ••••1234"), base URL and model, with each kind's defaults as placeholders.
  - Each provider can be tested (showing the translation or the error) and deleted.
- **Models:**
  - Each catalogue model shows its size, licence (linked) and description.
  - It offers Download with a progress bar, or Delete once installed.

## Out of scope
- Using the models: upscaling is M35 and M39. OCR models are M36, when the ONNX runtime arrives.
- Batching and glossaries (M37). Google Translate. Syncing secrets.

## Done-check
- **Rust:**
  - **Vault:** round trip; the file contains no plain text; a wrong key gives an empty vault; a deleted entry is gone.
  - **Catalogue:** valid (unique ids, 64-hex hashes, https URLs).
  - **Download** from a local server: installs with a correct hash. A wrong hash or a too-long file leaves nothing behind. Delete removes the model.
  - **Each translator kind** against a local mock: request shape and headers, the parsed reply, a count mismatch, and a provider error (including Claude's refusal).
  - **App:** saving a provider without a key keeps the old key, and `""` clears it.
- **Vitest:** the AI page lists providers with key hints, saves a new provider with its key, tests one, and downloads a model with the progress shown.
- 📱 **On the emulator:**
  - download Real-CUGAN 2× and see it installed under `models/`, then delete it;
  - add a Claude provider with a dummy key; Test shows Anthropic's authentication error;
  - `secrets.db` holds no plain-text key.

## Decisions
- **The vault key lives in `local_meta`,** not the OS keystore.
  - **Why not the keystore:** Android Keystore and Windows DPAPI would tie keys to one install and break the portable data folder.
  - **What it protects against:** the encrypted file is safe from casual reading, backups and sync, which never include `local_meta` or `secrets.db`. It does not protect against someone holding the whole data folder.
- **Claude goes over raw HTTP** with `anthropic-version: 2026-10-21`, as Rust has no official Anthropic SDK.
  - The request sends no sampling parameters (removed in that version) and leaves thinking at the model's default.
  - The answer comes back as structured JSON through `output_config.format`.
  - The default model is `claude-sonnet-5-5`, as ARCHITECTURE §8.2 sets for translation volume. The model is a setting on each translator.
- **The provider list is device-local,** like the keys, so a synced config never points at a key the other device lacks.
- **Models are verified while downloading** (size cap plus SHA-256) and only renamed into place when complete. "Installed" means every file is present at its exact size, so the listing never re-hashes.
- **f64 sizes are `number | null` in TypeScript** (specta's NaN rule), so the UI defaults them to 0, as `BackupCard` does.
- **The progress bar is styled with WebKit pseudo-elements,** because Android's WebView draws a plain grey bar for `<progress>` and ignores `accent-color`.

## Verification (2026-10-31, Pixel_4 emulator)
- **Checks:** all pass.
  - Rust: `cargo test --workspace` (plus 8 new tests in `raygun-ai` and 1 in the app) and clippy.
  - Front end: `pnpm lint`, `pnpm type-check` and `pnpm test` (131 app tests, including 6 new AI tests).
- **Models:**
  - Settings showed "0 translators · 0 models installed", and the AI page listed the four models at 2.5–2.7 MB with MIT links.
  - Real-CUGAN 2× downloaded with a progress bar into `models/realcugan-up2x-conservative/`, with both files at their catalogue sizes, and the log showed "model installed".
  - Delete emptied `models/`.
  - After restyling, the progress bar's dark rounded track showed at 0%. Downloads on the emulator finish too quickly to capture it part-filled.
- **Translators:**
  - Adding Claude with a made-up key showed "Claude · key ••••9876".
  - Test reached Anthropic, which answered "the translator answered 401: invalid x-api-key".
  - The key survived an app restart. Deleting the translator shrank `secrets.db` to 34 bytes, an encrypted empty map.
- **Secrets file:** `secrets.db` pulled from the device starts with `RGS1` and contains none of the key's text.
