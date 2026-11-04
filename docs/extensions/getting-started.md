# Getting started

This page makes a comics source for Pepper&Carrot, David Revoy's webcomic under CC BY 4.0, and
runs it in Raygun. Every kind works the same way; only the template differs.

## What you need

- A checkout of Raygun, with its packages installed. The SDK and `raygun-ext` live in it
  (`packages/sdk`) and aren't published on npm.
- Node.js 22 or newer and pnpm.
- Rust (`cargo`, from [rustup.rs](https://rustup.rs)) for `raygun-ext test`, which runs your
  extension in Raygun's own extension host.

```sh
git clone https://github.com/uvxszi/raygun.git
cd raygun
pnpm install
```

## Create the extension

From the checkout's root, make the extension in a folder of your own, here `my-extensions` beside
the checkout:

```sh
pnpm raygun-ext init ../my-extensions/peppercarrot --kind manga.source --id dev.example.peppercarrot --name "Pepper&Carrot"
```

- `--kind` picks the template: one of the [kinds](README.md#kinds). Without it, `init` makes a
  `music.metadata` source.
- `--id` is the extension's id for good: reverse-DNS in lower case, starting with a domain or name
  you own (`dev.yourname.peppercarrot`). Updates match by id.
- `--name` is what Raygun shows.

`init` links the SDK from this checkout (`"@raygun/sdk": "link:…"` in `package.json`), so the
checkout must stay where it is; Raygun's own extensions live in the checkout's `extensions/`
folder, where the SDK comes from the workspace instead. Install the extension's packages once, so
your editor finds the SDK's types and `raygun-ext` runs inside its folder:

```sh
cd ../my-extensions/peppercarrot
pnpm install
```

Every command below runs inside the extension's folder. From the checkout's root,
`pnpm raygun-ext <command> ../my-extensions/peppercarrot` does the same.

The folder holds:

| File                    | What it is                                                         |
| ----------------------- | ------------------------------------------------------------------ |
| `raygun-extension.json` | The [manifest](manifest.md): id, version, kinds, network access    |
| `src/index.ts`          | The source; everything it imports is bundled into one script       |
| `package.json`          | Scripts for `build`, `validate`, `dev:android` and `type-check`    |
| `tsconfig.json`         | QuickJS's ES2022, without browser or Node types                    |

## Build and check

```sh
pnpm build
pnpm validate
```

`build` writes `dist/index.js`. `validate` checks everything Raygun checks, and more:

- the manifest, with the rules Raygun applies when it loads one (it refuses unknown fields);
- the types, against the provider contracts;
- the providers the script registers: their kinds, ids and calls, with "did you mean" for a call
  Raygun never makes;
- requests to hosts that `permissions.net` doesn't list, and browser or Node APIs QuickJS lacks.

Errors make Raygun refuse the extension or fail its calls; `pack` won't publish an extension
with errors. Warnings are likely mistakes.

## Try a call

```sh
pnpm exec raygun-ext test
pnpm exec raygun-ext test peppercarrot chapters id=peppercarrot
pnpm exec raygun-ext test peppercarrot pages chapterId=ep01_Potion-of-Flight --set language=French
```

`test` builds the extension and runs one call in Raygun's extension host, the way the app runs it:
the same sandbox, network rules and limits. Without a provider it lists what the extension
registers. Arguments are `key=value` words over the ones the app passes that call (a value is
read as JSON when it can be, so `page=2` is a number), or one JSON object. It prints:

- the answer, as JSON (on stdout, so it can be piped to a file or `jq`);
- the extension's log, and the stack trace of an error;
- what Raygun keeps of the answer, and why it drops the rest;
- how long the call took, against the time Raygun gives it.

`--set <key>=<value>` sets one of the extension's settings, `--nsfw` turns NSFW content on, and
`--data <folder>` keeps the extension's storage and cookies between runs. The first run compiles
the host, which takes a few minutes; later runs start at once.

Under the hood, `test` runs the host's `call` example from the checkout's root, which takes the
arguments as JSON:

```sh
cargo run -p raygun-ext-host --example call -- ../my-extensions/peppercarrot peppercarrot chapters '{"id":"peppercarrot"}'
```

In Windows PowerShell, prefer `raygun-ext test`: its `key=value` words need no JSON quoting.

## Run it in Raygun

### Desktop

Raygun's desktop build keeps its data in a `raygun-data` folder beside the executable, or in the
folder `RAYGUN_HOME` names. `dev` rebuilds on every save and installs into its `extensions/`:

```sh
pnpm exec raygun-ext dev --into "/path/to/raygun-data/extensions"
```

A build of Raygun made with `pnpm dev` keeps its data in `target/debug/raygun-data` and reloads
installed extensions by itself. A release build reloads them from Browse → Extensions →
Installed → ⋯ → Reload.

### Android

```sh
pnpm dev:android
```

This installs every good build into a debug build of Raygun on the emulator or a phone
connected over USB (`--serial <device>` picks one), through `adb`; debug builds reload installed
extensions by themselves. `adb` is found through `ADB`, `ANDROID_HOME` or the `PATH`.

## Install it from a repository

A repository is a folder with an `index.json` and the packages it lists. Pack the extension into
one, here `my-repo` beside `my-extensions`:

```sh
pnpm exec raygun-ext pack --repo ../../my-repo --name "My sources"
```

Then add it in Raygun: Browse → ⋯ → Extension repositories → Add repository, and install the
extension from Browse → Extensions.

- **Desktop**: the index's path is enough: `/path/to/my-repo/index.json`.
- **Android emulator**: serve the folder, forward the port, and add
  `http://localhost:8765/index.json` (plain `http` is allowed only for `localhost` and
  `10.0.2.2`):

  ```sh
  cd ../../my-repo
  python -m http.server 8765
  adb reverse tcp:8765 tcp:8765
  ```

[Repositories](repositories.md) covers publishing one on the web.

## Next

- The page of your extension's kind, from the [kinds](README.md#kinds).
- The [host API](host-api.md): requests, HTML, storage, settings.
- The [manifest](manifest.md): permissions, settings, ratings.
