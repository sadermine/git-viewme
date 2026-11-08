use git_viewme_core::{CoreError, error_for_graphql, error_for_status};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;

const NOW: u64 = 1_800_000_000;

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    pairs
        .iter()
        .map(|(name, value)| {
            (
                HeaderName::from_static(name),
                HeaderValue::from_static(value),
            )
        })
        .collect()
}

#[test]
fn error_for_status_401_is_unauthorized() {
    let error = error_for_status(401, &headers(&[]), NOW, r#"{"message":"Bad credentials"}"#);

    assert_eq!(error, CoreError::Unauthorized);
}

#[test]
fn error_for_status_403_with_no_requests_left_is_rate_limited_until_reset() {
    let headers = headers(&[
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "1800000900"),
    ]);

    let error = error_for_status(403, &headers, NOW, "{}");

    assert_eq!(
        error,
        CoreError::RateLimited {
            reset_at: 1_800_000_900
        }
    );
}

#[test]
fn error_for_status_429_with_retry_after_is_rate_limited_from_now() {
    let headers = headers(&[("x-ratelimit-remaining", "12"), ("retry-after", "30")]);

    let error = error_for_status(429, &headers, NOW, "{}");

    assert_eq!(error, CoreError::RateLimited { reset_at: NOW + 30 });
}

#[test]
fn error_for_status_403_with_requests_left_is_other_with_github_message() {
    let headers = headers(&[("x-ratelimit-remaining", "4000")]);

    let error = error_for_status(
        403,
        &headers,
        NOW,
        r#"{"message":"Resource not accessible"}"#,
    );

    assert_eq!(
        error,
        CoreError::Other {
            message: "GitHub answered 403: Resource not accessible".into()
        }
    );
}

#[test]
fn error_for_status_404_is_not_found() {
    assert_eq!(
        error_for_status(404, &headers(&[]), NOW, ""),
        CoreError::NotFound
    );
}

#[test]
fn core_error_serializes_as_tagged_union() {
    assert_eq!(
        serde_json::to_value(CoreError::RateLimited { reset_at: 5 }).unwrap(),
        json!({ "kind": "rate_limited", "reset_at": 5 })
    );
    assert_eq!(
        serde_json::to_value(CoreError::Network).unwrap(),
        json!({ "kind": "network" })
    );
}

#[test]
fn error_for_graphql_without_errors_is_ok() {
    assert_eq!(error_for_graphql(r#"{"data":{}}"#, None, NOW), Ok(()));
}

#[test]
fn error_for_graphql_insufficient_scopes_is_missing_scope() {
    let body = r#"{"errors":[{"type":"INSUFFICIENT_SCOPES","locations":[{"line":2,"column":3}],"message":"Your token has not been granted the required scopes to execute this query. The 'updateProjectV2ItemFieldValue' field requires one of the following scopes: ['project'], but your token has only been granted the: ['read:project', 'repo'] scopes. Please modify your token's scopes at: https://github.com/settings/tokens."}]}"#;

    assert_eq!(
        error_for_graphql(body, None, NOW),
        Err(CoreError::MissingScope)
    );
}

#[test]
fn error_for_graphql_forbidden_mutation_is_forbidden() {
    // Recorded from a clearProjectV2ItemFieldValue on a public project the account cannot write.
    let body = r#"{"data":{"clearProjectV2ItemFieldValue":null},"errors":[{"type":"FORBIDDEN","path":["clearProjectV2ItemFieldValue"],"locations":[{"line":1,"column":12}],"message":"sader-agents does not have the correct permissions to execute `ClearProjectV2ItemFieldValue`"}]}"#;

    assert_eq!(
        error_for_graphql(body, None, NOW),
        Err(CoreError::Forbidden)
    );
}

#[test]
fn error_for_graphql_rate_limited_resets_at_the_header() {
    let body = r#"{"errors":[{"type":"RATE_LIMITED","message":"API rate limit exceeded"}]}"#;

    assert_eq!(
        error_for_graphql(body, Some(NOW + 900), NOW),
        Err(CoreError::RateLimited {
            reset_at: NOW + 900
        })
    );
}
