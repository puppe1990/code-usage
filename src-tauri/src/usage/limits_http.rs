//! Blocking HTTP shared by the plan-limit fetchers: one client config and one bearer GET whose
//! errors carry the URL and status.

use super::CollectError;
use std::time::Duration;

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Blocking client with the project timeout and user agent.
pub fn client() -> Result<reqwest::blocking::Client, CollectError> {
    reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("code-usage/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| CollectError::Failed(format!("http client: {error}")))
}

/// Bearer-authenticated GET returning the body, or an error carrying the URL and status.
pub fn get_bearer_json(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<String, CollectError> {
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|error| CollectError::Failed(format!("{url}: {error}")))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|error| CollectError::Failed(format!("{url}: {error}")))?;

    if !status.is_success() {
        return Err(CollectError::Failed(format!("{url}: HTTP {status}")));
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::test_server;

    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .expect("test client")
    }

    #[test]
    fn returns_the_body_of_a_successful_get() {
        let base = test_server::spawn(vec![(200, "{\"ok\":true}".to_string())]);

        let body = get_bearer_json(&test_client(), &base, "secret").expect("body");

        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn reports_the_status_of_a_failed_get() {
        let base = test_server::spawn(vec![(500, "{\"error\":true}".to_string())]);

        let error = get_bearer_json(&test_client(), &base, "secret").expect_err("http error");

        assert!(matches!(error, CollectError::Failed(message) if message.contains("HTTP 500")));
    }
}
