//! Minimal iCal (RFC 5545) reader for calendar events, with common recurrence rules.
//!
//! Times with a TZID are read as local time. Times in UTC (`Z`) are converted to local time.

use crate::model::Event;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc, Weekday};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct Raw {
    uid: String,
    summary: String,
    start: Option<(NaiveDateTime, bool)>,
    end: Option<NaiveDateTime>,
    duration: Option<Duration>,
    rrule: Option<String>,
    exdates: Vec<NaiveDateTime>,
    recurrence_id: Option<NaiveDateTime>,
    cancelled: bool,
}

fn parse_dt(params: &str, value: &str) -> Option<(NaiveDateTime, bool)> {
    let v = value.trim();
    if params.contains("VALUE=DATE") && !params.contains("VALUE=DATE-TIME") || v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return Some((d.and_time(NaiveTime::MIN), true));
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        let local = DateTime::<Utc>::from_naive_utc_and_offset(n, Utc).with_timezone(&Local).naive_local();
        return Some((local, false));
    }
    NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok().map(|n| (n, false))
}

fn parse_duration(v: &str) -> Option<Duration> {
    let v = v.trim().trim_start_matches('+');
    let (neg, v) = v.strip_prefix('-').map(|r| (true, r)).unwrap_or((false, v));
    let v = v.strip_prefix('P')?;
    let mut total = Duration::zero();
    let mut num = String::new();
    for c in v.chars() {
        match c {
            '0'..='9' => num.push(c),
            'T' => {}
            'W' | 'D' | 'H' | 'M' | 'S' => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += match c {
                    'W' => Duration::weeks(n),
                    'D' => Duration::days(n),
                    'H' => Duration::hours(n),
                    'M' => Duration::minutes(n),
                    _ => Duration::seconds(n),
                };
            }
            _ => return None,
        }
    }
    Some(if neg { -total } else { total })
}

fn weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

fn add_months(d: NaiveDate, months: i32) -> Option<NaiveDate> {
    let total = d.year() * 12 + d.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, d.day())
}

/// Start times of every occurrence of `rule` that begins on or before `to`.
fn expand(start: NaiveDateTime, rule: &str, to: NaiveDate) -> Vec<NaiveDateTime> {
    let mut freq = "";
    let mut interval = 1i64;
    let mut count: Option<usize> = None;
    let mut until: Option<NaiveDateTime> = None;
    let mut byday: Vec<Weekday> = Vec::new();
    for part in rule.split(';') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        match k {
            "FREQ" => freq = v,
            "INTERVAL" => interval = v.parse().unwrap_or(1).max(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_dt("", v).map(|(d, all_day)| if all_day { d + Duration::days(1) } else { d }),
            "BYDAY" => byday = v.split(',').filter_map(|d| weekday(&d[d.len().saturating_sub(2)..])).collect(),
            _ => {}
        }
    }
    let limit = until.map(|u| u.min(to.and_time(NaiveTime::MIN) + Duration::days(1))).unwrap_or(to.and_time(NaiveTime::MIN) + Duration::days(1));
    let max = count.unwrap_or(usize::MAX);
    let mut out = Vec::new();
    let time = start.time();
    let push = |dt: NaiveDateTime, out: &mut Vec<NaiveDateTime>| -> bool {
        if dt > limit || out.len() >= max {
            return false;
        }
        if dt >= start {
            out.push(dt);
        }
        true
    };
    match freq {
        "DAILY" => {
            let mut d = start;
            while push(d, &mut out) {
                d += Duration::days(interval);
            }
        }
        "WEEKLY" => {
            let days = if byday.is_empty() { vec![start.weekday()] } else { byday };
            let week0 = start.date() - Duration::days(start.weekday().num_days_from_monday() as i64);
            let mut w = 0i64;
            'outer: loop {
                let monday = week0 + Duration::weeks(w * interval);
                let mut week: Vec<NaiveDate> = days.iter().map(|d| monday + Duration::days(d.num_days_from_monday() as i64)).collect();
                week.sort();
                for d in week {
                    if !push(d.and_time(time), &mut out) {
                        break 'outer;
                    }
                }
                w += 1;
                if w > 2000 {
                    break;
                }
            }
        }
        "MONTHLY" | "YEARLY" => {
            let step = if freq == "MONTHLY" { interval as i32 } else { 12 * interval as i32 };
            let mut k = 0;
            while k < 5000 {
                if let Some(d) = add_months(start.date(), step * k) {
                    if !push(d.and_time(time), &mut out) {
                        break;
                    }
                }
                k += 1;
            }
        }
        _ => out.push(start),
    }
    out
}

pub fn parse(text: &str, from: NaiveDate, to: NaiveDate) -> Vec<Event> {
    // Unfold continuation lines.
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }

    let mut raws = Vec::new();
    let mut cur: Option<Raw> = None;
    let mut depth = 0;
    for line in &lines {
        if line == "BEGIN:VEVENT" {
            cur = Some(Raw::default());
            depth = 0;
            continue;
        }
        if line == "END:VEVENT" {
            raws.extend(cur.take());
            continue;
        }
        let Some(ev) = cur.as_mut() else { continue };
        if line.starts_with("BEGIN:") {
            depth += 1;
        } else if line.starts_with("END:") {
            depth -= 1;
        }
        if depth > 0 {
            continue; // Skip VALARM and other nested blocks.
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let (name, params) = key.split_once(';').unwrap_or((key, ""));
        match name {
            "UID" => ev.uid = value.to_string(),
            "SUMMARY" => ev.summary = unescape(value),
            "DTSTART" => ev.start = parse_dt(params, value),
            "DTEND" => ev.end = parse_dt(params, value).map(|d| d.0),
            "DURATION" => ev.duration = parse_duration(value),
            "RRULE" => ev.rrule = Some(value.to_string()),
            "EXDATE" => ev.exdates.extend(value.split(',').filter_map(|v| parse_dt(params, v).map(|d| d.0))),
            "RECURRENCE-ID" => ev.recurrence_id = parse_dt(params, value).map(|d| d.0),
            "STATUS" => ev.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }

    // Instances that a RECURRENCE-ID entry replaces.
    let overridden: HashSet<(String, NaiveDateTime)> =
        raws.iter().filter_map(|r| r.recurrence_id.map(|d| (r.uid.clone(), d))).collect();

    let mut seen: HashMap<(String, NaiveDateTime), ()> = HashMap::new();
    let mut out = Vec::new();
    let lo = from.and_time(NaiveTime::MIN);
    for r in &raws {
        let Some((start, all_day)) = r.start else { continue };
        if r.cancelled {
            continue;
        }
        let len = match (r.end, r.duration) {
            (Some(e), _) => e - start,
            (None, Some(d)) => d,
            (None, None) if all_day => Duration::days(1),
            _ => Duration::zero(),
        };
        let starts = match (&r.rrule, r.recurrence_id) {
            (Some(rule), None) => expand(start, rule, to),
            _ => vec![start],
        };
        for s in starts {
            if r.recurrence_id.is_none() && (r.exdates.contains(&s) || overridden.contains(&(r.uid.clone(), s))) {
                continue;
            }
            if s + len < lo || s.date() > to {
                continue;
            }
            if seen.insert((r.uid.clone() + &r.summary, s), ()).is_some() {
                continue;
            }
            out.push(Event { title: r.summary.clone(), start: s, end: s + len, all_day });
        }
    }
    out.sort_by_key(|e| e.start);
    out
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_class_with_exdate() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:ITM415 - 011 - LEC\r\nDTSTART;TZID=America/Toronto:20260908T150000\r\nDTEND;TZID=America/Toronto:20260908T180000\r\nRRULE:FREQ=WEEKLY;BYDAY=TU;UNTIL=20261208T235959Z\r\nEXDATE;TZID=America/Toronto:20261013T150000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let from = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let ev = parse(ics, from, to);
        let days: Vec<u32> = ev.iter().map(|e| e.start.day()).collect();
        assert_eq!(days, vec![6, 20, 27]);
        assert_eq!(ev[0].end - ev[0].start, Duration::hours(3));
    }

    #[test]
    fn all_day_event() {
        let ics = "BEGIN:VEVENT\nUID:b\nSUMMARY:halloween\nDTSTART;VALUE=DATE:20261031\nDTEND;VALUE=DATE:20261101\nEND:VEVENT\n";
        let d = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let ev = parse(ics, d, d);
        assert_eq!(ev.len(), 1);
        assert!(ev[0].all_day);
    }
}
