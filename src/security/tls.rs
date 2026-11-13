//! HTTPS listener and app-generated local certificate authority.
//!
//! The pure server refuses cleartext HTTP on a non-loopback bind. A local CA
//! and server leaf are generated under `ADDRESS_EXPLORER_TLS_DIR`; clients
//! install the CA once or the operator supplies a trusted certificate.

use crate::security::files::{ensure_private_dir, write_private};
use dioxus::server::axum::Router;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

struct TlsInfo {
    cert_path: PathBuf,
    key_path: PathBuf,
    fingerprint: Option<String>,
    ca_pem: Option<Vec<u8>>,
}

static INFO: LazyLock<RwLock<Option<TlsInfo>>> = LazyLock::new(|| RwLock::new(None));

/// Generate or load the certificate material. Safe to call more than once.
pub fn ensure() -> Result<(), String> {
    if matches!(INFO.read(), Ok(info) if info.is_some()) {
        return Ok(());
    }
    let info = load_or_generate()?;
    let mut guard = INFO
        .write()
        .map_err(|_| "the TLS state lock was poisoned".to_string())?;
    *guard = Some(info);
    Ok(())
}

/// SHA-256 fingerprint of the active leaf certificate, when generated locally.
pub fn fingerprint() -> Option<String> {
    INFO.read()
        .ok()
        .and_then(|info| info.as_ref().and_then(|info| info.fingerprint.clone()))
}

/// PEM of the local CA, when this server generated one.
pub fn ca_pem() -> Option<Vec<u8>> {
    INFO.read()
        .ok()
        .and_then(|info| info.as_ref().and_then(|info| info.ca_pem.clone()))
}

/// Serve the router over TLS. This blocks until the process exits.
pub fn serve(router: Router, addr: SocketAddr) -> Result<(), String> {
    ensure()?;
    let (cert_path, key_path) = {
        let guard = INFO
            .read()
            .map_err(|_| "the TLS state lock was poisoned".to_string())?;
        let info = guard
            .as_ref()
            .ok_or_else(|| "TLS certificate state is missing".to_string())?;
        (info.cert_path.clone(), info.key_path.clone())
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the server runtime: {error}"))?;
    runtime.block_on(async move {
        let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .map_err(|error| format!("could not load the TLS certificate: {error}"))?;
        axum_server::bind_rustls(addr, config)
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|error| format!("TLS server failed: {error}"))
    })
}

fn load_or_generate() -> Result<TlsInfo, String> {
    if let (Ok(cert), Ok(key)) = (
        std::env::var("ADDRESS_EXPLORER_TLS_CERT"),
        std::env::var("ADDRESS_EXPLORER_TLS_KEY"),
    ) {
        let cert_path = PathBuf::from(cert);
        let key_path = PathBuf::from(key);
        if !cert_path.is_file() || !key_path.is_file() {
            return Err("ADDRESS_EXPLORER_TLS_CERT/KEY must point to existing files".to_string());
        }
        let ca_pem = std::env::var("ADDRESS_EXPLORER_TLS_CA")
            .ok()
            .and_then(|path| std::fs::read(path).ok());
        return Ok(TlsInfo {
            cert_path,
            key_path,
            fingerprint: None,
            ca_pem,
        });
    }
    generate_local_ca()
}

fn generate_local_ca() -> Result<TlsInfo, String> {
    let directory = tls_directory()?;
    ensure_private_dir(&directory)?;
    crate::security::files::restrict_dir(&directory)?;
    let ca_cert_path = directory.join("ca.crt");
    let ca_key_path = directory.join("ca.key");
    let cert_path = directory.join("server.crt");
    let key_path = directory.join("server.key");
    let manifest_path = directory.join("tls-manifest.json");
    let hosts = configured_hosts()?;

    let mut ca_generated = false;
    if !ca_cert_path.is_file() || !ca_key_path.is_file() {
        generate_ca(&ca_cert_path, &ca_key_path)?;
        ca_generated = true;
    }

    let manifest_matches = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|text| serde_json::from_str::<TlsManifest>(&text).ok())
        .is_some_and(|manifest| manifest.hosts == hosts);
    if ca_generated || !manifest_matches || !cert_path.is_file() || !key_path.is_file() {
        generate_leaf(&ca_cert_path, &ca_key_path, &cert_path, &key_path, &hosts)?;
    }

    let fingerprint = certificate_fingerprint(&cert_path)?;
    let ca_pem = std::fs::read(&ca_cert_path)
        .map_err(|error| format!("could not read the local CA certificate: {error}"))?;
    let manifest = TlsManifest {
        hosts,
        fingerprint: fingerprint.clone(),
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("could not encode the TLS manifest: {error}"))?;
    write_private(&manifest_path, &manifest_json)?;
    Ok(TlsInfo {
        cert_path,
        key_path,
        fingerprint: Some(fingerprint),
        ca_pem: Some(ca_pem),
    })
}

fn generate_ca(cert_path: &Path, key_path: &Path) -> Result<(), String> {
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|error| format!("could not build the CA parameters: {error}"))?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Address Explorer Local CA");
    params
        .distinguished_name
        .push(DnType::CommonName, "Address Explorer Local CA");
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    params.not_before = rcgen::date_time_ymd(2025, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let key =
        KeyPair::generate().map_err(|error| format!("could not generate the CA key: {error}"))?;
    let cert = params
        .self_signed(&key)
        .map_err(|error| format!("could not sign the local CA: {error}"))?;
    write_private(cert_path, cert.pem().as_bytes())?;
    write_private(key_path, key.serialize_pem().as_bytes())
}

fn generate_leaf(
    ca_cert_path: &Path,
    ca_key_path: &Path,
    cert_path: &Path,
    key_path: &Path,
    hosts: &[String],
) -> Result<(), String> {
    let ca_pem = std::fs::read_to_string(ca_cert_path)
        .map_err(|error| format!("could not read the local CA: {error}"))?;
    let ca_key_pem = std::fs::read_to_string(ca_key_path)
        .map_err(|error| format!("could not read the local CA key: {error}"))?;
    let ca_key = KeyPair::from_pem(&ca_key_pem)
        .map_err(|error| format!("could not parse the local CA key: {error}"))?;
    let issuer = Issuer::from_ca_cert_pem(&ca_pem, ca_key)
        .map_err(|error| format!("could not use the local CA: {error}"))?;
    let mut params = CertificateParams::new(hosts.to_vec())
        .map_err(|error| format!("could not build the server certificate: {error}"))?;
    params.distinguished_name.push(
        DnType::CommonName,
        hosts.first().map(String::as_str).unwrap_or("localhost"),
    );
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.use_authority_key_identifier_extension = true;
    params.not_before = rcgen::date_time_ymd(2025, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let key = KeyPair::generate()
        .map_err(|error| format!("could not generate the server key: {error}"))?;
    let cert = params
        .signed_by(&key, &issuer)
        .map_err(|error| format!("could not sign the server certificate: {error}"))?;
    write_private(cert_path, cert.pem().as_bytes())?;
    write_private(key_path, key.serialize_pem().as_bytes())
}

fn certificate_fingerprint(path: &Path) -> Result<String, String> {
    let pem = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read the server certificate: {error}"))?;
    let der = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, der.as_bytes())
        .map_err(|_| "the server certificate is not valid PEM".to_string())?;
    let digest = Sha256::digest(&der);
    Ok(hex(&digest))
}

fn configured_hosts() -> Result<Vec<String>, String> {
    let mut hosts = Vec::new();
    if let Ok(value) = std::env::var("ADDRESS_EXPLORER_TLS_HOSTS") {
        hosts.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string),
        );
    }
    if hosts.is_empty() {
        let ip = std::env::var("IP").unwrap_or_else(|_| "127.0.0.1".to_string());
        if ip == "0.0.0.0" || ip == "::" || ip.is_empty() {
            return Err(
                "ADDRESS_EXPLORER_TLS_HOSTS is required when binding to a wildcard address"
                    .to_string(),
            );
        }
        hosts.push(ip);
    }
    for local in ["localhost", "127.0.0.1", "::1"] {
        if !hosts.iter().any(|host| host == local) {
            hosts.push(local.to_string());
        }
    }
    Ok(hosts)
}

fn tls_directory() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ADDRESS_EXPLORER_TLS_DIR") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    let database = crate::data::Database::default_path()
        .map_err(|error| format!("could not resolve the database path: {error}"))?;
    Ok(database
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tls"))
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct TlsManifest {
    hosts: Vec<String>,
    fingerprint: String,
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn generated_ca_and_leaf_are_private_and_valid_pem() {
        use std::os::unix::fs::PermissionsExt;

        let directory =
            std::env::temp_dir().join(format!("address-explorer-tls-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        ensure_private_dir(&directory).expect("directory"); // allow-unwrap: test setup.
        let ca_cert = directory.join("ca.crt");
        let ca_key = directory.join("ca.key");
        let cert = directory.join("server.crt");
        let key = directory.join("server.key");
        generate_ca(&ca_cert, &ca_key).expect("ca"); // allow-unwrap: test setup.
        generate_leaf(
            &ca_cert,
            &ca_key,
            &cert,
            &key,
            &["localhost".to_string(), "127.0.0.1".to_string()],
        )
        .expect("leaf"); // allow-unwrap: test setup.
        let ca_pem = std::fs::read_to_string(&ca_cert).expect("ca pem"); // allow-unwrap: test setup.
        assert!(ca_pem.contains("BEGIN CERTIFICATE"));
        let fingerprint = certificate_fingerprint(&cert).expect("fingerprint"); // allow-unwrap: test setup.
        assert_eq!(fingerprint.len(), 64);
        for path in [ca_cert, ca_key, cert, key] {
            let metadata = std::fs::metadata(&path).expect("metadata"); // allow-unwrap: test setup.
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(directory);
    }
}
