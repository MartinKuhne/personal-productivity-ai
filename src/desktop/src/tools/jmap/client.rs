use crate::config::JmapClient;
use serde_json::Value;

pub fn get_jmap_session(client: &JmapClient) -> Result<(String, String, Value), String> {
    let url = &client.url;
    let token = &client.token;
    
    let session_url = if url.ends_with("/api") {
        format!("{}/session", url.strip_suffix("/api").unwrap_or(url))
    } else {
        url.to_string()
    };

    let resp = match ureq::get(&session_url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
    {
        Ok(r) => {
            println!("JMAP Status Code: {}", r.status());
            r
        }
        Err(e) => {
            if let ureq::Error::Status(code, response) = e {
                let body = response.into_string().unwrap_or_default();
                return Err(format!("JMAP Error {}: {}", code, body));
            }
            return Err(e.to_string());
        }
    };
    
    let json: Value = resp.into_json().map_err(|e| e.to_string())?;
    let api_url = json["apiUrl"].as_str().unwrap_or(url).to_string();
    let primary_accounts = json["primaryAccounts"].clone();
        
    Ok((api_url, token.to_string(), primary_accounts))
}

pub fn jmap_call(api_url: &str, token: &str, capabilities: &[&str], method_calls: Value) -> Result<Value, String> {
    let mut using = vec!["urn:ietf:params:jmap:core"];
    using.extend(capabilities);
    
    let payload = serde_json::json!({
        "using": using,
        "methodCalls": method_calls
    });
    
    let resp = match ureq::post(api_url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Content-Type", "application/json")
        .send_json(payload)
    {
        Ok(r) => {
            println!("JMAP Status Code: {}", r.status());
            r
        }
        Err(e) => {
            if let ureq::Error::Status(code, response) = e {
                let body = response.into_string().unwrap_or_default();
                return Err(format!("JMAP Error {}: {}", code, body));
            }
            return Err(e.to_string());
        }
    };
        
    resp.into_json().map_err(|e| e.to_string())
}

pub fn get_account_id(primary_accounts: &Value, cap: &str) -> String {
    primary_accounts[cap].as_str()
        .or_else(|| primary_accounts["urn:ietf:params:jmap:core"].as_str())
        .unwrap_or("")
        .to_string()
}
