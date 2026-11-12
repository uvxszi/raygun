//! Exchange rates: the European Central Bank's euro reference rates, published each working day
//! around 16:00 CET for about 30 currencies (reuse allowed with the source named; they're for
//! information, not transactions). Fetched at most every few hours, kept in
//! `cache/answers/ecb-rates.json` so conversions work offline with the day they're from.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::dictionary::cache::now_ms;
use crate::dictionary::polite::{Fetched, Polite};

pub const ECB_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
/// The page people read the rates on.
pub const ECB_PAGE: &str = "https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html";
pub const ECB_NAME: &str = "European Central Bank";
/// Rates fetched this recently aren't fetched again.
const FRESH: Duration = Duration::from_secs(4 * 60 * 60);
/// A failed fetch isn't tried again for this long.
const RETRY: Duration = Duration::from_secs(10 * 60);

/// The rates of one day: units of each currency per euro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    /// The day they're for: `2026-11-10`.
    pub date: String,
    pub rates: HashMap<String, f64>,
    #[serde(default)]
    pub fetched_ms: u64,
}

/// The ECB's daily file read: the day and each currency's rate.
pub fn parse(xml: &str) -> Option<Rates> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut date = None;
    let mut rates = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) => {
                if element.local_name().as_ref() != "Cube" {
                    continue;
                }
                let mut currency = None;
                let mut rate = None;
                for attribute in element.attributes().flatten() {
                    let value = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .ok()?
                        .trim()
                        .to_owned();
                    match attribute.key.local_name().as_ref() {
                        "time" => date = Some(value),
                        "currency" => currency = Some(value),
                        "rate" => rate = value.parse::<f64>().ok(),
                        _ => {}
                    }
                }
                if let (Some(currency), Some(rate)) = (currency, rate)
                    && currency.len() == 3
                    && currency.bytes().all(|byte| byte.is_ascii_uppercase())
                    && rate.is_finite()
                    && rate > 0.0
                {
                    rates.insert(currency, rate);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    let date = date?;
    (!rates.is_empty()).then_some(Rates {
        date,
        rates,
        fetched_ms: 0,
    })
}

pub struct ExchangeRates {
    file: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    rates: Option<Rates>,
    loaded: bool,
    failed_ms: Option<u64>,
}

impl ExchangeRates {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            file: dir.join("ecb-rates.json"),
            state: Mutex::new(State::default()),
        }
    }

    /// The latest rates: kept ones while fresh (or offline, or when the ECB can't be reached),
    /// else fetched now. `online` says whether fetching is allowed.
    pub async fn get(&self, polite: &Polite, online: bool) -> Result<Rates, String> {
        let mut state = self.state.lock().await;
        if !state.loaded {
            state.loaded = true;
            if let Ok(bytes) = tokio::fs::read(&self.file).await {
                state.rates = serde_json::from_slice(&bytes).ok();
            }
        }
        let now = now_ms();
        let fresh = state
            .rates
            .as_ref()
            .is_some_and(|rates| now.saturating_sub(rates.fetched_ms) < FRESH.as_millis() as u64);
        let resting = state
            .failed_ms
            .is_some_and(|failed| now.saturating_sub(failed) < RETRY.as_millis() as u64);
        if fresh || !online || resting {
            return state
                .rates
                .clone()
                .ok_or_else(|| "Exchange rates need a connection the first time".to_owned());
        }
        match polite
            .get(ECB_URL, "application/xml, text/xml", ECB_NAME)
            .await
        {
            Fetched::Body(xml) => match parse(&xml) {
                Some(mut rates) => {
                    rates.fetched_ms = now;
                    if let Ok(json) = serde_json::to_vec(&rates) {
                        if let Some(parent) = self.file.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        let partial = self.file.with_extension("part");
                        if tokio::fs::write(&partial, &json).await.is_ok() {
                            let _ = tokio::fs::rename(&partial, &self.file).await;
                        }
                    }
                    state.rates = Some(rates.clone());
                    state.failed_ms = None;
                    Ok(rates)
                }
                None => {
                    state.failed_ms = Some(now);
                    state
                        .rates
                        .clone()
                        .ok_or_else(|| format!("{ECB_NAME}'s rates couldn't be read"))
                }
            },
            Fetched::NotFound => {
                state.failed_ms = Some(now);
                state
                    .rates
                    .clone()
                    .ok_or_else(|| format!("{ECB_NAME}'s rates weren't found"))
            }
            Fetched::Failed(issue) => {
                state.failed_ms = Some(now);
                state.rates.clone().ok_or(issue)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_daily_file_is_read() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
  <gesmes:subject>Reference rates</gesmes:subject>
  <gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
  <Cube>
    <Cube time='2026-11-10'>
      <Cube currency='USD' rate='1.0823'/>
      <Cube currency='JPY' rate='163.45'/>
      <Cube currency='GBP' rate='0.8412'/>
      <Cube currency='bad' rate='1'/>
      <Cube currency='XXX' rate='-1'/>
    </Cube>
  </Cube>
</gesmes:Envelope>"#;
        let rates = parse(xml).unwrap();
        assert_eq!(rates.date, "2026-11-10");
        assert_eq!(rates.rates.len(), 3);
        assert_eq!(rates.rates["USD"], 1.0823);
        assert!(parse("<html>not rates</html>").is_none());
    }
}
