use std::env;

fn main() {
    let base = env::var("VEIL_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:7788".to_string());
    let token = env::var("VEIL_NODE_TOKEN").unwrap_or_default();

    let health = get_json(&format!("{base}/health"), &token);
    println!("health: {health}");

    let status = get_json(&format!("{base}/status"), &token);
    println!("status: {status}");
}

fn get_json(url: &str, token: &str) -> String {
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url);
    if !token.is_empty() {
        request = request.header("x-veil-token", token);
    }
    match request.send() {
        Ok(response) => response
            .text()
            .unwrap_or_else(|_| "{\"error\":\"read_failed\"}".to_string()),
        Err(err) => format!("{{\"error\":\"{err}\"}}"),
    }
}
