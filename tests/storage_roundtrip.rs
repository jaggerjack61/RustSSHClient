use tempfile::tempdir;

use chrono::{Duration, Utc};

use rust_ssh_client::models::{AuthType, HostRecord, LoginRequest, SaveLifetime, SshKeyRecord};
use rust_ssh_client::storage::credentials::CredentialsStore;
use rust_ssh_client::storage::keys::KeyStore;
use rust_ssh_client::storage::{StorageFacade, StorageSnapshot};

#[test]
fn encrypted_host_storage_round_trips() {
    let tempdir = tempdir().expect("create tempdir");
    let store = CredentialsStore::with_key(tempdir.path().join("hosts.vault"), [3; 32]);

    let request = LoginRequest {
        label: Some("Production DB".into()),
        host: "10.0.4.12".into(),
        port: 22,
        username: "root".into(),
        password: Some("secret".into()),
        auth_type: AuthType::Password,
        key_reference: None,
        save_host: true,
        save_lifetime: SaveLifetime::Forever,
    };

    let hosts = vec![HostRecord::new(&request)];
    store.save_hosts(&hosts).expect("save hosts");

    let loaded = store.load_hosts().expect("load hosts");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].host, "10.0.4.12");
    assert_eq!(loaded[0].password.as_deref(), Some("secret"));
}

#[test]
fn encrypted_key_storage_round_trips() {
    let tempdir = tempdir().expect("create tempdir");
    let store = KeyStore::with_key(tempdir.path().join("keys.vault"), [5; 32]);
    let keys = vec![SshKeyRecord::new(
        "deploy",
        "-----BEGIN PRIVATE KEY-----\n...",
    )];

    store.save_keys(&keys).expect("save keys");
    let loaded = store.load_keys().expect("load keys");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].label, "deploy");
    assert!(loaded[0].pem_contents.contains("BEGIN PRIVATE KEY"));
}

#[test]
fn storage_facade_round_trips_snapshot_across_instances() {
    let tempdir = tempdir().expect("create tempdir");
    let facade = StorageFacade::for_root(tempdir.path().to_path_buf());

    let snapshot = StorageSnapshot {
        hosts: vec![HostRecord::new(&LoginRequest {
            label: Some("Production DB".into()),
            host: "10.0.4.12".into(),
            port: 22,
            username: "root".into(),
            password: Some("secret".into()),
            auth_type: AuthType::Password,
            key_reference: None,
            save_host: true,
            save_lifetime: SaveLifetime::Forever,
        })],
        keys: vec![SshKeyRecord::new(
            "deploy",
            "-----BEGIN PRIVATE KEY-----\n...",
        )],
    };

    facade.save_snapshot(&snapshot).expect("save snapshot");

    let reloaded = StorageFacade::for_root(tempdir.path().to_path_buf())
        .load_snapshot()
        .expect("reload snapshot");

    assert_eq!(reloaded.hosts, snapshot.hosts);
    assert_eq!(reloaded.keys, snapshot.keys);
}

#[test]
fn storage_facade_prunes_expired_hosts() {
    let tempdir = tempdir().expect("create tempdir");
    let facade = StorageFacade::for_root(tempdir.path().to_path_buf());

    let mut expired = HostRecord::new(&LoginRequest {
        label: Some("Temporary".into()),
        host: "10.0.4.12".into(),
        port: 22,
        username: "root".into(),
        password: Some("secret".into()),
        auth_type: AuthType::Password,
        key_reference: None,
        save_host: true,
        save_lifetime: SaveLifetime::OneHour,
    });
    expired.expires_at = Some(Utc::now() - Duration::minutes(5));

    facade
        .save_snapshot(&StorageSnapshot {
            hosts: vec![expired],
            keys: Vec::new(),
        })
        .expect("save snapshot");

    let reloaded = facade.load_snapshot().expect("reload snapshot");

    assert!(reloaded.hosts.is_empty());
}
