use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use ssh2::{MethodType, Session};
use tracing::{info, warn};

use crate::error::{AppError, AppResult};
use crate::models::{AuthType, LoginRequest, SshKeyRecord};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SESSION_TIMEOUT_MS: u32 = 30_000;
const PREFERRED_KEX_METHODS: &[&str] = &[
    "curve25519-sha256",
    "curve25519-sha256@libssh.org",
    "sntrup761x25519-sha512@openssh.com",
    "sntrup761x25519-sha512",
    "mlkem768x25519-sha256",
    "ecdh-sha2-nistp521",
    "ecdh-sha2-nistp384",
    "ecdh-sha2-nistp256",
    "diffie-hellman-group18-sha512",
    "diffie-hellman-group16-sha512",
    "diffie-hellman-group14-sha256",
    "diffie-hellman-group14-sha1",
    "diffie-hellman-group-exchange-sha256",
    "diffie-hellman-group-exchange-sha1",
];
const PREFERRED_HOSTKEY_METHODS: &[&str] = &[
    "ssh-ed25519",
    "ecdsa-sha2-nistp521",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp256",
    "rsa-sha2-512",
    "rsa-sha2-256",
    "ssh-rsa",
    "ssh-dss",
];

pub fn connect_session(request: &LoginRequest, key: Option<&SshKeyRecord>) -> AppResult<Session> {
    request.validate()?;

    let tcp = connect_tcp_stream(request)?;
    // TCP-level timeouts must NOT be set before handshake — a 5-second read
    // timeout fires mid key-exchange on slow/high-latency servers and libssh2
    // maps the resulting IO error to LIBSSH2_ERROR_KEX_FAILURE (-5).
    // session.set_timeout() below controls per-operation timeouts instead.

    let mut session = Session::new().map_err(|error| AppError::Ssh(error.to_string()))?;
    session.set_tcp_stream(tcp);
    let kex_preferences = configure_method_preferences(&session, MethodType::Kex, PREFERRED_KEX_METHODS);
    let hostkey_preferences = configure_method_preferences(
        &session,
        MethodType::HostKey,
        PREFERRED_HOSTKEY_METHODS,
    );

    session.handshake().map_err(|error| {
        handshake_error(
            request,
            error,
            kex_preferences.as_deref(),
            hostkey_preferences.as_deref(),
        )
    })?;
    session.set_timeout(SESSION_TIMEOUT_MS);
    session.set_keepalive(true, 15);

    info!(
        host = %request.host,
        negotiated_kex = session.methods(MethodType::Kex).unwrap_or("unknown"),
        negotiated_host_key = session.methods(MethodType::HostKey).unwrap_or("unknown"),
        "SSH handshake completed"
    );

    authenticate(&session, request, key)?;
    info!(host = %request.host, user = %request.username, "SSH session authenticated");
    Ok(session)
}

pub fn resolve_home_directory(session: &Session) -> AppResult<String> {
    let sftp = session.sftp()?;
    let home = sftp.realpath(Path::new("."))?;
    home.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| AppError::Ssh("Remote home directory contains non-UTF-8 text.".into()))
}

fn authenticate(
    session: &Session,
    request: &LoginRequest,
    key: Option<&SshKeyRecord>,
) -> AppResult<()> {
    match request.auth_type {
        AuthType::Password => session.userauth_password(
            &request.username,
            request.password.as_deref().unwrap_or_default(),
        )?,
        AuthType::Key => {
            let key = key.ok_or_else(|| {
                AppError::Validation(
                    "Key authentication was selected but no key was loaded.".into(),
                )
            })?;

            session.userauth_pubkey_memory(&request.username, None, &key.pem_contents, None)?;
        }
    }

    if !session.authenticated() {
        return Err(AppError::Ssh("Authentication failed.".into()));
    }

    Ok(())
}

fn connect_tcp_stream(request: &LoginRequest) -> AppResult<TcpStream> {
    let socket_address = request.socket_address();
    let addresses = socket_address
        .to_socket_addrs()?
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        return Err(AppError::Validation(format!(
            "Unable to resolve any socket addresses for {}.",
            socket_address
        )));
    }

    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| std::io::Error::other(format!("Unable to connect to {}.", socket_address)))
        .into())
}

fn configure_method_preferences(
    session: &Session,
    method_type: MethodType,
    preferred_methods: &[&str],
) -> Option<String> {
    let supported = match session.supported_algs(method_type) {
        Ok(supported) => supported,
        Err(error) => {
            warn!(
                method = method_type_name(method_type),
                error = %error,
                "Unable to query libssh2-supported algorithms; falling back to default negotiation order"
            );
            return None;
        }
    };

    let preferences = build_method_preferences(&supported, preferred_methods);
    if preferences.is_empty() {
        warn!(
            method = method_type_name(method_type),
            "No supported algorithms were reported; falling back to libssh2 defaults"
        );
        return None;
    }

    if let Err(error) = session.method_pref(method_type, &preferences) {
        warn!(
            method = method_type_name(method_type),
            error = %error,
            "Unable to apply preferred algorithm order; falling back to libssh2 defaults"
        );
        return None;
    }

    Some(preferences)
}

fn build_method_preferences(supported: &[&str], preferred_methods: &[&str]) -> String {
    let mut ordered = Vec::with_capacity(supported.len());

    for preferred in preferred_methods {
        if supported.contains(preferred) {
            ordered.push(*preferred);
        }
    }

    for algorithm in supported {
        if !ordered.contains(algorithm) {
            ordered.push(*algorithm);
        }
    }

    ordered.join(",")
}

fn method_type_name(method_type: MethodType) -> &'static str {
    match method_type {
        MethodType::Kex => "kex",
        MethodType::HostKey => "host-key",
        MethodType::CryptCs => "client cipher",
        MethodType::CryptSc => "server cipher",
        MethodType::MacCs => "client mac",
        MethodType::MacSc => "server mac",
        MethodType::CompCs => "client compression",
        MethodType::CompSc => "server compression",
        MethodType::LangCs => "client language",
        MethodType::LangSc => "server language",
        MethodType::SignAlgo => "signature algorithm",
    }
}

fn handshake_error(
    request: &LoginRequest,
    error: ssh2::Error,
    kex_preferences: Option<&str>,
    hostkey_preferences: Option<&str>,
) -> AppError {
    AppError::Ssh(format!(
        "Failed SSH handshake with {}:{}: {}. Client KEX proposal: {}. Client host-key proposal: {}.",
        request.host.trim(),
        request.port,
        error,
        kex_preferences.unwrap_or("libssh2 default order"),
        hostkey_preferences.unwrap_or("libssh2 default order"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{build_method_preferences, PREFERRED_KEX_METHODS};
    #[cfg(windows)]
    use ssh2::{MethodType, Session};

    #[test]
    fn preserves_supported_algorithms_outside_the_preferred_subset() {
        let supported = [
            "diffie-hellman-group16-sha512",
            "curve25519-sha256",
            "ecdh-sha2-nistp256",
            "vendor-specific-kex",
        ];

        let preferences = build_method_preferences(&supported, PREFERRED_KEX_METHODS);

        assert_eq!(
            preferences,
            "curve25519-sha256,ecdh-sha2-nistp256,diffie-hellman-group16-sha512,vendor-specific-kex"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_build_exposes_modern_kex_and_hostkey_algorithms() {
        let session = Session::new().expect("create session");
        let kex = session.supported_algs(MethodType::Kex).expect("kex algorithms");
        let host_keys = session
            .supported_algs(MethodType::HostKey)
            .expect("host key algorithms");

        assert!(
            kex.contains(&"curve25519-sha256") || kex.contains(&"ecdh-sha2-nistp256"),
            "expected curve25519 or ecdh support, got {kex:?}"
        );
        assert!(
            host_keys.contains(&"ssh-ed25519") || host_keys.contains(&"ecdsa-sha2-nistp256"),
            "expected ed25519 or ecdsa support, got {host_keys:?}"
        );
    }
}
