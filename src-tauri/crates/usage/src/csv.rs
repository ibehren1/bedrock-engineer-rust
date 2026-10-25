//! CSV export of recorded calls.

use crate::EventRow;
use chrono::{Local, TimeZone};

const HEADER: &str = "time,day,model,model_id,region,service,feature,agent_id,session_id,\
input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,units,unit,estimated_cost_usd,\
tokens_estimated,source";

/// Quote a field when it contains a comma, quote or line break (RFC 4180).
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn local_time(ts_ms: i64) -> String {
    Local
        .timestamp_millis_opt(ts_ms)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

pub fn events_csv<'a>(rows: impl Iterator<Item = &'a EventRow>) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for r in rows {
        let cells = [
            local_time(r.ts),
            r.local_day.clone(),
            r.model_name.clone(),
            r.model_id.clone(),
            r.region.clone(),
            r.service.clone(),
            r.feature.clone(),
            r.agent_id.clone().unwrap_or_default(),
            r.session_id.clone().unwrap_or_default(),
            r.input_tokens.to_string(),
            r.output_tokens.to_string(),
            r.cache_read_tokens.to_string(),
            r.cache_write_tokens.to_string(),
            r.units.map(|u| u.to_string()).unwrap_or_default(),
            r.unit_kind.clone().unwrap_or_default(),
            r.cost_usd.map(|c| format!("{c:.6}")).unwrap_or_default(),
            r.estimated.to_string(),
            r.source.clone(),
        ];
        out.push_str(&cells.iter().map(|c| field(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(field("plain"), "plain");
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
