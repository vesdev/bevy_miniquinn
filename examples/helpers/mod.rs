use std::sync::Arc;

use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    rustls::{self, SignatureScheme, pki_types},
};

pub fn insecure_client_config() -> quinn::ClientConfig {
    let rustls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(rustls_config).unwrap()))
}

pub fn insecure_server_config() -> quinn::ServerConfig {
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    let subject_alt_names = vec!["miniquinn-server".to_string(), "localhost".to_string()];

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(subject_alt_names).unwrap();

    let rustls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            pki_types::PrivateKeyDer::Pkcs8(signing_key.serialize_der().into()),
        )
        .unwrap();

    quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(rustls_config).unwrap()))
}

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _: &pki_types::CertificateDer,
        _: &[pki_types::CertificateDer],
        _: &pki_types::ServerName,
        _: &[u8],
        _: pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &pki_types::CertificateDer,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ED25519,
        ]
    }
}
