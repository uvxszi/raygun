# Crash logs and other logs

Raygun keeps its logs on the device and never sends them anywhere. This page says where they are and how to get them off a phone or a PC.

## In Raygun

- **About → Crash logs** lists what went wrong, newest first: when, what kind, and one line. Open a record to read all of it, then **Share** it (any app: mail, chat, a notes app) or **Save** it in `Download/Raygun/logs`. **Clear crash logs** removes them all, after a second tap.
- **About → Logs folder**:
  - On a PC, it opens the folder that holds the daily logs and the `crashes` folder.
  - On Android, it saves every log as one zip in `Download/Raygun/logs`. The zip holds the daily logs (`raygun.<date>.log`, older days gzipped) and the crash records.
- If Raygun's screen itself stops working, Raygun shows its own error screen instead of the page. Its buttons:
  - **Try again**
  - **See logs**, which opens the record just kept. On Android this works even when the page is gone, because the screen is drawn natively.
  - **Start in safe mode**
  - **Clear caches**

### What a crash record holds

Each record is a text file in `logs/crashes/` in Raygun's data folder. It starts with `Kind:`, `Time:`, `Version:`, `Pid:` (and on Android `Process:`) and `Summary:` lines, then the details. The kinds are:

| Kind | What happened | Details |
|---|---|---|
| `panic` | A part of Raygun's core stopped (a Rust panic). Raygun often goes on without it. | Message, place in the code, thread, backtrace, and which sources were loading or running |
| `java` | An exception on the Android side ended Raygun. | Stack trace, device memory |
| `renderer` | The page's renderer process ended: it crashed, or Android ended it to free memory. | Device memory, whether Raygun was in front |
| `java`, `native`, `anr`, `memory`, `exit` | How Android says one of Raygun's processes ended (Android 11 and later): a crash, a native crash, not responding, the system ending a process in use for memory, a signal. Read when a process starts, when Raygun comes to the front and when Crash logs opens. A crash Raygun recorded itself at the time (same process, same moment) isn't recorded again, and neither are processes Raygun ended itself. | A native crash keeps its tombstone beside the record (`<time>-native.tombstone.pb`; read it with AOSP's `pbtombstone`). An ANR keeps its thread dump. |
| `start` | Raygun stopped within 20 s of starting and nothing else explained it. An update or reinstall, Force stop or swiping Raygun away isn't counted. | The sources that were at work |
| `ui`, `page` | The page met an error it couldn't get past, or Raygun's page couldn't load. | |

### Safe start

When Raygun stops within 20 s of starting, or its page stops working, the next start is a **safe start**. Stopping that way by an update or reinstall, Force stop or swiping Raygun away doesn't count: Android says how the process ended, and Raygun checks whether it was updated since. You can also ask for one from the error screen. In a safe start:

- The page doesn't restore the answers it saved.
- Downloads, transcriptions and the last queue resume only after 20 s of healthy running.
- Extensions load one at a time.

Raygun then says it recovered from a crash, names the sources that were at work, and offers to turn them off. When two starts in a row stop with the same source at work, Raygun holds that source back: it isn't loaded until you turn it on again in About → Crash logs.

## Android: Logcat in Android Studio

Logcat shows what Raygun and Android log while it runs. It is the only way to see a crash that happens before Raygun can write anything.

1. Connect the phone with **USB**:
   1. Turn on Developer options: Settings → About phone → tap Build number seven times.
   2. Turn on Developer options → USB debugging.
   3. Connect the cable and accept the prompt on the phone.

   Or connect over **Wi-Fi** (Android 11 and later):
   1. Turn on Developer options → Wireless debugging.
   2. In Android Studio, open Device Manager → Pair Devices Using Wi-Fi, and scan the QR code from the phone's Wireless debugging screen.
2. Open **View → Tool Windows → Logcat** and pick the phone.
3. Filter by the package: `package:app.raygun.player`. Add `level:error` to see only errors.
   - Java crashes are tagged `AndroidRuntime`.
   - Native crashes are written by the system under the tag `DEBUG`, which a package filter can hide. Clear the filter, or use `tag:DEBUG`, when Raygun closes without a crash record.
4. Reproduce the problem, then select the lines (or all of them) and copy them into a text file.

Without Android Studio, the same with `adb`:

```sh
adb logcat -v time > raygun-logcat.txt   # everything, until Ctrl+C
adb logcat --pid=$(adb shell pidof -s app.raygun.player)
adb bugreport raygun-bugreport.zip       # includes tombstones and ANR traces
```

Debug builds can also read Raygun's own files directly: `adb shell "run-as app.raygun.player ls logs/crashes"`.

## On a PC

- The portable build keeps its data in `raygun-data` beside `Raygun.exe`, and the logs in `raygun-data/logs`.
- In installed mode, the data is in `%LOCALAPPDATA%\app.raygun.player`, and the logs in its `logs` folder.

## Uninstalling and keeping your data

- **Android**: the system's uninstall dialog offers **Keep app data**. Tick it, and the library, downloads and settings are there again when Raygun is installed again.
- **Windows setup (NSIS)**: the uninstaller keeps Raygun's data unless **Delete the application data** is ticked. When it is ticked, the uninstaller removes `raygun-data` beside `Raygun.exe` and `%LOCALAPPDATA%\app.raygun.player`. Updates never delete data.
- **Windows MSI** never deletes Raygun's data. To remove it, delete the folders above yourself.
- **Portable build**: there's nothing to uninstall. Delete the folder, including `raygun-data` if you don't want to keep the data.
