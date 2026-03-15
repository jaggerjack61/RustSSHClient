use std::env;
use std::fs;

use tempfile::tempdir;

use rust_ssh_client::models::{AuthType, LoginRequest, SaveLifetime, TransferDirection};
use rust_ssh_client::sftp::client as sftp_client;
use rust_ssh_client::ssh::client::{connect_session, resolve_home_directory};

fn env_request() -> Option<LoginRequest> {
    Some(LoginRequest {
        label: Some("env-test".into()),
        host: env::var("TEST_SSH_HOST").ok()?,
        port: env::var("TEST_SSH_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(22),
        username: env::var("TEST_SSH_USERNAME").ok()?,
        password: Some(env::var("TEST_SSH_PASSWORD").ok()?),
        auth_type: AuthType::Password,
        key_reference: None,
        save_host: false,
        save_lifetime: SaveLifetime::Forever,
    })
}

#[test]
#[ignore = "requires TEST_SSH_* environment variables"]
fn can_connect_and_list_home_directory() {
    let request = env_request().expect("missing TEST_SSH_* environment variables");
    let session = connect_session(&request, None).expect("connect session");
    let home = resolve_home_directory(&session).expect("resolve home");
    let sftp = session.sftp().expect("create sftp");
    let entries = sftp_client::list_directory(&sftp, &home).expect("list remote home");

    assert!(home.starts_with('/'));
    assert!(entries.iter().all(|entry| !entry.name.is_empty()));
}

#[test]
#[ignore = "requires TEST_SSH_* variables and TEST_SSH_WRITE_DIR"]
fn can_upload_and_download_a_file_round_trip() {
    let request = env_request().expect("missing TEST_SSH_* environment variables");
    let remote_directory = env::var("TEST_SSH_WRITE_DIR").expect("missing TEST_SSH_WRITE_DIR");
    let session = connect_session(&request, None).expect("connect session");
    let sftp = session.sftp().expect("create sftp");

    let tempdir = tempdir().expect("create tempdir");
    let local_source = tempdir.path().join("roundtrip.txt");
    fs::write(&local_source, "roundtrip-data").expect("write local source");

    let mut upload = sftp_client::queued_transfer("roundtrip.txt", TransferDirection::Upload, 0);
    sftp_client::upload_paths(
        &sftp,
        &[local_source.clone()],
        &remote_directory,
        &mut upload,
        |_| {},
    )
    .expect("upload file");

    let remote_path = format!("{}/roundtrip.txt", remote_directory.trim_end_matches('/'));
    let download_dir = tempdir.path().join("download");
    let mut download =
        sftp_client::queued_transfer("roundtrip.txt", TransferDirection::Download, 0);
    sftp_client::download_entry(&sftp, &remote_path, &download_dir, &mut download, |_| {})
        .expect("download file");

    let downloaded_contents =
        fs::read_to_string(download_dir.join("roundtrip.txt")).expect("read downloaded file");
    assert_eq!(downloaded_contents, "roundtrip-data");

    sftp_client::delete_entry(&sftp, &remote_path).expect("cleanup remote file");
}
