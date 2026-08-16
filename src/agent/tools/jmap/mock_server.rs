//! Mock JMAP server for unit tests.
//!
//! Spawns a lightweight TCP server that mimics a JMAP server's session
//! resource and method response endpoints, used by email tool tests.
//!
//! The session resource and capabilities are modelled after RFC 8620
//! (JMAP Core) and RFC 8621 (JMAP for Mail):
//! - Session resource: RFC 8620 §2
//! - Core capability (`urn:ietf:params:jmap:core`): RFC 8620 §2
//! - Mail capability (`urn:ietf:params:jmap:mail`): RFC 8621 §1.3.1
//! - Submission capability (`urn:ietf:params:jmap:submission`): RFC 8621 §1.3.2
//! - Vacation response capability (`urn:ietf:params:jmap:vacationresponse`): RFC 8621 §1.3.3

use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

/// JMAP capability URI for the core protocol (RFC 8620).
const CAP_CORE: &str = "urn:ietf:params:jmap:core";

/// JMAP capability URI for mail (RFC 8621 §1.3.1).
const CAP_MAIL: &str = "urn:ietf:params:jmap:mail";

/// JMAP capability URI for submission (RFC 8621 §1.3.2).
const CAP_SUBMISSION: &str = "urn:ietf:params:jmap:submission";

/// JMAP capability URI for vacation response (RFC 8621 §1.3.3).
const CAP_VACATION: &str = "urn:ietf:params:jmap:vacationresponse";

/// Return the session-level capabilities object (RFC 8620 §2 / RFC 8621 §1.3).
///
/// Per the RFCs, the session-level capability objects for `mail`, `submission`,
/// and `vacationresponse` are **empty objects** — they only advertise that the
/// capability exists.  The detailed constraints live in each account's
/// `accountCapabilities` (see [`account_capabilities`]).
fn session_capabilities() -> serde_json::Value {
    json!({
        CAP_CORE: {
            "maxSizeUpload": 100_000_000,
            "maxConcurrentUpload": 5,
            "maxSizeRequest": 100_000_000,
            "maxConcurrentRequests": 10,
            "maxCallsInRequest": 16,
            "maxObjectsInGet": 1000,
            "maxObjectsInSet": 100,
            "collationAlgorithms": ["i;ascii-numeric", "i;ascii-casemap", "i;unicode-casemap"]
        },
        CAP_MAIL: {},
        CAP_SUBMISSION: {},
        CAP_VACATION: {}
    })
}

/// Return the account-level capabilities object for a mail account
/// (RFC 8621 §1.3.1–1.3.3).
///
/// These are the constraint objects that appear inside each account's
/// `accountCapabilities` map and describe per-account limits.
fn account_capabilities() -> serde_json::Value {
    json!({
        CAP_CORE: {},
        CAP_MAIL: {
            "maxMailboxesPerEmail": 100,
            "maxMailboxDepth": 10,
            "maxSizeMailboxName": 256,
            "maxSizeAttachmentsPerEmail": 100_000_000,
            "emailQuerySortOptions": ["receivedAt", "sentAt", "from", "to", "subject"],
            "mayCreateTopLevelMailbox": true
        },
        CAP_SUBMISSION: {
            "maxDelayedSend": 0,
            "submissionExtensions": {}
        },
        CAP_VACATION: {}
    })
}

/// Spawn a mock JMAP server that returns session resources and method responses.
///
/// The server handles two endpoints per RFC 8620 §2:
/// - `GET /.well-known/jmap` (or any GET) — returns a JMAP Session resource
/// - `POST` to `apiUrl` — returns a JMAP Response object with `methodResponses`
///
/// The `body` parameter is a JSON object containing:
/// - Session fields (`apiUrl`, `primaryAccounts`, etc.) — `{API_URL}` will be
///   replaced with the actual mock server URL.
/// - `methodResponses` — an array of JMAP method response tuples
///   `["MethodName", {args}, "callId"]`.
///
/// Returns the base URL of the mock server.
pub fn spawn_mock_server(body: impl Into<String>) -> String {
    spawn_mock_server_inner(body, None)
}

/// Spawn a mock JMAP server that records the raw bytes of every POST body
/// it receives into a shared recorder.
///
/// Returns `(url, recorder)`. Each POST to the server appends its body bytes
/// to `recorder`; tests can `lock()` and inspect the bodies in arrival order
/// to assert on the outgoing JMAP method-call arguments.
///
/// Use this when the test needs to verify *what the client sent* (e.g. that
/// `Email/get` requests include `maxBodyValueBytes`); [`spawn_mock_server`]
/// is enough when the test only cares about return-value behaviour.
pub fn spawn_recording_mock_server(body: impl Into<String>) -> (String, Arc<Mutex<Vec<Vec<u8>>>>) {
    let recorder: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let url = spawn_mock_server_inner(body, Some(recorder.clone()));
    (url, recorder)
}

fn spawn_mock_server_inner(
    body: impl Into<String>,
    recorder: Option<Arc<Mutex<Vec<Vec<u8>>>>>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let api_url = format!("http://127.0.0.1:{port}");
    let body_str = body.into().replace("{API_URL}", &api_url);
    let server_body: serde_json::Value = serde_json::from_str(&body_str).unwrap_or_else(|e| {
        panic!(
            "body must be valid JSON: {}\nBody: {:?}",
            e,
            &body_str[..body_str.len().min(500)]
        )
    });

    let session_resource = build_session_resource(&server_body, &api_url);
    let session_str = serde_json::to_string(&session_resource).unwrap();

    let method_responses = server_body
        .get("methodResponses")
        .and_then(|mr| mr.as_array())
        .cloned()
        .unwrap_or_default();

    thread::spawn(move || {
        let mut request_count = 0u32;
        for mut stream in listener.incoming().flatten() {
            request_count += 1;
            let mut buf = [0; 65536];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            eprintln!(
                "[mock_server] request #{request_count}: {}...",
                &request[..request.len().min(200)]
            );

            if request.starts_with("GET") {
                let response_str = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json; charset=utf-8\r\n\
                     Cache-Control: no-cache, private\r\n\
                     Connection: close\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    session_str.len(),
                    session_str
                );
                let _ = stream.write_all(response_str.as_bytes());
            } else if request.starts_with("POST") {
                let body_text = request.split("\r\n\r\n").nth(1).unwrap_or("");
                eprintln!(
                    "[mock_server] POST body: {}",
                    &body_text[..body_text.len().min(500)]
                );
                if let Some(rec) = recorder.as_ref() {
                    rec.lock()
                        .expect("mock recorder poisoned")
                        .push(body_text.as_bytes().to_vec());
                }
                let response = handle_jmap_post(body_text, &method_responses);

                let resp_str = serde_json::to_string(&response).unwrap();
                if resp_str.contains("Email/get") {
                    eprintln!("[mock_server] FULL Email/get response: {}", resp_str);
                }
                eprintln!(
                    "[mock_server] response: {}",
                    &resp_str[..resp_str.len().min(500)]
                );
                let response_str = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json; charset=utf-8\r\n\
                     Cache-Control: no-cache, private\r\n\
                     Connection: close\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    resp_str.len(),
                    resp_str
                );
                let _ = stream.write_all(response_str.as_bytes());
            } else {
                let response_str = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json; charset=utf-8\r\n\
                     Connection: close\r\n\
                     Content-Length: {}\r\n\
                     \r\n\
                     {}",
                    session_str.len(),
                    session_str
                );
                let _ = stream.write_all(response_str.as_bytes());
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    format!("http://127.0.0.1:{}", port)
}

/// Build a complete JMAP Session resource (RFC 8620 §2).
///
/// Copies caller-supplied fields from `server_body` (everything except
/// `methodResponses`) and fills in RFC-required defaults.
fn build_session_resource(server_body: &serde_json::Value, api_url: &str) -> serde_json::Value {
    let mut session_obj = serde_json::Map::new();
    let body_obj = server_body.as_object().unwrap();

    for (k, v) in body_obj {
        if k != "methodResponses" {
            session_obj.insert(k.clone(), v.clone());
        }
    }

    session_obj
        .entry("capabilities".to_string())
        .or_insert(session_capabilities());

    if (!session_obj.contains_key("accounts")
        || session_obj["accounts"]
            .as_object()
            .is_none_or(|o| o.is_empty()))
        && let Some(primary) = session_obj
            .get("primaryAccounts")
            .and_then(|p| p.as_object())
    {
        let mut accounts = serde_json::Map::new();
        for (_cap, account_id) in primary {
            if let Some(id_str) = account_id.as_str() {
                accounts.entry(id_str.to_string()).or_insert(json!({
                    "name": "Test Account",
                    "isPersonal": true,
                    "isReadOnly": false,
                    "accountCapabilities": account_capabilities()
                }));
            }
        }
        session_obj.insert("accounts".to_string(), serde_json::Value::Object(accounts));
    }

    session_obj
        .entry("username".to_string())
        .or_insert(json!("test"));
    session_obj
        .entry("apiUrl".to_string())
        .or_insert(json!(api_url));
    session_obj
        .entry("downloadUrl".to_string())
        .or_insert(json!(format!(
            "{api_url}/download/{{accountId}}/{{blobId}}/{{name}}"
        )));
    session_obj
        .entry("uploadUrl".to_string())
        .or_insert(json!(format!("{api_url}/upload/{{accountId}}")));
    session_obj
        .entry("eventSourceUrl".to_string())
        .or_insert(json!(format!("{api_url}/eventsource")));
    session_obj
        .entry("state".to_string())
        .or_insert(json!("mock-state-0"));

    serde_json::Value::Object(session_obj)
}

/// Handle a JMAP POST request and build the response object (RFC 8620 §3.4).
///
/// Matches incoming `methodCalls` against the pre-configured `method_responses`
/// by call ID first (preferred per RFC 8620 §3.3), then by method name as
/// fallback.  Each matched response is enriched with RFC-required fields.
fn handle_jmap_post(body_text: &str, method_responses: &[serde_json::Value]) -> serde_json::Value {
    let body_json = serde_json::from_str::<serde_json::Value>(body_text).ok();

    if let Some(body) = body_json {
        if let Some(calls) = body.get("methodCalls").and_then(|c| c.as_array()) {
            let mut responses: Vec<serde_json::Value> = Vec::new();
            for call in calls {
                if let Some(call_arr) = call.as_array() {
                    let method_name = call_arr.first().and_then(|m| m.as_str()).unwrap_or("");
                    let call_id = call_arr.get(2).and_then(|id| id.as_str()).unwrap_or("0");
                    let call_args = call_arr.get(1);

                    let matched =
                        find_method_response(method_name, call_id, call_args, method_responses);

                    if let Some(resp) = matched {
                        let mut resp_arr = resp.as_array().cloned().unwrap_or_default();
                        if resp_arr.len() >= 3 {
                            resp_arr[2] = json!(call_id);
                        }
                        filter_response_by_ids(&mut resp_arr, method_name, call_args);
                        enrich_method_response(&mut resp_arr, method_name);
                        responses.push(json!(resp_arr));
                    } else {
                        responses.push(json!([
                            "error",
                            {
                                "type": "unknownMethod",
                                "description": format!("Method {method_name} is not supported by this mock server")
                            },
                            call_id
                        ]));
                    }
                } else {
                    responses.push(json!([
                        "error",
                        {"type": "invalidResultReference"},
                        "0"
                    ]));
                }
            }
            json!({ "methodResponses": responses, "sessionState": "mock-state-0" })
        } else {
            json!({ "methodResponses": method_responses, "sessionState": "mock-state-0" })
        }
    } else {
        json!({ "methodResponses": method_responses, "sessionState": "mock-state-0" })
    }
}

/// Filter a `/get` response's `list` to only include items whose `id` matches
/// the `ids` array in the request arguments.
///
/// Real JMAP servers only return the requested objects.  The mock stores a
/// single pre-configured response per method name, so when the same response
/// is reused for different calls we must filter it down to the requested IDs
/// to avoid returning objects the caller did not ask for.
fn filter_response_by_ids(
    resp_arr: &mut [serde_json::Value],
    method_name: &str,
    call_args: Option<&serde_json::Value>,
) {
    if method_name != "Email/get" && method_name != "Mailbox/get" {
        return;
    }
    let Some(args) = call_args.and_then(|a| a.as_object()) else {
        return;
    };
    let Some(requested_ids) = args.get("ids").and_then(|v| v.as_array()) else {
        return;
    };
    let id_set: Vec<&str> = requested_ids.iter().filter_map(|v| v.as_str()).collect();
    let Some(resp_obj) = resp_arr.get_mut(1).and_then(|v| v.as_object_mut()) else {
        return;
    };
    if let Some(list) = resp_obj.get_mut("list").and_then(|v| v.as_array_mut()) {
        list.retain(|item| {
            item.get("id")
                .and_then(|id| id.as_str())
                .is_some_and(|id| id_set.contains(&id))
        });
    }
}

/// Find the best matching method response for a given method call.
///
/// Matching order (per RFC 8620 §3.3):
/// 1. Exact match on method name **and** call ID (and matching filter if configured in response)
/// 2. Match on method name only (first match wins — for backwards compat)
fn find_method_response<'a>(
    method_name: &str,
    call_id: &str,
    call_args: Option<&serde_json::Value>,
    method_responses: &'a [serde_json::Value],
) -> Option<&'a serde_json::Value> {
    let matches_filter = |r: &serde_json::Value| -> bool {
        let Some(resp_obj) = r.get(1).and_then(|v| v.as_object()) else {
            return true;
        };
        if let Some(expected_filter) = resp_obj.get("filter") {
            let actual_filter = call_args.and_then(|a| a.get("filter"));
            actual_filter == Some(expected_filter)
        } else {
            true
        }
    };

    method_responses
        .iter()
        .find(|r| {
            let Some(r_arr) = r.as_array() else {
                return false;
            };
            let r_method = r_arr.first().and_then(|m| m.as_str()).unwrap_or("");
            let r_call_id = r_arr.get(2).and_then(|id| id.as_str()).unwrap_or("");
            r_method.eq_ignore_ascii_case(method_name) && r_call_id == call_id && matches_filter(r)
        })
        .or_else(|| {
            method_responses.iter().find(|r| {
                let r_method = r.get(0).and_then(|m| m.as_str()).unwrap_or("");
                r_method.eq_ignore_ascii_case(method_name) && matches_filter(r)
            })
        })
}

/// Enrich a method response tuple with RFC-required fields.
fn enrich_method_response(resp_arr: &mut [serde_json::Value], method_name: &str) {
    let Some(resp_obj) = resp_arr.get_mut(1).and_then(|v| v.as_object_mut()) else {
        return;
    };
    resp_obj.entry("accountId").or_insert(json!("acc1"));

    match method_name {
        "Email/query" | "Mailbox/query" => {
            resp_obj.entry("queryState").or_insert(json!("q-state-0"));
            resp_obj
                .entry("canCalculateChanges")
                .or_insert(json!(false));
            resp_obj.entry("position").or_insert(json!(0));
            let total = resp_obj
                .get("ids")
                .and_then(|ids| ids.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            resp_obj.entry("total").or_insert(json!(total));
        }
        "Email/get" | "Mailbox/get" => {
            resp_obj.entry("state").or_insert(json!("msg-state-0"));
            resp_obj.entry("notFound").or_insert(json!([]));
            if let Some(list) = resp_obj.get_mut("list").and_then(|v| v.as_array_mut()) {
                for email_val in list.iter_mut() {
                    if let Some(email_obj) = email_val.as_object_mut() {
                        let id = email_obj
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        email_obj
                            .entry("blobId")
                            .or_insert(json!(format!("blob_{id}")));
                        email_obj
                            .entry("threadId")
                            .or_insert(json!(format!("thread_{id}")));
                        email_obj
                            .entry("mailboxIds")
                            .or_insert(json!({"inbox-id": true}));
                        email_obj.entry("size").or_insert(json!(1024));
                        email_obj.entry("keywords").or_insert(json!({}));
                    }
                }
            }
        }
        "Email/set" | "Mailbox/set" | "EmailSubmission/set" => {
            resp_obj.entry("accountId").or_insert(json!("acc1"));
            resp_obj.entry("oldState").or_insert(json!("state-0"));
            resp_obj.entry("newState").or_insert(json!("state-1"));
        }
        "Email/import" => {
            resp_obj.entry("accountId").or_insert(json!("acc1"));
            resp_obj.entry("oldState").or_insert(json!("state-0"));
            resp_obj.entry("newState").or_insert(json!("state-1"));
        }
        _ => {}
    }
}
