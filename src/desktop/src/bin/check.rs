use fastmd::config::AppConfig;

fn main() {
    let yaml1 = r#"
tool_groups:
  trello: true
trello_client:
  token: ATTA123
  apiKey: 5d8a
"#;
    let config1: AppConfig = serde_norway::from_str(yaml1).unwrap_or_default();
    println!("Parsed config1 (apiKey): {:?}", config1.trello_client);

    let yaml2 = r#"
tool_groups:
  trello: true
trello_client:
  token: ATTA123
  api_key: 5d8a
"#;
    let config2: AppConfig = serde_norway::from_str(yaml2).unwrap_or_default();
    println!("Parsed config2 (api_key): {:?}", config2.trello_client);
}
