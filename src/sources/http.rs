//! Plain HTTP/HTTPS file fetching.
//!
//! Provides [`fetch_url`] for downloading raw file contents from any
//! HTTP or HTTPS URL.

use anyhow::{Context, Result};

/// Fetch raw bytes from an HTTP/HTTPS URL.
///
/// # Errors
///
/// Returns an error if the request fails or the server returns a
/// non-success status code.
pub async fn fetch_url(url: &str) -> Result<Vec<u8>> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to fetch {url}"))?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} when fetching {url}");
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("Failed to read response body from {url}"))?;

    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/test.txt")
            .with_status(200)
            .with_body("hello world")
            .create_async()
            .await;

        let bytes = fetch_url(&format!("{}/test.txt", server.url()))
            .await
            .unwrap();
        assert_eq!(bytes, b"hello world");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/missing.txt")
            .with_status(404)
            .create_async()
            .await;

        let result = fetch_url(&format!("{}/missing.txt", server.url())).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("404"), "Error should mention 404: {err}");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_500() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/error")
            .with_status(500)
            .create_async()
            .await;

        let result = fetch_url(&format!("{}/error", server.url())).await;
        assert!(result.is_err());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_binary() {
        let mut server = mockito::Server::new_async().await;
        let binary_data: Vec<u8> = (0..=255).collect();
        let mock = server
            .mock("GET", "/binary.bin")
            .with_status(200)
            .with_body(&binary_data)
            .create_async()
            .await;

        let bytes = fetch_url(&format!("{}/binary.bin", server.url()))
            .await
            .unwrap();
        assert_eq!(bytes, binary_data);
        mock.assert_async().await;
    }
}
