//! Where an export goes, from the settings and the request.

use raygun_model::{ExportDestination, ExportRequest, SavePrefs};

use crate::output::Place;

/// Where saved files go, from Settings → Downloads.
pub fn place_for(prefs: &SavePrefs, destination: ExportDestination) -> Result<Place, String> {
    match destination {
        ExportDestination::Downloads => Ok(Place::Downloads),
        ExportDestination::Share => Ok(Place::Share),
        ExportDestination::Folder => match &prefs.folder_uri {
            Some(uri) => Ok(Place::Folder {
                uri: uri.clone(),
                name: prefs.folder_name.clone().unwrap_or_else(|| "Folder".into()),
            }),
            None => Err("Choose a folder first".into()),
        },
    }
}

/// Where a request's files go: the folder chosen in the export sheet, else as [`place_for`].
pub fn place_for_request(prefs: &SavePrefs, request: &ExportRequest) -> Result<Place, String> {
    match (&request.destination, &request.folder) {
        (ExportDestination::Folder, Some(folder)) => Ok(Place::Folder {
            uri: folder.uri.clone(),
            name: folder.name.clone(),
        }),
        _ => place_for(prefs, request.destination),
    }
}
