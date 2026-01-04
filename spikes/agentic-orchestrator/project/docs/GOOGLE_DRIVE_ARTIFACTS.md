# Google Drive artifacts

This project can optionally upload each job's ZIP bundle to a Google Drive folder.

When enabled, the daemon returns `gdrive://<file_id>` as the `artifact_uri` from:

`POST /v1/jobs/{job_id}/artifacts`

The local filesystem copy is still kept under `--artifact-store-dir`.

## Setup

### 1) Enable the Drive API

In your Google Cloud project:

1. Enable **Google Drive API**.
2. Create a **service account**.
3. Create a JSON key and download it (keep it secret).

### 2) Create a destination folder and share it

1. Create a folder in Google Drive (My Drive or a Shared Drive).
2. Share that folder with the service account's `client_email` (from the JSON key).
   Give it **Editor** access.
3. Copy the folder ID from the URL. It looks like:

   `https://drive.google.com/drive/folders/<FOLDER_ID>`

### 3) Configure the daemon

Start the daemon with:

```bash
cargo run -p orchestrator-daemon -- \
  --gdrive-service-account-json /path/to/service_account.json \
  --gdrive-folder-id <FOLDER_ID>
```

## Notes

- Uploaded files inherit the folder's sharing permissions.
  If you want humans to open the artifacts, share the folder appropriately.
- If Drive upload fails (quota, auth, permissions), the daemon logs a warning and
  falls back to returning the local artifact path.
