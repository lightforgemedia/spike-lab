//! Google Drive artifact uploads.
//!
//! Design goals:
//! - KISS: one function that takes bytes and uploads a file into a configured folder.
//! - No background workers: upload happens inline on the upload endpoint.
//! - Safe defaults: uploaded files inherit the destination folder's permissions.
//!
//! Notes:
//! - This uses a service-account JSON key.
//! - The destination folder must be shared with the service account's `client_email`.

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};

// google-apis-rs generated client.
use google_drive3 as drive3;

#[derive(Clone, Debug)]
pub struct UploadOutcome {
    /// Drive file ID.
    pub file_id: String,
    /// Optional browser-view link (requires correct API response fields and permissions).
    pub web_view_link: Option<String>,
}

pub async fn upload_zip_bytes(
    service_account_json: &Path,
    folder_id: &str,
    filename: &str,
    bytes: Vec<u8>,
) -> Result<UploadOutcome> {
    // Read service account key.
    let key = drive3::yup_oauth2::read_service_account_key(service_account_json)
        .await
        .with_context(|| {
            format!(
                "read google service account key from {}",
                service_account_json.display()
            )
        })?;

    // TLS connector + HTTP client.
    let connector = drive3::hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .context("load native root certs")?
        .https_only()
        .enable_http2()
        .build();

    let client = drive3::hyper_util::client::legacy::Client::builder(
        drive3::hyper_util::rt::TokioExecutor::new(),
    )
    .build(connector);

    // Service-account authenticator. We pass a custom client builder so yup-oauth2
    // doesn't need its own TLS stack configured.
    let auth = drive3::yup_oauth2::ServiceAccountAuthenticator::with_client(
        key,
        drive3::yup_oauth2::client::CustomHyperClientBuilder::from(client.clone()),
    )
    .build()
    .await
    .context("build service account authenticator")?;

    let hub = drive3::DriveHub::new(client, auth);

    // File metadata.
    let mut f = drive3::api::File::default();
    f.name = Some(filename.to_string());
    f.parents = Some(vec![folder_id.to_string()]);

    // Upload.
    let mime: drive3::mime::Mime = "application/zip".parse().expect("valid mime");
    let cursor = Cursor::new(bytes);

    let (_resp, created) = hub
        .files()
        .create(f)
        // Make this work for shared drives too.
        .supports_all_drives(true)
        // Only ask for what we need.
        .param("fields", "id,webViewLink")
        .upload(cursor, mime)
        .await
        .context("upload to Google Drive")?;

    let file_id = created.id.unwrap_or_default();
    Ok(UploadOutcome {
        file_id,
        web_view_link: created.web_view_link,
    })
}
