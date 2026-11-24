//! Encrypted UDP move previews (helpers only; no transport is built).
//!
//! Previews are an early read of the opponent's move; the signed `game.move`
//! over HTTP stays authoritative. Each seat puts an ephemeral X25519 public
//! key (`preview_pk`) in its signed `game.create` / `game.join`, so the key
//! exchange is authenticated by ML-DSA for free and needs no extra round trip.
//!
//! - Shared secret: X25519(my secret, their `preview_pk`).
//! - Keys: HKDF-SHA256(ikm = shared secret, salt = SHA-256(`PREVIEW_DOMAIN` ‖
//!   create tx id ‖ join tx id)), one 32-byte key per sending seat
//!   (info = `PREVIEW_DOMAIN` ‖ `"/seat"` ‖ seat), so the two directions never
//!   share a (key, nonce).
//! - Packet: 12-byte big-endian counter (the ChaCha20-Poly1305 nonce) ‖
//!   ciphertext ‖ 16-byte tag; 28 bytes of overhead. The AAD is the stream id.
//! - Receivers keep the highest counter seen and reject anything not above it
//!   (stale, replayed, or reordered packets are dropped; previews are
//!   disposable).
//!
//! Hybrid ML-KEM + X25519 is the upgrade path if previews need post-quantum
//! confidentiality; the payload field would grow, the rest stays.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use apa_kan_view::Seat;

/// Domain for preview key derivation.
pub const PREVIEW_DOMAIN: &[u8] = b"apa-kan/preview/v1";
/// Domain for deriving a per-game preview secret from a device secret.
pub const PREVIEW_SECRET_DOMAIN: &[u8] = b"apa-kan/preview-secret/v1";
/// Counter (nonce) bytes at the front of a packet.
pub const COUNTER_LEN: usize = 12;
/// AEAD tag bytes.
pub const TAG_LEN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PreviewError {
	#[error("preview key must be 64 lowercase hex")]
	BadKey,
	#[error("packet too short")]
	Short,
	#[error("stale or replayed counter")]
	Stale,
	#[error("packet failed authentication")]
	Auth,
	#[error("counter exhausted")]
	Exhausted,
}

/// Deterministic per-game X25519 secret from a device secret, so a client can
/// recover it after a reload (like [`crate::deal_salt`]). Any 32 random bytes
/// work; this is a client convenience.
pub fn preview_secret(device_secret: &[u8; 32], stream: &str) -> [u8; 32] {
	let mut h = Sha256::new();
	h.update(PREVIEW_SECRET_DOMAIN);
	h.update(device_secret);
	h.update(stream.as_bytes());
	h.finalize().into()
}

/// Hex public key for `secret`, as put in `preview_pk`.
pub fn preview_public(secret: &[u8; 32]) -> String {
	hex::encode(PublicKey::from(&StaticSecret::from(*secret)).as_bytes())
}

/// Parse a `preview_pk` field (64 lowercase hex).
pub fn parse_preview_pk(s: &str) -> Result<[u8; 32], PreviewError> {
	if s.len() != 64 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
		return Err(PreviewError::BadKey);
	}
	hex::decode(s).ok().and_then(|v| v.try_into().ok()).ok_or(PreviewError::BadKey)
}

/// Both directions' keys for one game, from this seat's view.
pub struct PreviewSession {
	stream: String,
	send: ChaCha20Poly1305,
	recv: ChaCha20Poly1305,
	next_send: u128,
	last_recv: Option<u128>,
}

fn seat_key(hk: &Hkdf<Sha256>, seat: Seat) -> ChaCha20Poly1305 {
	let mut info = PREVIEW_DOMAIN.to_vec();
	info.extend_from_slice(b"/seat");
	info.push(seat);
	let mut key = [0u8; 32];
	// 32 bytes is far below HKDF-SHA256's output limit.
	let _ = hk.expand(&info, &mut key);
	ChaCha20Poly1305::new(Key::from_slice(&key))
}

impl PreviewSession {
	/// `my_seat`'s session for `stream`, from its secret, the peer's
	/// `preview_pk`, and the ids of the create and join transactions.
	pub fn new(
		stream: &str,
		my_seat: Seat,
		my_secret: &[u8; 32],
		peer_pk: &[u8; 32],
		create_id: &[u8; 32],
		join_id: &[u8; 32],
	) -> Self {
		let shared = StaticSecret::from(*my_secret).diffie_hellman(&PublicKey::from(*peer_pk));
		let mut t = Sha256::new();
		t.update(PREVIEW_DOMAIN);
		t.update(create_id);
		t.update(join_id);
		let salt: [u8; 32] = t.finalize().into();
		let hk = Hkdf::<Sha256>::new(Some(&salt), shared.as_bytes());
		let peer_seat = if my_seat == 1 { 2 } else { 1 };
		Self {
			stream: stream.to_string(),
			send: seat_key(&hk, my_seat),
			recv: seat_key(&hk, peer_seat),
			next_send: 0,
			last_recv: None,
		}
	}

	/// Seal `msg` (a compact preview: seq, action, signed tx hash, …).
	pub fn seal(&mut self, msg: &[u8]) -> Result<Vec<u8>, PreviewError> {
		let ctr = self.next_send;
		if ctr >= 1u128 << 96 {
			return Err(PreviewError::Exhausted);
		}
		self.next_send += 1;
		let nonce_bytes: [u8; COUNTER_LEN] = ctr.to_be_bytes()[16 - COUNTER_LEN..]
			.try_into()
			.map_err(|_| PreviewError::Exhausted)?;
		let ct = self
			.send
			.encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg, aad: self.stream.as_bytes() })
			.map_err(|_| PreviewError::Auth)?;
		let mut out = Vec::with_capacity(COUNTER_LEN + ct.len());
		out.extend_from_slice(&nonce_bytes);
		out.extend_from_slice(&ct);
		Ok(out)
	}

	/// Open a packet from the peer; rejects counters not above the last
	/// accepted one. A failed packet does not advance the counter.
	pub fn open(&mut self, packet: &[u8]) -> Result<Vec<u8>, PreviewError> {
		if packet.len() < COUNTER_LEN + TAG_LEN {
			return Err(PreviewError::Short);
		}
		let (nonce_bytes, ct) = packet.split_at(COUNTER_LEN);
		let mut wide = [0u8; 16];
		wide[16 - COUNTER_LEN..].copy_from_slice(nonce_bytes);
		let ctr = u128::from_be_bytes(wide);
		if self.last_recv.is_some_and(|last| ctr <= last) {
			return Err(PreviewError::Stale);
		}
		let msg = self
			.recv
			.decrypt(
				Nonce::from_slice(nonce_bytes),
				Payload { msg: ct, aad: self.stream.as_bytes() },
			)
			.map_err(|_| PreviewError::Auth)?;
		self.last_recv = Some(ctr);
		Ok(msg)
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	fn pair() -> (PreviewSession, PreviewSession) {
		let (a, b) = ([1u8; 32], [2u8; 32]);
		let pk = |s: &[u8; 32]| parse_preview_pk(&preview_public(s)).unwrap();
		let (c, j) = ([7u8; 32], [8u8; 32]);
		(
			PreviewSession::new("game/x", 1, &a, &pk(&b), &c, &j),
			PreviewSession::new("game/x", 2, &b, &pk(&a), &c, &j),
		)
	}

	#[test]
	fn round_trip_both_directions() {
		let (mut one, mut two) = pair();
		let p = one.seal(b"move 3").unwrap();
		assert_eq!(p.len(), 6 + COUNTER_LEN + TAG_LEN);
		assert_eq!(two.open(&p).unwrap(), b"move 3");
		let q = two.seal(b"move 4").unwrap();
		assert_eq!(one.open(&q).unwrap(), b"move 4");
		// A seat can't open its own packets (directional keys).
		let mine = one.seal(b"x").unwrap();
		assert_eq!(one.open(&mine), Err(PreviewError::Auth));
	}

	#[test]
	fn rejects_stale_replayed_and_tampered() {
		let (mut one, mut two) = pair();
		let p0 = one.seal(b"a").unwrap();
		let p1 = one.seal(b"b").unwrap();
		let p2 = one.seal(b"c").unwrap();
		assert!(two.open(&p1).is_ok());
		assert_eq!(two.open(&p1), Err(PreviewError::Stale));
		assert_eq!(two.open(&p0), Err(PreviewError::Stale));
		let mut bad = p2.clone();
		let last = bad.len() - 1;
		bad[last] ^= 1;
		assert_eq!(two.open(&bad), Err(PreviewError::Auth));
		assert!(two.open(&p2).is_ok());
		assert_eq!(two.open(&[0; 5]), Err(PreviewError::Short));
	}

	#[test]
	fn bound_to_game_and_transcript() {
		let (mut one, _) = pair();
		let p = one.seal(b"a").unwrap();
		let pk = parse_preview_pk(&preview_public(&[1; 32])).unwrap();
		let mut other_join = PreviewSession::new("game/x", 2, &[2; 32], &pk, &[7; 32], &[9; 32]);
		assert_eq!(other_join.open(&p), Err(PreviewError::Auth));
		let mut other_stream = PreviewSession::new("game/y", 2, &[2; 32], &pk, &[7; 32], &[8; 32]);
		assert_eq!(other_stream.open(&p), Err(PreviewError::Auth));
		assert_eq!(parse_preview_pk("AB"), Err(PreviewError::BadKey));
	}
}
