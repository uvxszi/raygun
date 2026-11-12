//! What something is, from Wikipedia (CC BY-SA 4.0, credited with a link to the article): the
//! thing found by name on Wikidata, its article in the app's language (else English), and the
//! article's summary from the REST API (`page/summary`).

use raygun_model::{Answer, AnswerCredit, AnswerKind, AnswerText};
use serde_json::Value;

use super::wikidata::{self, Entity, Hit};
use crate::dictionary::polite::{Fetched, Polite};

pub const NAME: &str = "Wikipedia";
pub const LICENSE: &str = "CC BY-SA 4.0";
pub const LICENSE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/";
/// The longest summary kept.
const MAX_EXTRACT: usize = 2400;

/// An article's summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub title: String,
    pub description: Option<String>,
    pub extract: String,
    pub image: Option<String>,
    pub page: String,
}

pub fn parse_summary(answer: &Value) -> Option<Summary> {
    if answer.get("type").and_then(Value::as_str) == Some("disambiguation") {
        return None;
    }
    let text = |key: &str| {
        answer
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let extract: String = text("extract")?.chars().take(MAX_EXTRACT).collect();
    let page = answer
        .pointer("/content_urls/desktop/page")
        .and_then(Value::as_str)
        .filter(|page| page.starts_with("https://"))?
        .to_owned();
    Some(Summary {
        title: text("title")?,
        description: text("description"),
        extract,
        image: answer
            .pointer("/thumbnail/source")
            .and_then(Value::as_str)
            .filter(|source| source.starts_with("https://"))
            .map(str::to_owned),
        page,
    })
}

/// The article of `entity` to read: the app's Wikipedia's, else English's.
pub fn article(entity: &Entity, language: &str) -> Option<(String, String)> {
    let wiki = wikidata::wiki_language(language);
    let site = format!("{}wiki", wiki.replace('-', "_"));
    entity
        .sitelinks
        .get(&site)
        .map(|title| (wiki.clone(), title.clone()))
        .or_else(|| {
            entity
                .sitelinks
                .get("enwiki")
                .map(|title| ("en".to_owned(), title.clone()))
        })
}

pub async fn summary(polite: &Polite, wiki: &str, title: &str) -> Result<Option<Summary>, String> {
    let path: String = title.replace(' ', "_");
    let encoded: String = url::form_urlencoded::byte_serialize(path.as_bytes())
        .collect::<String>()
        .replace('+', "%20");
    let address = format!("https://{wiki}.wikipedia.org/api/rest_v1/page/summary/{encoded}");
    match polite.get(&address, "application/json", NAME).await {
        Fetched::Body(body) => Ok(serde_json::from_str(&body)
            .ok()
            .as_ref()
            .and_then(parse_summary)),
        Fetched::NotFound => Ok(None),
        Fetched::Failed(issue) => Err(issue),
    }
}

/// The summary as an answer: the article's start, its picture and a link to it.
pub fn answer_of(summary: Summary, hit: &Hit, entity: &Entity) -> Answer {
    let mut answer = super::blank(AnswerKind::Summary);
    answer.engine = "wikipedia".to_owned();
    let label = entity.label.clone().unwrap_or_else(|| hit.label.clone());
    let description = summary
        .description
        .clone()
        .or_else(|| entity.description.clone())
        .unwrap_or_else(|| hit.description.clone());
    answer.interpretation = Some(AnswerText::plain(if description.is_empty() {
        label.clone()
    } else {
        format!("{label} ({description})")
    }));
    answer.result = AnswerText::plain(summary.title.clone());
    answer.copy = summary.extract.clone();
    answer.summary = Some(summary.extract);
    answer.image_url = summary.image;
    answer.url = Some(summary.page.clone());
    answer.credits.push(AnswerCredit {
        name: NAME.to_owned(),
        url: Some(summary.page),
        license: Some(LICENSE.to_owned()),
        license_url: Some(LICENSE_URL.to_owned()),
    });
    answer.credits.push(AnswerCredit {
        name: wikidata::NAME.to_owned(),
        url: Some(format!("https://www.wikidata.org/wiki/{}", hit.id)),
        license: Some(wikidata::LICENSE.to_owned()),
        license_url: Some(wikidata::LICENSE_URL.to_owned()),
    });
    answer
}

/// What the thing named `subject` is: its summary, with the thing found.
pub async fn about(
    polite: &Polite,
    subject: &str,
    language: &str,
) -> Result<Option<(Answer, Hit, Entity)>, String> {
    let hits = wikidata::search(polite, subject).await?;
    let Some(hit) = hits.into_iter().next() else {
        return Ok(None);
    };
    let entities = wikidata::entities(polite, std::slice::from_ref(&hit.id), language).await?;
    let entity = entities.get(&hit.id).cloned().unwrap_or_default();
    let Some((wiki, title)) = article(&entity, language) else {
        return Ok(None);
    };
    let Some(found) = summary(polite, &wiki, &title).await? else {
        return Ok(None);
    };
    Ok(Some((answer_of(found, &hit, &entity), hit, entity)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn summaries_are_read() {
        let answer = json!({
            "type": "standard",
            "title": "Mount Everest",
            "description": "Earth's highest mountain above sea level",
            "extract": "Mount Everest is Earth's highest mountain above sea level.",
            "thumbnail": { "source": "https://upload.wikimedia.org/everest.jpg", "width": 320 },
            "content_urls": { "desktop": { "page": "https://en.wikipedia.org/wiki/Mount_Everest" } }
        });
        let summary = parse_summary(&answer).unwrap();
        assert_eq!(summary.title, "Mount Everest");
        assert_eq!(
            summary.image.as_deref(),
            Some("https://upload.wikimedia.org/everest.jpg")
        );
        assert!(
            parse_summary(&json!({ "type": "disambiguation", "title": "x", "extract": "y" }))
                .is_none()
        );
        let entity = Entity {
            sitelinks: [("enwiki".to_owned(), "Mount Everest".to_owned())].into(),
            ..Entity::default()
        };
        assert_eq!(
            article(&entity, "de"),
            Some(("en".to_owned(), "Mount Everest".to_owned()))
        );
    }
}
