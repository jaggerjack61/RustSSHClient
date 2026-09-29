use std::env;
use std::fs;

use tempfile::tempdir;

use std::time::{Duration, Instant};

use rust_ssh_client::models::{
    AuthType, LoginRequest, SaveLifetime, SshKeyRecord, TransferDirection,
};
use rust_ssh_client::sftp::client as sftp_client;
use rust_ssh_client::ssh::client::{HostKeyPolicy, connect_session, resolve_home_directory};
use rust_ssh_client::ssh::session::{self, ConnectionConfig, SessionCommand, SessionEvent};

/// Connection details from `TEST_SSH_*`. Uses key authentication when
/// `TEST_SSH_KEY_FILE` points to a private key, otherwise `TEST_SSH_PASSWORD`.
fn env_connection() -> Option<(LoginRequest, Option<SshKeyRecord>)> {
    let key = env::var("TEST_SSH_KEY_FILE")
        .ok()
        .map(|path| SshKeyRecord::new("env-key", fs::read_to_string(path).expect("read key")));

    let request = LoginRequest {
        label: Some("env-test".into()),
        host: env::var("TEST_SSH_HOST").ok()?,
        port: env::var("TEST_SSH_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(22),
        username: env::var("TEST_SSH_USERNAME").ok()?,
        password: match key {
            Some(_) => None,
            None => Some(env::var("TEST_SSH_PASSWORD").ok()?),
        },
        auth_type: if key.is_some() {
            AuthType::Key
        } else {
            AuthType::Password
        },
        key_reference: key.as_ref().map(|key| key.id),
        save_host: false,
        save_lifetime: SaveLifetime::Forever,
    };
    Some((request, key))
}

fn env_session() -> ssh2::Session {
    let (request, key) = env_connection().expect("missing TEST_SSH_* environment variables");
    connect_session(&request, key.as_ref(), HostKeyPolicy::Insecure).expect("connect session")
}

#[test]
#[ignore = "requires TEST_SSH_* environment variables"]
fn can_connect_and_list_home_directory() {
    let session = env_session();
    let sftp = session.sftp().expect("create sftp");
    let home = resolve_home_directory(&sftp).expect("resolve home");
    let entries = sftp_client::list_directory(&sftp, &home).expect("list remote home");

    assert!(home.starts_with('/'));
    assert!(entries.iter().all(|entry| !entry.name.is_empty()));
}

#[test]
#[ignore = "requires TEST_SSH_* variables and TEST_SSH_WRITE_DIR"]
fn can_upload_and_download_a_file_round_trip() {
    let remote_directory = env::var("TEST_SSH_WRITE_DIR").expect("missing TEST_SSH_WRITE_DIR");
    let session = env_session();
    let sftp = session.sftp().expect("create sftp");

    let tempdir = tempdir().expect("create tempdir");
    let local_source = tempdir.path().join("roundtrip.txt");
    fs::write(&local_source, "roundtrip-data").expect("write local source");

    let mut upload = sftp_client::queued_transfer("roundtrip.txt", TransferDirection::Upload, 0);
    sftp_client::upload_paths(
        &sftp,
        std::slice::from_ref(&local_source),
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

#[test]
#[ignore = "requires TEST_SSH_* environment variables and a readable child directory"]
fn can_expand_a_directory_in_tree_view() {
    let session = env_session();
    let sftp = session.sftp().expect("create sftp");
    let home = resolve_home_directory(&sftp).expect("resolve home");
    let entries = sftp_client::list_directory(&sftp, &home).expect("list remote home");

    let directory = entries
        .iter()
        .find(|entry| entry.is_directory())
        .expect("expected at least one readable directory in remote home");

    let child_entries =
        sftp_client::list_directory(&sftp, &directory.path).expect("list child directory");

    assert!(directory.path.starts_with('/'));
    assert!(
        child_entries
            .iter()
            .all(|entry| entry.path.starts_with(&directory.path))
    );
}

/// Drives the interactive session worker the way the UI does: host key
/// prompt, shell I/O, `~` directory listing, and an editor round trip.
#[test]
#[ignore = "requires TEST_SSH_* variables and TEST_SSH_WRITE_DIR"]
fn session_worker_end_to_end() {
    let (request, key) = env_connection().expect("missing TEST_SSH_* environment variables");
    let write_dir = env::var("TEST_SSH_WRITE_DIR").expect("missing TEST_SSH_WRITE_DIR");
    let vault = tempdir().expect("create tempdir");
    let known_hosts = vault.path().join("known_hosts");

    let config = ConnectionConfig {
        request,
        key,
        known_hosts: known_hosts.clone(),
    };
    let handle = session::spawn(config.clone()).expect("spawn session");
    let mut events = handle.take_event_receiver().expect("event receiver");

    let mut next_event = |timeout: Duration| -> SessionEvent {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(event) = events.try_recv() {
                return event;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for session event"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    // First connection: the unknown host key must be confirmed.
    match next_event(Duration::from_secs(15)) {
        SessionEvent::HostKeyUnknown(info) => assert!(info.fingerprint.starts_with("SHA256:")),
        other => panic!("expected host key prompt, got {other:?}"),
    }
    handle
        .send(SessionCommand::HostKeyDecision(true))
        .expect("send decision");

    let mut connected = false;
    let mut output = String::new();
    let mut listed_home = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && !(connected && listed_home) {
        match next_event(Duration::from_secs(15)) {
            SessionEvent::Connected { .. } => connected = true,
            SessionEvent::DirectoryLoaded { .. } => listed_home = true,
            SessionEvent::Output(bytes) => output.push_str(&String::from_utf8_lossy(&bytes)),
            SessionEvent::Disconnected(reason) => panic!("disconnected: {reason}"),
            _ => {}
        }
    }
    assert!(connected && listed_home);
    assert!(known_hosts.exists(), "trusted key should be persisted");

    handle
        .send(SessionCommand::SendInput(
            b"echo rustssh-$((40 + 2))\r".to_vec(),
        ))
        .expect("send input");
    handle
        .send(SessionCommand::RefreshDirectory("~".into()))
        .expect("refresh");
    let remote_file = format!("{}/rustssh-e2e.txt", write_dir.trim_end_matches('/'));
    handle
        .send(SessionCommand::WriteFile {
            remote_path: remote_file.clone(),
            contents: "hello from rustssh\n".into(),
        })
        .expect("write file");
    handle
        .send(SessionCommand::ReadFile {
            remote_path: remote_file.clone(),
        })
        .expect("read file");

    let (mut echoed, mut home_request, mut saved, mut read_back) = (false, false, false, false);
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && !(echoed && home_request && saved && read_back) {
        match next_event(Duration::from_secs(15)) {
            SessionEvent::Output(bytes) => {
                output.push_str(&String::from_utf8_lossy(&bytes));
                echoed |= output.contains("rustssh-42");
            }
            SessionEvent::DirectoryLoaded { request, cwd, .. } if request == "~" => {
                assert!(cwd.starts_with('/'));
                home_request = true;
            }
            SessionEvent::FileSaved { path } => saved |= path == remote_file,
            SessionEvent::FileOpened { contents, .. } => {
                assert_eq!(contents, "hello from rustssh\n");
                read_back = true;
            }
            SessionEvent::FileSaveFailed { error, .. }
            | SessionEvent::FileOpenFailed { error, .. } => {
                panic!("file operation failed: {error}")
            }
            SessionEvent::Disconnected(reason) => panic!("disconnected: {reason}"),
            _ => {}
        }
    }
    assert!(echoed, "shell output: {output}");
    assert!(home_request && saved && read_back);

    handle
        .send(SessionCommand::Delete {
            remote_path: remote_file,
        })
        .expect("delete");
    handle.send(SessionCommand::Disconnect).expect("disconnect");
    loop {
        if let SessionEvent::Disconnected(reason) = next_event(Duration::from_secs(10)) {
            assert_eq!(reason, session::DISCONNECTED_BY_USER);
            break;
        }
    }

    // Second connection: the saved key is trusted without a prompt.
    let handle = session::spawn(config).expect("respawn session");
    let mut events = handle.take_event_receiver().expect("event receiver");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(Instant::now() < deadline, "timed out reconnecting");
        match events.try_recv() {
            Ok(SessionEvent::Connected { .. }) => break,
            Ok(SessionEvent::HostKeyUnknown(_)) => panic!("known host prompted again"),
            Ok(SessionEvent::Disconnected(reason)) => panic!("disconnected: {reason}"),
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Regression test: heavy output while typing byte-by-byte and interleaving
/// SFTP work used to corrupt the channel window until the server dropped
/// the connection ("adjust ... overflows remote window").
#[test]
#[ignore = "requires TEST_SSH_* environment variables"]
fn session_survives_heavy_output_with_interleaved_commands() {
    let (request, key) = env_connection().expect("missing TEST_SSH_* environment variables");
    let vault = tempdir().expect("create tempdir");
    let handle = session::spawn(ConnectionConfig {
        request,
        key,
        known_hosts: vault.path().join("known_hosts"),
    })
    .expect("spawn session");
    let mut events = handle.take_event_receiver().expect("event receiver");

    let mut output = String::new();
    let pump = |events: &mut rust_ssh_client::ssh::session::EventReceiver, output: &mut String| {
        while let Ok(event) = events.try_recv() {
            match event {
                SessionEvent::HostKeyUnknown(_) => handle
                    .send(SessionCommand::HostKeyDecision(true))
                    .expect("trust"),
                SessionEvent::Output(bytes) => output.push_str(&String::from_utf8_lossy(&bytes)),
                SessionEvent::Disconnected(reason) => panic!("disconnected: {reason}"),
                _ => {}
            }
        }
    };

    let deadline = Instant::now() + Duration::from_secs(15);
    while !output.contains('$') && !output.contains('%') && !output.contains('#') {
        assert!(Instant::now() < deadline, "no prompt");
        pump(&mut events, &mut output);
        std::thread::sleep(Duration::from_millis(10));
    }

    let command = b"seq 1 150000; echo DONE-$((6 * 7))\r";
    for (index, byte) in command.iter().enumerate() {
        handle
            .send(SessionCommand::SendInput(vec![*byte]))
            .expect("send byte");
        if index % 5 == 0 {
            handle
                .send(SessionCommand::RefreshDirectory("~".into()))
                .expect("refresh");
        }
        pump(&mut events, &mut output);
    }
    // Keep typing while the output streams.
    for _ in 0..200 {
        handle
            .send(SessionCommand::SendInput(b" ".to_vec()))
            .expect("send space");
        handle
            .send(SessionCommand::ResizeTerminal {
                cols: 100,
                rows: 30,
            })
            .expect("resize");
        pump(&mut events, &mut output);
        std::thread::sleep(Duration::from_millis(2));
    }

    let deadline = Instant::now() + Duration::from_secs(30);
    while !output.contains("DONE-42") {
        assert!(
            Instant::now() < deadline,
            "output incomplete: {} bytes",
            output.len()
        );
        pump(&mut events, &mut output);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(output.contains("\n150000"), "lost output");
    handle.send(SessionCommand::Disconnect).expect("disconnect");
}
