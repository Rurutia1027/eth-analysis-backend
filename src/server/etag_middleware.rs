use axum::{
    body::HttpBody,
    http::{header, HeaderValue, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use bytes::BufMut;
use etag::EntityTag;
use reqwest::StatusCode;
use tracing::{error, trace};

pub async fn middleware_fn<B: std::fmt::Debug>(
    req: Request<B>,
    next: Next<B>
) -> Result<Response, StatusCode> {
    let if_none_match_header = req.headers().get(header::IF_NONE_MATCH).cloned();
    let path = req.uri().path().to_owned();
    let res = next.run(req).await;
    let (mut parts, mut body) = res.into_parts();
    let bytes = {
        let mut body_bytes = vec![];
        while let Some(inner) = body.data().await {
            let bytes = inner.unwrap();
            body_bytes.put(bytes);
        }
        body_bytes
    };

    match bytes.is_empty() {
        true => {
            Ok(parts.into_response())
        }
        false => match if_none_match_header {
            None => {
                let etag = EntityTag::from_data(&bytes);
                parts.headers.insert(header::ETAG,
                HeaderValue::from_str(&etag.to_string()).unwrap(),);
                Ok((parts, bytes).into_response())
            }
            Some(if_none_match_header) => {
                let if_none_match_header = if_none_match_header.to_str().unwrap().parse::<EntityTag>();
                match if_none_match_header {
                    Err(ref err) => {
                        let etag = EntityTag::from_data(&bytes);
                        parts.headers.insert(
                            header::ETAG,
                            HeaderValue::from_str(&etag.to_string()).unwrap(),
                        );
                        Ok((parts, bytes).into_response())
                    }
                    Ok(if_none_match_etag) => {
                        let etag = EntityTag::from_data(&bytes);
                        parts.headers.insert(
                            header::ETAG,
                            HeaderValue::from_str(&if_none_match_etag.to_string()).unwrap(),
                        );
                        let some_match = etag.strong_eq(&if_none_match_etag);

                        if some_match {
                            Ok((StatusCode::NOT_MODIFIED, parts).into_response())
                        } else {
                            Ok((parts, bytes).into_response())
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, Bytes},
        http::{Request, Response, StatusCode, header},
        middleware::from_fn,
        routing::get,
        Router,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_etag_middleware_no_if_none_match() {
        let app = Router::new()
            .route("/", get(|| async { "Hello, world!" }))
            .layer(from_fn(middleware_fn));

        let response = app.clone()
            .oneshot(Request::builder().uri("/").body(Body::from("Hello, world!")).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(header::ETAG));
    }

    #[tokio::test]
    async fn test_etag_middleware_with_matching_if_none_match() {
        let app = Router::new()
            .route("/", get(|| async { "Hello, world!" }))
            .layer(from_fn(middleware_fn));

        let initial_response = app.clone()
            .oneshot(Request::builder().uri("/").body(Body::from("Hello, world!")).unwrap())
            .await
            .unwrap();

        let etag = initial_response.headers().get(header::ETAG).unwrap().to_str().unwrap().to_string();

        let response = app.clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::IF_NONE_MATCH, etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }

    #[tokio::test]
    async fn test_etag_middleware_with_non_matching_if_none_match() {
        let app = Router::new()
            .route("/", get(|| async { "Hello, world!" }))
            .layer(from_fn(middleware_fn));

        let response = app.clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::IF_NONE_MATCH, "\"different-etag\"")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(header::ETAG));
    }
}