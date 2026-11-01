//! `podcast.directory` providers (M37): finding podcasts by name and listing popular ones. The app
//! subscribes to the feed URL a directory returns and reads the feed itself.

use serde_json::Value;

use crate::ExtensionHost;

pub const PODCAST_DIRECTORY: &str = "podcast.directory";
/// Most results kept from one call.
const MAX_RESULTS: usize = 100;

/// A podcast a directory found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastResult {
    pub title: String,
    pub author: Option<String>,
    /// http(s) only.
    pub feed_url: String,
    pub artwork_url: Option<String>,
    pub description: Option<String>,
}

impl ExtensionHost {
    pub async fn podcast_search(
        &self,
        provider: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<PodcastResult>, String> {
        let value = self
            .call(
                provider,
                "search",
                serde_json::json!({ "query": query, "limit": limit }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_results(&value))
    }

    /// Popular podcasts, for browsing before searching.
    pub async fn podcast_top(
        &self,
        provider: &str,
        limit: u32,
    ) -> Result<Vec<PodcastResult>, String> {
        let value = self
            .call(provider, "top", serde_json::json!({ "limit": limit }))
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_results(&value))
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn url(value: Option<&Value>) -> Option<String> {
    text(value).filter(|url| url.starts_with("https://") || url.starts_with("http://"))
}

/// Results as a provider returned them; entries without a title or an http(s) feed are dropped,
/// and so are repeats of a feed.
pub fn parse_results(value: &Value) -> Vec<PodcastResult> {
    let mut seen = std::collections::HashSet::new();
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(PodcastResult {
                title: text(item.get("title"))?,
                author: text(item.get("author")),
                feed_url: url(item.get("feedUrl"))?,
                artwork_url: url(item.get("artworkUrl")),
                description: text(item.get("description")),
            })
        })
        .filter(|result| seen.insert(result.feed_url.clone()))
        .take(MAX_RESULTS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_valid_results_once() {
        let value = serde_json::json!([
            { "title": " Show ", "author": "Host", "feedUrl": "https://f.example/a.xml",
              "artworkUrl": "https://i.example/a.jpg" },
            { "title": "Same feed", "feedUrl": "https://f.example/a.xml" },
            { "title": "No feed" },
            { "title": "Bad feed", "feedUrl": "javascript:alert(1)" },
            { "feedUrl": "https://f.example/untitled.xml" },
        ]);
        let results = parse_results(&value);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Show");
        assert_eq!(
            results[0].artwork_url.as_deref(),
            Some("https://i.example/a.jpg")
        );
        assert!(parse_results(&serde_json::json!({ "not": "a list" })).is_empty());
    }
}
