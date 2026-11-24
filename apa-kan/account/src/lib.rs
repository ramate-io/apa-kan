//! Account identity, ML-DSA signing, and link/recovery validation (non-Bevy, no I/O).

pub mod canonical;
pub mod crypto;
pub mod envelope;
pub mod id;
pub mod keystore;
pub mod link;
pub mod param;
pub mod passkey;
pub mod phrase;
pub mod recovery;
pub mod types;

pub use crypto::{
	sign_bytes, verify_bytes, CryptoError, Param, Seed, SignatureBytes, VerifyingKeyBytes,
};
pub use id::AccountId;
pub use keystore::{
	KeyStore, KeyStoreError, MemoryKeyStore, PassphraseKeyStore, PassphraseKeystoreBackend,
	PassphraseWrappedSeed, WrappedSeedBlob,
};
pub use link::{verify_link_approval, LinkApproval, LinkError, LinkRequest};
pub use param::AccountMlDsaParams;
pub use passkey::{access_key, prf_input, wrap_seed, PasskeyError, PasskeyWrappedSeed};
pub use phrase::{phrase_to_seed, seed_to_phrase, PhraseError};
pub use recovery::{
	RecoveryKind, RecoveryPending, COOLDOWN_EMAIL_LINK_SECS, COOLDOWN_RECOVERY_SECS,
};
pub use types::{AuthorizedKeyStatus, KeyLabel};
