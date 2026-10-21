//! What the agents spend (spec « Steering », AI cost; ADR 0058): the requests Claude Code reports
//! in its telemetry, summed over the time a Timonier session was open. Tokens are the measure;
//! the cost in dollars is the agent's own estimate, an indication.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One request of an agent to its model, as its telemetry reports it (`claude_code.api_request`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRequest {
    /// When it was made, Unix time in milliseconds.
    pub at_ms: i64,
    pub model: String,
    /// `main`, `subagent` or `auxiliary`, when the agent says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_source: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
    /// The agent's estimate of its cost, in millionths of a dollar.
    pub cost_usd_micros: u64,
}

/// The tokens of a session, or of a task (ADR 0058). Absent where nothing was received: the views
/// say « not measured », never an estimate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TokenUsage {
    /// Requests to the model.
    pub requests: u32,
    pub input: u64,
    pub output: u64,
    /// Read from the prompt cache.
    pub cache_read: u64,
    /// Written to the prompt cache.
    pub cache_creation: u64,
    /// The agent's estimate, in millionths of a dollar: an indication.
    #[ts(type = "number")]
    pub cost_usd_micros: u64,
}

impl TokenUsage {
    pub fn add(&mut self, other: &TokenUsage) {
        self.requests = self.requests.saturating_add(other.requests);
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_creation = self.cache_creation.saturating_add(other.cache_creation);
        self.cost_usd_micros = self.cost_usd_micros.saturating_add(other.cost_usd_micros);
    }

    /// Every token, cache included.
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_creation)
    }
}

/// The requests made in `[from_ms, to_ms]`, summed; `None` when there is none.
pub fn within<'a>(
    requests: impl IntoIterator<Item = &'a AgentRequest>,
    from_ms: i64,
    to_ms: i64,
) -> Option<TokenUsage> {
    let mut usage = TokenUsage::default();
    for request in requests {
        if request.at_ms < from_ms || request.at_ms > to_ms {
            continue;
        }
        usage.add(&TokenUsage {
            requests: 1,
            input: request.input,
            output: request.output,
            cache_read: request.cache_read,
            cache_creation: request.cache_creation,
            cost_usd_micros: request.cost_usd_micros,
        });
    }
    (usage.requests > 0).then_some(usage)
}

/// Unix time in milliseconds of a time [`crate::tracking::utc`] wrote (`2026-10-21T18:07:58Z`);
/// `None` for anything else.
pub fn parse_utc(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' {
        return None;
    }
    if b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hour, minute, second) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    if second > 60 {
        return None;
    }
    // Days since the epoch of a civil date (Howard Hinnant's `days_from_civil`).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400) + hour * 3600 + minute * 60 + second) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::utc;

    fn request(at_ms: i64, input: u64) -> AgentRequest {
        AgentRequest {
            at_ms,
            model: "claude-opus-5-5".into(),
            query_source: Some("main".into()),
            input,
            output: 10,
            cache_read: 100,
            cache_creation: 1,
            cost_usd_micros: 2500,
        }
    }

    #[test]
    fn a_time_written_reads_back() {
        for millis in [0, 1_792_000_078_000, 951_782_400_000, 4_107_542_399_000] {
            assert_eq!(parse_utc(&utc(millis)), Some(millis));
        }
        assert_eq!(parse_utc("2026-10-21T18:07:58Z"), Some(1_792_606_078_000));
        for wrong in [
            "",
            "2026-10-21 18:07:58Z",
            "2026-13-01T00:00:00Z",
            "2026-10-21T18:07:58",
        ] {
            assert_eq!(parse_utc(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn only_the_requests_of_the_interval_count() {
        let requests = [
            request(999, 1),
            request(1000, 2),
            request(2000, 3),
            request(2001, 4),
        ];
        assert_eq!(
            within(&requests, 1000, 2000),
            Some(TokenUsage {
                requests: 2,
                input: 5,
                output: 20,
                cache_read: 200,
                cache_creation: 2,
                cost_usd_micros: 5000,
            })
        );
        // Nothing received: not measured, not zero.
        assert_eq!(within(&requests, 3000, 4000), None);
        assert_eq!(within(&requests, 1000, 2000).unwrap().total(), 227);
    }
}
