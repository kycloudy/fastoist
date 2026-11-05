//! Google Calendar: desktop OAuth (PKCE with a loopback redirect) and the Calendar API.

use crate::model::{Event, GCal};
use base64::Engine;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

type Res<T> = Result<T, String>;

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REVOKE: &str = "https://oauth2.googleapis.com/revoke";
const API: &str = "https://www.googleapis.com/calendar/v3";
const SCOPES: &str = "https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const SIGN_IN_WAIT: Duration = Duration::from_secs(180);

/// Shown when Google refuses the saved refresh token.
pub const EXPIRED: &str = "Google sign-in expired. Sign in again in Settings.";

#[derive(Clone, Debug, Default)]
pub struct Creds {
    pub client_id: String,
    pub client_secret: String,
    pub refresh: String,
}

/// A signed-in session with a cached access token.
pub struct Session {
    pub creds: Creds,
    access: String,
    expires: Instant,
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .user_agent("fastoist/0.1")
        .build()
        .into()
}

// ---------- sign-in ----------

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn pkce() -> Pkce {
    let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    Pkce { challenge: challenge_for(&verifier), verifier }
}

pub fn challenge_for(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn auth_url(client_id: &str, redirect: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    let q: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect();
    format!("{AUTH}?{}", q.join("&"))
}

/// Query parameters of the first line of an HTTP request, such as `GET /?code=x HTTP/1.1`.
pub fn parse_callback(request: &str) -> HashMap<String, String> {
    let line = request.lines().next().unwrap_or("");
    let target = line.split_whitespace().nth(1).unwrap_or("");
    let query = target.split_once('?').map(|x| x.1).unwrap_or("");
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

fn page(msg: &str) -> String {
    let body = format!("<!doctype html><meta charset=utf-8><title>Fastoist</title><body style=\"font:16px system-ui;background:#1f1f1f;color:#eee;display:grid;place-items:center;height:100vh;margin:0\"><p>{msg}</p>");
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

/// Wait for the browser to come back to the loopback address. Returns the authorization code.
fn wait_for_code(listener: &TcpListener, state: &str) -> Res<String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + SIGN_IN_WAIT;
    loop {
        if Instant::now() > deadline {
            return Err("Google sign-in timed out. Try again.".into());
        }
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let params = parse_callback(&String::from_utf8_lossy(&buf[..n]));
        if let Some(err) = params.get("error") {
            let _ = stream.write_all(page("Sign-in was cancelled. You can close this tab.").as_bytes());
            return Err(format!("Google sign-in failed: {err}"));
        }
        match (params.get("code"), params.get("state")) {
            (Some(code), Some(s)) if s == state => {
                let _ = stream.write_all(page("Signed in. You can close this tab and return to Fastoist.").as_bytes());
                return Ok(code.clone());
            }
            // A stray request such as the favicon.
            _ => {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        }
    }
}

/// Run the whole sign-in. Blocks until the user finishes in the browser, so call it off the UI thread.
pub fn sign_in(agent: &ureq::Agent, client_id: &str, client_secret: &str) -> Res<Creds> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Cannot listen for Google: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let pk = pkce();
    let state = uuid::Uuid::new_v4().simple().to_string();
    open_browser(&auth_url(client_id, &redirect, &pk.challenge, &state));
    let code = wait_for_code(&listener, &state)?;
    let v = token_request(
        agent,
        &[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", pk.verifier.as_str()),
        ],
    )?;
    let refresh = v["refresh_token"].as_str().ok_or("Google did not return a refresh token. Try again.")?;
    Ok(Creds { client_id: client_id.into(), client_secret: client_secret.into(), refresh: refresh.into() })
}

fn token_request(agent: &ureq::Agent, form: &[(&str, &str)]) -> Res<Value> {
    let r = agent.post(TOKEN).send_form(form.iter().copied()).map_err(|e| format!("Network error: {e}"))?;
    finish(r).map_err(|e| if e.contains("invalid_grant") { EXPIRED.to_string() } else { e })
}

/// Turn a response into JSON, or into the error message Google sent.
fn finish(mut r: ureq::http::Response<ureq::Body>) -> Res<Value> {
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return Ok(v);
    }
    let msg = v["error"]["message"]
        .as_str()
        .or(v["error_description"].as_str())
        .or(v["error"].as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("HTTP {status}"));
    let code = v["error"].as_str().unwrap_or("");
    Err(format!("{msg} {code}").trim().to_string())
}

impl Session {
    pub fn new(creds: Creds) -> Self {
        Self { creds, access: String::new(), expires: Instant::now() }
    }

    fn token(&mut self, agent: &ureq::Agent) -> Res<String> {
        if self.access.is_empty() || Instant::now() + Duration::from_secs(60) > self.expires {
            let v = token_request(
                agent,
                &[
                    ("client_id", self.creds.client_id.as_str()),
                    ("client_secret", self.creds.client_secret.as_str()),
                    ("refresh_token", self.creds.refresh.as_str()),
                    ("grant_type", "refresh_token"),
                ],
            )?;
            self.access = v["access_token"].as_str().ok_or("Google sent no access token")?.to_string();
            self.expires = Instant::now() + Duration::from_secs(v["expires_in"].as_u64().unwrap_or(3600));
        }
        Ok(self.access.clone())
    }

    fn send(&mut self, agent: &ureq::Agent, method: &str, path: &str, query: &[(&str, String)], body: Option<Value>) -> Res<Value> {
        let auth = format!("Bearer {}", self.token(agent)?);
        let url = format!("{API}/{path}");
        let net = |e: ureq::Error| format!("Network error: {e}");
        let r = match (method, body) {
            ("GET", _) => {
                let mut req = agent.get(&url).header("Authorization", &auth);
                for (k, v) in query {
                    req = req.query(*k, v);
                }
                req.call().map_err(net)?
            }
            ("DELETE", _) => agent.delete(&url).header("Authorization", &auth).call().map_err(net)?,
            ("PATCH", Some(b)) => agent.patch(&url).header("Authorization", &auth).send_json(b).map_err(net)?,
            ("POST", Some(b)) => agent.post(&url).header("Authorization", &auth).send_json(b).map_err(net)?,
            _ => return Err("Bad Google request".into()),
        };
        finish(r).map_err(|e| format!("Google Calendar: {e}"))
    }

    pub fn calendars(&mut self, agent: &ureq::Agent) -> Res<Vec<GCal>> {
        let v = self.send(agent, "GET", "users/me/calendarList", &[("maxResults", "250".into()), ("minAccessRole", "reader".into())], None)?;
        Ok(v["items"].as_array().map(|a| a.iter().filter_map(parse_calendar).collect()).unwrap_or_default())
    }

    pub fn events(&mut self, agent: &ureq::Agent, cal: &GCal, from: NaiveDate, to: NaiveDate) -> Res<Vec<Event>> {
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut q = vec![
                ("timeMin", rfc3339(from.and_time(NaiveTime::MIN))),
                ("timeMax", rfc3339(to.and_time(NaiveTime::MIN))),
                ("singleEvents", "true".into()),
                ("orderBy", "startTime".into()),
                ("maxResults", "250".into()),
            ];
            if let Some(p) = &page {
                q.push(("pageToken", p.clone()));
            }
            let v = self.send(agent, "GET", &format!("calendars/{}/events", encode(&cal.id)), &q, None)?;
            if let Some(a) = v["items"].as_array() {
                out.extend(a.iter().filter_map(|e| parse_event(e, cal)));
            }
            match v["nextPageToken"].as_str() {
                Some(t) if !t.is_empty() => page = Some(t.to_string()),
                _ => return Ok(out),
            }
        }
    }

    pub fn create(&mut self, agent: &ureq::Agent, cal: &str, title: &str, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> Res<()> {
        let body = event_body(Some(title), start, end, all_day);
        self.send(agent, "POST", &format!("calendars/{}/events", encode(cal)), &[], Some(body)).map(|_| ())
    }

    pub fn update(&mut self, agent: &ureq::Agent, cal: &str, id: &str, title: Option<&str>, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> Res<()> {
        let body = event_body(title, start, end, all_day);
        self.send(agent, "PATCH", &format!("calendars/{}/events/{}", encode(cal), encode(id)), &[], Some(body)).map(|_| ())
    }

    pub fn delete(&mut self, agent: &ureq::Agent, cal: &str, id: &str) -> Res<()> {
        self.send(agent, "DELETE", &format!("calendars/{}/events/{}", encode(cal), encode(id)), &[], None).map(|_| ())
    }

    /// Best effort: tell Google to forget this sign-in.
    pub fn revoke(&self, agent: &ureq::Agent) {
        let _ = agent.post(REVOKE).send_form([("token", self.creds.refresh.as_str())]);
    }
}

// ---------- conversion ----------

fn hex_color(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

pub fn parse_calendar(v: &Value) -> Option<GCal> {
    if v["deleted"].as_bool().unwrap_or(false) {
        return None;
    }
    let role = v["accessRole"].as_str().unwrap_or("reader");
    Some(GCal {
        id: v["id"].as_str()?.to_string(),
        name: v["summaryOverride"].as_str().or(v["summary"].as_str()).unwrap_or("Calendar").to_string(),
        color: v["backgroundColor"].as_str().and_then(hex_color).unwrap_or([0x4a, 0x90, 0xe2]),
        writable: role == "owner" || role == "writer",
        primary: v["primary"].as_bool().unwrap_or(false),
    })
}

fn rfc3339(n: NaiveDateTime) -> String {
    match Local.from_local_datetime(&n).earliest() {
        Some(d) => d.to_rfc3339(),
        // A time skipped by a clock change: read it as UTC.
        None => format!("{}Z", n.format("%Y-%m-%dT%H:%M:%S")),
    }
}

fn parse_when(v: &Value) -> Option<(NaiveDateTime, bool)> {
    if let Some(d) = v["date"].as_str() {
        return Some((NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?.and_time(NaiveTime::MIN), true));
    }
    let dt = DateTime::parse_from_rfc3339(v["dateTime"].as_str()?).ok()?;
    Some((dt.with_timezone(&Local).naive_local(), false))
}

pub fn parse_event(v: &Value, cal: &GCal) -> Option<Event> {
    if v["status"].as_str() == Some("cancelled") {
        return None;
    }
    // Hide events the user declined.
    let declined = v["attendees"].as_array().is_some_and(|a| a.iter().any(|x| x["self"].as_bool() == Some(true) && x["responseStatus"].as_str() == Some("declined")));
    if declined {
        return None;
    }
    let (start, all_day) = parse_when(&v["start"])?;
    let (end, _) = parse_when(&v["end"]).unwrap_or((start, all_day));
    let mut e = Event::new(v["summary"].as_str().unwrap_or("(No title)").to_string(), start, end.max(start), all_day);
    e.id = v["id"].as_str()?.to_string();
    e.cal = cal.id.clone();
    e.color = Some(cal.color);
    e.editable = cal.writable && matches!(v["eventType"].as_str(), None | Some("default"));
    Some(e)
}

pub fn event_body(title: Option<&str>, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> Value {
    let (s, e) = if all_day {
        let end = end.date().max(start.date() + chrono::Duration::days(1));
        (json!({ "date": start.date().to_string() }), json!({ "date": end.to_string() }))
    } else {
        (json!({ "dateTime": rfc3339(start) }), json!({ "dateTime": rfc3339(end) }))
    };
    let mut b = json!({ "start": s, "end": e });
    if let Some(t) = title {
        b["summary"] = json!(t);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn cal(writable: bool) -> GCal {
        GCal { id: "c@x".into(), name: "C".into(), color: [1, 2, 3], writable, primary: false }
    }

    #[test]
    fn pkce_matches_rfc7636_example() {
        assert_eq!(challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let p = pkce();
        assert!(p.verifier.len() >= 43 && p.verifier.len() <= 128);
    }

    #[test]
    fn auth_url_has_required_parameters() {
        let u = auth_url("id.apps", "http://127.0.0.1:5000", "chal", "st");
        assert!(u.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(u.contains("client_id=id.apps"));
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5000"));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("access_type=offline"));
        assert!(u.contains("calendar.events%20https"));
    }

    #[test]
    fn callback_is_parsed() {
        let p = parse_callback("GET /?state=ab&code=4%2F0Aeo%3D-x&scope=a+b HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        assert_eq!(p["code"], "4/0Aeo=-x");
        assert_eq!(p["state"], "ab");
        assert_eq!(p["scope"], "a b");
        assert!(parse_callback("GET /favicon.ico HTTP/1.1\r\n\r\n").is_empty());
        assert_eq!(parse_callback("GET /?error=access_denied HTTP/1.1")["error"], "access_denied");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }

    #[test]
    fn loopback_receives_code() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").unwrap();
            let mut sink = String::new();
            let _ = s.read_to_string(&mut sink);
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(b"GET /?code=abc&state=xyz HTTP/1.1\r\n\r\n").unwrap();
            let mut ok = String::new();
            let _ = s.read_to_string(&mut ok);
            ok
        });
        assert_eq!(wait_for_code(&l, "xyz").unwrap(), "abc");
        assert!(t.join().unwrap().contains("Signed in"));
    }

    #[test]
    fn parses_timed_and_all_day_events() {
        let timed = json!({ "id": "e1", "summary": "Class", "start": { "dateTime": "2026-11-02T10:00:00Z" }, "end": { "dateTime": "2026-11-02T11:30:00Z" } });
        let e = parse_event(&timed, &cal(true)).unwrap();
        assert!(!e.all_day && e.editable);
        assert_eq!(e.end - e.start, chrono::Duration::minutes(90));
        assert_eq!(e.color, Some([1, 2, 3]));
        assert_eq!(e.key(), "c@x|e1");

        let all_day = json!({ "id": "e2", "summary": "Break", "start": { "date": "2026-11-03" }, "end": { "date": "2026-11-05" } });
        let e = parse_event(&all_day, &cal(false)).unwrap();
        assert!(e.all_day && !e.editable);
        assert_eq!(e.start, dt("2026-11-03 00:00"));
        assert_eq!(e.end, dt("2026-11-05 00:00"));
    }

    #[test]
    fn skips_cancelled_and_declined_events() {
        let c = json!({ "id": "a", "status": "cancelled", "start": { "date": "2026-11-03" }, "end": { "date": "2026-11-04" } });
        assert!(parse_event(&c, &cal(true)).is_none());
        let d = json!({ "id": "b", "start": { "date": "2026-11-03" }, "end": { "date": "2026-11-04" }, "attendees": [{ "self": true, "responseStatus": "declined" }] });
        assert!(parse_event(&d, &cal(true)).is_none());
    }

    #[test]
    fn birthdays_are_read_only() {
        let b = json!({ "id": "b", "eventType": "birthday", "summary": "Sam", "start": { "date": "2026-11-03" }, "end": { "date": "2026-11-04" } });
        assert!(!parse_event(&b, &cal(true)).unwrap().editable);
    }

    #[test]
    fn parses_calendars() {
        let v = json!({ "id": "me@x.com", "summary": "Me", "summaryOverride": "Mine", "backgroundColor": "#9fe1e7", "accessRole": "owner", "primary": true });
        let c = parse_calendar(&v).unwrap();
        assert_eq!((c.name.as_str(), c.color, c.writable, c.primary), ("Mine", [0x9f, 0xe1, 0xe7], true, true));
        let v = json!({ "id": "h", "summary": "Holidays", "accessRole": "reader" });
        assert!(!parse_calendar(&v).unwrap().writable);
        assert!(parse_calendar(&json!({ "id": "x", "deleted": true })).is_none());
    }

    #[test]
    fn event_body_shapes() {
        let b = event_body(Some("Study"), dt("2026-11-02 10:00"), dt("2026-11-02 11:00"), false);
        assert_eq!(b["summary"], "Study");
        assert!(b["start"]["dateTime"].as_str().unwrap().starts_with("2026-11-02T10:00:00"));
        let b = event_body(None, dt("2026-11-02 00:00"), dt("2026-11-02 00:00"), true);
        assert!(b.get("summary").is_none());
        assert_eq!(b["start"]["date"], "2026-11-02");
        assert_eq!(b["end"]["date"], "2026-11-03");
    }
}
