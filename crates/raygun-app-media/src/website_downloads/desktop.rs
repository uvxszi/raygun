//! The desktop browser's download question: a site window's download (cancelled in the webview)
//! is offered here with its name, size and type, to save in Downloads/Raygun or a picked folder.

use tauri::{AppHandle, Manager};

use super::{Destination, FileRequest};

/// Labels of the question's buttons.
const TO_DOWNLOADS: &str = "Download to Downloads/Raygun";
const CHOOSE: &str = "Choose folder…";
const CANCEL: &str = "Cancel";

/// `1.4 MB`.
pub fn size_text(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Asks where a site window's download goes, then starts it. Runs on its own thread: reading the
/// webview's cookies waits on the UI thread, and so do the dialogs.
pub fn offer(app: AppHandle, label: String, mut request: FileRequest) {
    std::thread::spawn(move || {
        let window = app.get_webview_window(&label);
        if request.cookies.is_none()
            && let Some(window) = &window
            && let Ok(url) = tauri::Url::parse(&request.url)
            && let Ok(cookies) = window.cookies_for_url(url)
        {
            let header = cookies
                .iter()
                .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
                .collect::<Vec<_>>()
                .join("; ");
            request.cookies = (!header.is_empty()).then_some(header);
        }
        let name = request.file_name();
        let mut lines = vec![name.clone()];
        let details: Vec<String> = [
            request.size.filter(|size| *size > 0.0).map(size_text),
            request
                .mime
                .as_deref()
                .and_then(|mime| mime.split(';').next())
                .map(str::trim)
                .filter(|mime| !mime.is_empty())
                .map(str::to_owned),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !details.is_empty() {
            lines.push(details.join(" · "));
        }
        if let Some(host) = request
            .referer
            .as_deref()
            .and_then(|page| tauri::Url::parse(page).ok())
            .and_then(|page| page.host_str().map(str::to_owned))
        {
            lines.push(format!("From {host}"));
        }
        let mut dialog = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("Download file")
            .set_description(lines.join("\n"))
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                TO_DOWNLOADS.to_owned(),
                CHOOSE.to_owned(),
                CANCEL.to_owned(),
            ));
        if let Some(window) = &window {
            dialog = dialog.set_parent(window);
        }
        let answer = dialog.show();
        let destination = match answer {
            rfd::MessageDialogResult::Custom(label) if label == TO_DOWNLOADS => {
                Destination::Downloads
            }
            rfd::MessageDialogResult::Yes => Destination::Downloads,
            rfd::MessageDialogResult::Custom(label) if label == CHOOSE => {
                let mut picker = rfd::FileDialog::new().set_title("Save the file in");
                if let Some(window) = &window {
                    picker = picker.set_parent(window);
                }
                let Some(folder) = picker.pick_folder() else {
                    return;
                };
                let name = folder
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| folder.to_string_lossy().into_owned());
                Destination::Folder {
                    uri: folder.to_string_lossy().into_owned(),
                    name,
                }
            }
            _ => return,
        };
        request.destination = destination;
        super::accept(&app, request);
    });
}
