//! Recovery phrase: the 32-byte account seed as 24 BIP39 English words
//! (256 bits of entropy + an 8-bit SHA-256 checksum).
//!
//! The words encode the seed itself (BIP39 "entropy"), not a PBKDF2-stretched
//! BIP39 seed: phrase → seed → ML-DSA key is a bijection, so the phrase
//! restores exactly the key a device had. It is the fallback when no passkey
//! is available (or the passkey is lost).

use crate::crypto::Seed;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PhraseError {
	#[error("recovery phrase: {0}")]
	Invalid(String),
	#[error("recovery phrase must be 24 words, got {0}")]
	WrongLength(usize),
}

/// Number of words in a recovery phrase.
pub const PHRASE_WORDS: usize = 24;

/// The seed's recovery phrase (24 lowercase words, space-separated).
pub fn seed_to_phrase(seed: &Seed) -> String {
	// 32 bytes is a valid BIP39 entropy length, so this cannot fail.
	bip39::Mnemonic::from_entropy(&seed.0)
		.map(|m| m.to_string())
		.unwrap_or_default()
}

/// Parse a phrase back to its seed. Case, extra whitespace, and line breaks
/// are ignored; the checksum must match.
pub fn phrase_to_seed(phrase: &str) -> Result<Seed, PhraseError> {
	let normalized = phrase.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>();
	if normalized.len() != PHRASE_WORDS {
		return Err(PhraseError::WrongLength(normalized.len()));
	}
	let m = bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &normalized.join(" "))
		.map_err(|e| PhraseError::Invalid(e.to_string()))?;
	let bytes: [u8; 32] = m
		.to_entropy()
		.try_into()
		.map_err(|_| PhraseError::Invalid("unexpected entropy length".into()))?;
	Ok(Seed::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn round_trip_and_checksum() {
		let seed = Seed::from_bytes([7; 32]);
		let phrase = seed_to_phrase(&seed);
		assert_eq!(phrase.split(' ').count(), 24);
		assert_eq!(phrase_to_seed(&phrase).unwrap().0, seed.0);
		// Sloppy input still parses.
		let sloppy = format!("  {}\n", phrase.to_uppercase().replace(' ', "   "));
		assert_eq!(phrase_to_seed(&sloppy).unwrap().0, seed.0);
		// Swapping two words breaks the checksum (or the parse).
		let mut words: Vec<&str> = phrase.split(' ').collect();
		words.swap(0, 23);
		if words[0] != words[23] {
			assert!(phrase_to_seed(&words.join(" ")).is_err());
		}
		assert_eq!(phrase_to_seed("abandon abandon").err(), Some(PhraseError::WrongLength(2)));
	}

	#[test]
	fn known_vector() {
		// BIP39 test vector: 32 zero bytes.
		let p = seed_to_phrase(&Seed::from_bytes([0; 32]));
		assert!(p.starts_with("abandon abandon"));
		assert!(p.ends_with("abandon art"));
	}
}
