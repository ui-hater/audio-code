//! End-to-end encryption of every connection: an X25519 key exchange when the connection
//! opens, then XChaCha20-Poly1305 with one key per direction and counting nonces.
//!
//! Every connection makes fresh keys, so recorded traffic cannot be read later even if a
//! computer is taken over (forward secrecy). A changed, repeated, reordered or dropped message
//! fails the authentication and closes the connection.

use std::io;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

/// Authenticated with every message, so messages of other programs never decrypt.
const AAD: &[u8] = b"audio-code net v2";

/// A fresh key pair for one connection.
pub(crate) struct KeyPair {
    secret: StaticSecret,
    pub(crate) public: [u8; 32],
}

impl KeyPair {
    pub(crate) fn new() -> io::Result<Self> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
        let secret = StaticSecret::from(bytes);
        let public = PublicKey::from(&secret).to_bytes();
        Ok(Self { secret, public })
    }
}

/// The encryption state of one connection.
pub(crate) struct Cipher {
    send: XChaCha20Poly1305,
    receive: XChaCha20Poly1305,
    sent: u64,
    received: u64,
}

impl Cipher {
    /// Combines our key pair with the other side's public key. `None` if the other side sent a
    /// weak key that would make the shared secret predictable.
    pub(crate) fn new(mine: KeyPair, theirs: [u8; 32], client: bool) -> Option<Self> {
        let shared = mine.secret.diffie_hellman(&PublicKey::from(theirs));
        if !shared.was_contributory() {
            return None;
        }
        let (client_key, server_key) = if client { (mine.public, theirs) } else { (theirs, mine.public) };
        let derive = |label: &[u8]| {
            let mut hash = Sha256::new();
            hash.update(label);
            hash.update(shared.as_bytes());
            hash.update(client_key);
            hash.update(server_key);
            let key: [u8; 32] = hash.finalize().into();
            XChaCha20Poly1305::new(&Key::from(key))
        };
        let to_server = derive(b"audio-code net client to server");
        let to_client = derive(b"audio-code net server to client");
        let (send, receive) = if client { (to_server, to_client) } else { (to_client, to_server) };
        Some(Self { send, receive, sent: 0, received: 0 })
    }

    pub(crate) fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let nonce = nonce(self.sent);
        self.sent += 1;
        self.send.encrypt(&nonce, Payload { msg: plain, aad: AAD }).expect("encrypting into memory cannot fail")
    }

    /// `None` if the message was changed or is not the next one.
    pub(crate) fn open(&mut self, sealed: &[u8]) -> Option<Vec<u8>> {
        let plain = self.receive.decrypt(&nonce(self.received), Payload { msg: sealed, aad: AAD }).ok()?;
        self.received += 1;
        Some(plain)
    }
}

fn nonce(counter: u64) -> XNonce {
    let mut nonce = [0; 24];
    nonce[..8].copy_from_slice(&counter.to_le_bytes());
    XNonce::from(nonce)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Cipher, Cipher) {
        let (c, s) = (KeyPair::new().unwrap(), KeyPair::new().unwrap());
        let (c_pub, s_pub) = (c.public, s.public);
        (Cipher::new(c, s_pub, true).unwrap(), Cipher::new(s, c_pub, false).unwrap())
    }

    #[test]
    fn both_directions() {
        let (mut client, mut server) = pair();
        for i in 0..3u8 {
            let sealed = client.seal(&[i; 5]);
            assert_ne!(&sealed[..5], &[i; 5]);
            assert_eq!(server.open(&sealed).unwrap(), [i; 5]);
            assert_eq!(client.open(&server.seal(b"back")).unwrap(), b"back");
        }
    }

    #[test]
    fn rejects_changes_replays_and_weak_keys() {
        let (mut client, mut server) = pair();
        let mut sealed = client.seal(b"move");
        let copy = sealed.clone();
        sealed[0] ^= 1;
        assert!(server.open(&sealed).is_none());
        assert!(server.open(&copy).is_some());
        assert!(server.open(&copy).is_none(), "a replayed message must not decrypt");
        assert!(Cipher::new(KeyPair::new().unwrap(), [0; 32], true).is_none());
    }
}
