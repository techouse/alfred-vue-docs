use super::*;
use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig};

fn config() -> AlgoliaSearchConfig {
    AlgoliaSearchConfig {
        application_id: "app".to_owned(),
        api_key: "key".to_owned(),
        index_name: "vue".to_owned(),
    }
}

#[test]
fn endpoint_uses_single_index_search_route() -> Result<()> {
    let base_url = Url::parse("http://127.0.0.1:8080/api/")?;
    let client = AlgoliaSearch::with_base_url(config(), base_url.clone())?;

    assert_eq!(
        client.endpoint(&base_url)?.as_str(),
        "http://127.0.0.1:8080/api/1/indexes/vue/query"
    );
    Ok(())
}

#[test]
fn client_uses_dsn_and_numbered_read_hosts_in_order() -> Result<()> {
    let client = AlgoliaSearch::new(config())?;

    assert_eq!(
        client
            .read_hosts
            .iter()
            .map(Url::as_str)
            .collect::<Vec<_>>(),
        vec![
            "https://app-dsn.algolia.net/",
            "https://app-1.algolianet.com/",
            "https://app-2.algolianet.com/",
            "https://app-3.algolianet.com/",
        ]
    );
    Ok(())
}

#[test]
fn client_uses_platform_verifier_and_search_timeouts() -> Result<()> {
    let client = AlgoliaSearch::with_base_url(config(), Url::parse("http://localhost/")?)?;
    let timeouts = client.agent.config().timeouts();

    assert!(matches!(
        client.agent.config().tls_config().root_certs(),
        RootCerts::PlatformVerifier
    ));
    assert_eq!(timeouts.connect, Some(CONNECT_TIMEOUT));
    assert_eq!(timeouts.global, Some(SEARCH_TIMEOUT));
    Ok(())
}

#[test]
fn request_body_preserves_vue_search_contract() -> Result<()> {
    let client = AlgoliaSearch::with_base_url(config(), Url::parse("http://localhost/")?)?;
    let body: Value = serde_json::from_str(&client.request_body("composition", "v3")?)?;

    assert_eq!(
        body,
        json!({
            "query": "composition",
            "facetFilters": ["version:v3"],
            "attributesToRetrieve": [
                "hierarchy.lvl0", "hierarchy.lvl1", "hierarchy.lvl2",
                "hierarchy.lvl3", "hierarchy.lvl4", "hierarchy.lvl5",
                "hierarchy.lvl6", "content", "type", "url"
            ],
            "attributesToSnippet": [
                "hierarchy.lvl1:10", "hierarchy.lvl2:10", "hierarchy.lvl3:10",
                "hierarchy.lvl4:10", "hierarchy.lvl5:10", "hierarchy.lvl6:10",
                "content:10"
            ],
            "snippetEllipsisText": "...",
            "page": 0,
            "hitsPerPage": 20
        })
    );
    Ok(())
}

#[test]
fn query_retries_hosts_in_order_with_one_deadline() -> Result<()> {
    let client = client_with_hosts(&[
        "http://first.test/",
        "http://second.test/",
        "http://third.test/",
    ])?;
    let mut endpoints = Vec::new();
    let mut remaining_times = Vec::new();

    let hits = client.query_with("composition", "v3", |endpoint, _, remaining| {
        endpoints.push(endpoint.as_str().to_owned());
        remaining_times.push(remaining);
        if endpoints.len() < 3 {
            Err(AttemptFailure::Retryable(anyhow::anyhow!(
                "temporary failure"
            )))
        } else {
            Ok(Vec::new())
        }
    })?;

    assert!(hits.is_empty());
    assert_eq!(
        endpoints,
        vec![
            "http://first.test/1/indexes/vue/query",
            "http://second.test/1/indexes/vue/query",
            "http://third.test/1/indexes/vue/query",
        ]
    );
    assert!(
        remaining_times
            .windows(2)
            .all(|window| window[0] > window[1])
    );
    Ok(())
}

#[test]
fn query_stops_after_terminal_failure() -> Result<()> {
    let client = client_with_hosts(&["http://first.test/", "http://second.test/"])?;
    let mut attempts = 0;

    let error = client
        .query_with("composition", "v3", |_, _, _| {
            attempts += 1;
            Err(AttemptFailure::Terminal(anyhow::anyhow!("invalid request")))
        })
        .expect_err("terminal failures must stop failover");

    assert_eq!(attempts, 1);
    assert_eq!(error.to_string(), "invalid request");
    Ok(())
}

#[test]
fn query_reports_all_retryable_host_failures() -> Result<()> {
    let client = client_with_hosts(&["http://first.test/", "http://second.test/"])?;
    let error = client
        .query_with("composition", "v3", |endpoint, _, _| {
            Err(AttemptFailure::Retryable(anyhow::anyhow!(
                "{endpoint} failed"
            )))
        })
        .expect_err("all retryable failures must be reported");
    let message = error.to_string();

    assert!(message.contains("after trying 2 read hosts"));
    assert!(message.contains("first.test/1/indexes/vue/query failed"));
    assert!(message.contains("second.test/1/indexes/vue/query failed"));
    Ok(())
}

#[test]
fn transport_failure_classification_matches_retry_policy() -> Result<()> {
    let endpoint = Url::parse("http://first.test/")?;

    assert!(matches!(
        classify_ureq_error(&endpoint, "send", ureq::Error::HostNotFound),
        AttemptFailure::Retryable(_)
    ));
    assert!(matches!(
        classify_ureq_error(&endpoint, "send", ureq::Error::ConnectionFailed),
        AttemptFailure::Retryable(_)
    ));
    assert!(matches!(
        classify_ureq_error(
            &endpoint,
            "send",
            ureq::Error::Timeout(ureq::Timeout::Global)
        ),
        AttemptFailure::Retryable(_)
    ));
    Ok(())
}

#[test]
fn search_response_deserializes_hierarchy_and_content() -> Result<()> {
    let response: SearchResponse = serde_json::from_value(json!({
        "hits": [{
            "objectID": "component",
            "type": "lvl2",
            "url": "https://vuejs.org/guide/component",
            "hierarchy": {
                "lvl0": "Guide",
                "lvl1": "Components",
                "lvl2": "Component Basics",
                "lvl3": null,
                "lvl4": null,
                "lvl5": null,
                "lvl6": null
            },
            "content": "content"
        }]
    }))?;

    assert_eq!(
        (
            response.hits[0].object_id.as_str(),
            response.hits[0].hierarchy.last(),
            response.hits[0].content.as_deref()
        ),
        ("component", "Component Basics", Some("content"))
    );
    Ok(())
}

#[test]
fn empty_configuration_values_are_rejected_before_client_creation() {
    for (field, expected) in [
        ("application_id", "ALGOLIA_APPLICATION_ID must not be empty"),
        ("api_key", "ALGOLIA_SEARCH_ONLY_API_KEY must not be empty"),
        ("index_name", "ALGOLIA_SEARCH_INDEX must not be empty"),
    ] {
        let mut invalid = config();
        match field {
            "application_id" => invalid.application_id.clear(),
            "api_key" => invalid.api_key.clear(),
            "index_name" => invalid.index_name.clear(),
            _ => unreachable!("test field is fixed"),
        }

        let error = AlgoliaSearch::new(invalid).expect_err("empty values must fail");
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn query_falls_back_after_5xx_response() -> Result<()> {
    let (first_url, first_server) = serve_once(503, r#"{"message":"temporary"}"#)?;
    let (second_url, second_server) = serve_once(200, r#"{"hits":[]}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(
        config(),
        vec![first_url, second_url],
        no_proxy_agent(),
    )?;

    let hits = client.query("composition", "v3")?;

    join_server(first_server)?;
    join_server(second_server)?;
    assert!(hits.is_empty());
    Ok(())
}

#[test]
fn malformed_4xx_body_is_terminal_without_failover() -> Result<()> {
    let (url, server) = serve_once_with_body(401, b"not-gzip".to_vec(), Some("gzip"))?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(config(), vec![url], no_proxy_agent())?;

    let error = client
        .query("composition", "v3")
        .expect_err("malformed 4xx body must be terminal");
    join_server(server)?;

    let message = error.to_string();
    assert!(message.contains("HTTP status 401"));
    assert!(!message.contains("after trying"));
    Ok(())
}

#[test]
fn malformed_5xx_body_retries_next_host() -> Result<()> {
    let (first_url, first_server) = serve_once_with_body(503, b"not-gzip".to_vec(), Some("gzip"))?;
    let (second_url, second_server) = serve_once(200, r#"{"hits":[]}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(
        config(),
        vec![first_url, second_url],
        no_proxy_agent(),
    )?;

    let hits = client.query("composition", "v3")?;

    join_server(first_server)?;
    join_server(second_server)?;
    assert!(hits.is_empty());
    Ok(())
}

#[test]
fn invalid_utf8_5xx_body_retries_next_host() -> Result<()> {
    let (first_url, first_server) = serve_once_with_body(503, vec![0xff], None)?;
    let (second_url, second_server) = serve_once(200, r#"{"hits":[]}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(
        config(),
        vec![first_url, second_url],
        no_proxy_agent(),
    )?;

    let hits = client.query("composition", "v3")?;

    join_server(first_server)?;
    join_server(second_server)?;
    assert!(hits.is_empty());
    Ok(())
}

#[test]
fn four_xx_diagnostics_include_status_and_body() -> Result<()> {
    let (url, server) = serve_once(401, r#"{"message":"invalid key"}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(config(), vec![url], no_proxy_agent())?;

    let error = client
        .query("composition", "v3")
        .expect_err("4xx responses must fail");
    join_server(server)?;
    let message = error.to_string();

    assert!(message.contains("HTTP status 401"));
    assert!(message.contains(r#"{"message":"invalid key"}"#));
    Ok(())
}

#[test]
fn malformed_successful_json_is_terminal() -> Result<()> {
    let (url, server) = serve_once(200, "not json")?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(config(), vec![url], no_proxy_agent())?;

    let error = client
        .query("composition", "v3")
        .expect_err("malformed successful JSON must fail");
    join_server(server)?;
    assert!(error.to_string().contains("failed to deserialize"));
    Ok(())
}

#[test]
fn oversized_successful_response_is_terminal() -> Result<()> {
    let body = "x".repeat((MAX_RESPONSE_BYTES + 1) as usize);
    let (url, server) = serve_gzip_once(200, &body)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(config(), vec![url], no_proxy_agent())?;

    let error = client
        .query("composition", "v3")
        .expect_err("oversized successful responses must fail");
    join_server(server)?;
    assert!(error.to_string().contains("exceeds"));
    Ok(())
}

#[test]
fn oversized_5xx_response_retries_next_host() -> Result<()> {
    let body = "x".repeat((MAX_RESPONSE_BYTES + 1) as usize);
    let (first_url, first_server) = serve_gzip_once(503, &body)?;
    let (second_url, second_server) = serve_once(200, r#"{"hits":[]}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(
        config(),
        vec![first_url, second_url],
        no_proxy_agent(),
    )?;

    let hits = client.query("composition", "v3")?;

    join_server(first_server)?;
    join_server(second_server)?;
    assert!(hits.is_empty());
    Ok(())
}

#[test]
fn exact_decoded_response_limit_is_accepted() -> Result<()> {
    let prefix = "{\"hits\":[],\"pad\":\"";
    let suffix = "\"}";
    let pad_length = MAX_RESPONSE_BYTES as usize - prefix.len() - suffix.len();
    let body = format!("{prefix}{}{suffix}", "x".repeat(pad_length));
    assert_eq!(body.len() as u64, MAX_RESPONSE_BYTES);
    let (url, server) = serve_once(200, &body)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(config(), vec![url], no_proxy_agent())?;

    let hits = client.query("composition", "v3")?;

    join_server(server)?;
    assert!(hits.is_empty());
    Ok(())
}

#[test]
fn malformed_http_retries_next_host() -> Result<()> {
    let (first_url, first_server) = serve_raw_once(b"BOGUS/1.1 200\r\n\r\n")?;
    let (second_url, second_server) = serve_once(200, r#"{"hits":[]}"#)?;
    let client = AlgoliaSearch::with_read_hosts_and_agent(
        config(),
        vec![first_url, second_url],
        no_proxy_agent(),
    )?;

    let hits = client.query("composition", "v3")?;

    join_server(first_server)?;
    join_server(second_server)?;
    assert!(hits.is_empty());
    Ok(())
}

fn client_with_hosts(hosts: &[&str]) -> Result<AlgoliaSearch> {
    let read_hosts = hosts
        .iter()
        .map(|host| Url::parse(host))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    AlgoliaSearch::with_read_hosts_and_agent(config(), read_hosts, no_proxy_agent())
}

fn no_proxy_agent() -> Agent {
    Agent::config_builder()
        .proxy(None)
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(SEARCH_TIMEOUT))
        .build()
        .into()
}

fn join_server(server: JoinHandle<()>) -> Result<()> {
    server
        .join()
        .map_err(|_| anyhow::anyhow!("test server thread panicked"))
}

fn serve_once(status: u16, body: &str) -> Result<(Url, JoinHandle<()>)> {
    serve_once_with_body(status, body.as_bytes().to_vec(), None)
}

fn serve_gzip_once(status: u16, body: &str) -> Result<(Url, JoinHandle<()>)> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body.as_bytes())?;
    serve_once_with_body(status, encoder.finish()?, Some("gzip"))
}

fn serve_raw_once(raw: &'static [u8]) -> Result<(Url, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let server = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("test server must accept a request");
        read_request(&mut stream);
        stream
            .write_all(raw)
            .expect("test server must write the response");
        stream.flush().expect("test server must flush the response");
    });

    Ok((Url::parse(&format!("http://{address}/"))?, server))
}

fn read_request(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let bytes = stream
            .read(&mut buffer)
            .expect("test server must read the request");
        if bytes == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..bytes]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("Content-Length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
}

fn serve_once_with_body(
    status: u16,
    body: Vec<u8>,
    content_encoding: Option<&str>,
) -> Result<(Url, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let content_encoding = content_encoding.map(str::to_owned).unwrap_or_default();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("test server must accept a request");
        read_request(&mut stream);
        let encoding_header = if content_encoding.is_empty() {
            String::new()
        } else {
            format!("Content-Encoding: {content_encoding}\r\n")
        };
        write!(
            stream,
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{encoding_header}Connection: close\r\n\r\n",
            body.len()
        )
        .expect("test server must write response headers");
        stream
            .write_all(&body)
            .expect("test server must write response body");
        stream.flush().expect("test server must flush the response");
    });

    Ok((Url::parse(&format!("http://{address}/"))?, server))
}
