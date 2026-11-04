//! Audiobookshelf servers (audiobookshelf.org, API at api.audiobookshelf.org): the books of
//! their book libraries as an `audiobook.directory`, each book's audio files as its chapters.
//! Requests sign in with a bearer token: an API key, or the access token a password sign-in
//! gives (it lasts an hour, so it's refreshed in the background while the server is connected,
//! and `raygun_net::server_auth` hands the current one to chapter streams and downloads).

use std::cell::RefCell;
use std::rc::Rc;

use base64::Engine;
use reqwest::Url;
use serde_json::{Value, json};

use super::http::{Http, id_arg, is_denied, number_arg, text_arg, under};
use crate::failure::Failure;

/// Books per page of a list.
const PAGE_SIZE: u32 = 24;

/// How an Audiobookshelf server takes the sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Sign {
    Key(String),
    Password { username: String, password: String },
}

/// The tokens a password sign-in holds.
#[derive(Debug, Default)]
struct Tokens {
    access: Option<String>,
    refresh: Option<String>,
}

/// An Audiobookshelf server.
pub(crate) struct Audiobookshelf {
    http: Http,
    base: Url,
    sign: Sign,
    tokens: Rc<RefCell<Tokens>>,
    /// Called with the bearer token whenever it changes, to register it for streams.
    on_token: Rc<dyn Fn(&str)>,
    /// The book libraries, once listed.
    libraries: RefCell<Option<Vec<String>>>,
}

/// What `/login` and `/auth/refresh` answer: `user.accessToken` (2.26 and newer, with
/// `user.refreshToken` when asked for) or the older `user.token`.
fn tokens_of(answer: &Value) -> Tokens {
    let user = answer.get("user").unwrap_or(&Value::Null);
    let text = |key: &str| {
        user.get(key)
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
    };
    Tokens {
        access: text("accessToken").or_else(|| text("token")),
        refresh: text("refreshToken"),
    }
}

/// Signs in with a user name and password; the tokens to use.
async fn login(http: &Http, base: &Url, username: &str, password: &str) -> Result<Tokens, Failure> {
    let request = http
        .client()
        .post(under(base, "login"))
        .header("x-return-tokens", "true")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json!({ "username": username, "password": password }).to_string());
    let answer: Value = http.json(request).await?;
    let tokens = tokens_of(&answer);
    if tokens.access.is_none() {
        return Err(super::http::unreadable(
            base.host_str().unwrap_or_default(),
            &"no token in the sign-in answer",
        ));
    }
    Ok(tokens)
}

/// Checks a sign-in when a server is added: the user's name on the server.
pub(crate) async fn check(http: &Http, base: &Url, sign: &Sign) -> Result<String, Failure> {
    let token = match sign {
        Sign::Key(key) => key.clone(),
        Sign::Password { username, password } => login(http, base, username, password)
            .await?
            .access
            .unwrap_or_default(),
    };
    let me: Value = http
        .json(http.client().get(under(base, "api/me")).bearer_auth(&token))
        .await?;
    Ok(me
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned())
}

/// "13:06:44" for a length in seconds.
fn runtime(seconds: f64) -> Option<String> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let total = seconds.round() as u64;
    Some(format!(
        "{}:{:02}:{:02}",
        total / 3600,
        total / 60 % 60,
        total % 60
    ))
}

impl Audiobookshelf {
    pub fn new(http: Http, base: Url, sign: Sign, on_token: Rc<dyn Fn(&str)>) -> Self {
        let tokens = Tokens {
            access: match &sign {
                Sign::Key(key) => Some(key.clone()),
                Sign::Password { .. } => None,
            },
            refresh: None,
        };
        if let Some(token) = &tokens.access {
            on_token(token);
        }
        Self {
            http,
            base,
            sign,
            tokens: Rc::new(RefCell::new(tokens)),
            on_token,
            libraries: RefCell::new(None),
        }
    }

    /// Whether the sign-in needs keeping fresh (a password's tokens expire; a key doesn't).
    pub fn expires(&self) -> bool {
        matches!(self.sign, Sign::Password { .. })
    }

    /// Signs in again, or refreshes the access token when a refresh token is held.
    pub async fn renew(&self) -> Result<String, Failure> {
        let Sign::Password { username, password } = &self.sign else {
            return Ok(self.tokens.borrow().access.clone().unwrap_or_default());
        };
        let refresh = self.tokens.borrow().refresh.clone();
        let renewed = match refresh {
            Some(refresh) => {
                let request = self
                    .http
                    .client()
                    .post(under(&self.base, "auth/refresh"))
                    .header("x-refresh-token", refresh)
                    .header("x-return-tokens", "true");
                match self.http.json::<Value>(request).await {
                    Ok(answer) if tokens_of(&answer).access.is_some() => tokens_of(&answer),
                    _ => login(&self.http, &self.base, username, password).await?,
                }
            }
            None => login(&self.http, &self.base, username, password).await?,
        };
        let access = renewed.access.clone().unwrap_or_default();
        {
            let mut tokens = self.tokens.borrow_mut();
            tokens.access = renewed.access;
            if renewed.refresh.is_some() {
                tokens.refresh = renewed.refresh;
            }
        }
        (self.on_token)(&access);
        Ok(access)
    }

    async fn token(&self) -> Result<String, Failure> {
        let held = self.tokens.borrow().access.clone();
        match held {
            Some(token) => Ok(token),
            None => self.renew().await,
        }
    }

    /// A GET under `/api`, signing in again once if the token was refused.
    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Failure> {
        let mut url = under(&self.base, &format!("api/{path}"));
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let token = self.token().await?;
        let first = self
            .http
            .json::<Value>(self.http.client().get(url.clone()).bearer_auth(&token))
            .await;
        match first {
            Err(failure) if is_denied(&failure) && self.expires() => {
                let token = self.renew().await?;
                self.http
                    .json(self.http.client().get(url).bearer_auth(&token))
                    .await
            }
            other => other,
        }
    }

    async fn libraries(&self) -> Result<Vec<String>, Failure> {
        if let Some(known) = self.libraries.borrow().clone() {
            return Ok(known);
        }
        let answer = self.get("libraries", &[]).await?;
        let libraries: Vec<String> = answer
            .get("libraries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|library| library.get("mediaType").and_then(Value::as_str) == Some("book"))
            .filter_map(|library| library.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect();
        *self.libraries.borrow_mut() = Some(libraries.clone());
        Ok(libraries)
    }

    /// A library item as an audiobook; books without audio (ebooks only) are left out.
    fn book(&self, item: &Value) -> Option<Value> {
        let id = item.get("id").and_then(Value::as_str)?;
        let media = item.get("media")?;
        let metadata = media.get("metadata").unwrap_or(&Value::Null);
        let duration = media
            .get("duration")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let files = media
            .get("numAudioFiles")
            .or_else(|| media.get("numTracks"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                media
                    .get("audioFiles")
                    .and_then(Value::as_array)
                    .map_or(0, |files| files.len() as u64)
            });
        if duration <= 0.0 && files == 0 {
            return None;
        }
        let author = metadata
            .get("authorName")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                let names: Vec<&str> = metadata
                    .get("authors")?
                    .as_array()?
                    .iter()
                    .filter_map(|author| author.get("name").and_then(Value::as_str))
                    .collect();
                (!names.is_empty()).then(|| names.join(", "))
            });
        let cover = media
            .get("coverPath")
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty())
            .then(|| {
                under(
                    &self.base,
                    &format!("api/items/{}/cover", super::http::segment(id)),
                )
                .to_string()
            });
        Some(json!({
            "id": id,
            "title": metadata.get("title").and_then(Value::as_str).unwrap_or_default(),
            "author": author,
            "artworkUrl": cover,
            "runtime": runtime(duration),
            "language": metadata.get("language").and_then(Value::as_str),
            "description": metadata.get("description").and_then(Value::as_str),
        }))
    }

    /// A page of each book library's items, `extra` narrowing them.
    async fn items(
        &self,
        page: u32,
        sort: &str,
        extra: Option<(&str, String)>,
    ) -> Result<Value, Failure> {
        let mut books = Vec::new();
        let mut more = false;
        for library in self.libraries().await? {
            let mut query = vec![
                ("limit", PAGE_SIZE.to_string()),
                ("page", (page - 1).to_string()),
                ("sort", sort.to_owned()),
                ("desc", "1".to_owned()),
                ("minified", "1".to_owned()),
            ];
            if let Some((key, value)) = &extra {
                query.push((key, value.clone()));
            }
            let answer = self
                .get(
                    &format!("libraries/{}/items", super::http::segment(&library)),
                    &query,
                )
                .await?;
            let results = answer
                .get("results")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let total = answer
                .get("total")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            more |= total > u64::from(page * PAGE_SIZE);
            books.extend(results.iter().filter_map(|item| self.book(item)));
        }
        Ok(json!({ "books": books, "hasMore": more }))
    }

    pub async fn books_call(&self, method: &str, args: &Value) -> Result<Value, Failure> {
        let page = number_arg(args, "page", 1, 1..=10_000);
        match method {
            "popular" | "latest" => self.items(page, "addedAt", None).await,
            "search" => {
                let Some(query) = text_arg(args, "query") else {
                    return Ok(json!({ "books": [], "hasMore": false }));
                };
                if page > 1 {
                    return Ok(json!({ "books": [], "hasMore": false }));
                }
                let mut books = Vec::new();
                for library in self.libraries().await? {
                    let answer = self
                        .get(
                            &format!("libraries/{}/search", super::http::segment(&library)),
                            &[("q", query.clone()), ("limit", "25".to_owned())],
                        )
                        .await?;
                    books.extend(
                        answer
                            .get("book")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|found| self.book(found.get("libraryItem")?)),
                    );
                }
                Ok(json!({ "books": books, "hasMore": false }))
            }
            "subject" => {
                let Some(subject) = text_arg(args, "subject") else {
                    return Ok(json!({ "books": [], "hasMore": false }));
                };
                // The library's own spelling of the genre.
                let wanted = subject.to_lowercase();
                let mut genre = None;
                for library in self.libraries().await? {
                    let data = self
                        .get(
                            &format!("libraries/{}/filterdata", super::http::segment(&library)),
                            &[],
                        )
                        .await
                        .unwrap_or(Value::Null);
                    genre = data
                        .get("genres")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .find(|genre| genre.to_lowercase() == wanted)
                        .or_else(|| {
                            data.get("genres")
                                .and_then(Value::as_array)?
                                .iter()
                                .filter_map(Value::as_str)
                                .find(|genre| genre.to_lowercase().contains(&wanted))
                        })
                        .map(str::to_owned);
                    if genre.is_some() {
                        break;
                    }
                }
                let Some(genre) = genre else {
                    return Ok(json!({ "books": [], "hasMore": false }));
                };
                let filter = format!(
                    "genres.{}",
                    base64::engine::general_purpose::STANDARD.encode(genre.as_bytes())
                );
                self.items(page, "addedAt", Some(("filter", filter))).await
            }
            "chapters" => self.chapters(&id_arg(args, "id")?).await,
            other => Err(Failure::said(format!("books has no method {other}"))),
        }
    }

    async fn chapters(&self, id: &str) -> Result<Value, Failure> {
        let item = self
            .get(
                &format!("items/{}", super::http::segment(id)),
                &[("expanded", "1".to_owned())],
            )
            .await?;
        let book = self
            .book(&item)
            .ok_or_else(|| Failure::said("This book has no audio on the server"))?;
        let media = item.get("media").unwrap_or(&Value::Null);
        let mut files: Vec<&Value> = media
            .get("audioFiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|file| file.get("exclude").and_then(Value::as_bool) != Some(true))
            .filter(|file| file.get("invalid").and_then(Value::as_bool) != Some(true))
            .collect();
        files.sort_by_key(|file| {
            file.get("index")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX)
        });
        let single = files.len() == 1;
        let chapters: Vec<Value> = files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                let ino = file.get("ino").and_then(Value::as_str)?;
                let named = file
                    .get("metaTags")
                    .and_then(|tags| tags.get("tagTitle"))
                    .and_then(Value::as_str)
                    .filter(|title| !title.trim().is_empty())
                    .map(str::to_owned)
                    .or_else(|| {
                        let name = file.get("metadata")?.get("filename")?.as_str()?;
                        Some(name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_owned())
                    });
                let title = if single {
                    book["title"].as_str().unwrap_or_default().to_owned()
                } else {
                    named.unwrap_or_else(|| format!("Part {}", index + 1))
                };
                Some(json!({
                    "id": ino,
                    "title": title,
                    "audioUrl": under(&self.base, &format!("api/items/{}/file/{}", super::http::segment(id), super::http::segment(ino))).to_string(),
                    "audioType": file.get("mimeType").and_then(Value::as_str),
                    "durationMs": file.get("duration").and_then(Value::as_f64).map(|seconds| (seconds * 1000.0) as i64),
                }))
            })
            .collect();
        if chapters.is_empty() {
            return Err(Failure::said("This book has no audio on the server"));
        }
        let link = under(&self.base, &format!("item/{}", super::http::segment(id)));
        Ok(json!({
            "title": book["title"],
            "author": book["author"],
            "description": book["description"],
            "artworkUrl": book["artworkUrl"],
            "link": link.to_string(),
            "chapters": chapters,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_read_as_hours() {
        assert_eq!(runtime(47_204.0).as_deref(), Some("13:06:44"));
        assert_eq!(runtime(59.4).as_deref(), Some("0:00:59"));
        assert_eq!(runtime(0.0), None);
    }

    #[test]
    fn tokens_come_from_either_field() {
        let new = tokens_of(
            &json!({ "user": { "accessToken": "a", "refreshToken": "r", "token": "old" } }),
        );
        assert_eq!(
            (new.access.as_deref(), new.refresh.as_deref()),
            (Some("a"), Some("r"))
        );
        let old = tokens_of(&json!({ "user": { "token": "old" } }));
        assert_eq!(old.access.as_deref(), Some("old"));
        assert!(tokens_of(&json!({})).access.is_none());
    }
}
