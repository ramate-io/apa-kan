//! Account identity, ML-DSA signing, and link/recovery validation (non-Bevy, no I/O).

pub mod canonical;
pub mod crypto;
pub mod envelope;
pub mod id;
pub mod link;
pub mod recovery;
pub mod types;

pub use crypto::{sign_bytes, verify_bytes, CryptoError, Param, Seed, SignatureBytes, VerifyingKeyBytes};
pub use id::AccountId;
pub use link::{verify_link_approval, LinkApproval, LinkRequest, LinkError};
pub use recovery::{
	COOLDOWN_RECOVERY_SECS, COOLDOWN_EMAIL_LINK_SECS, RecoveryKind, RecoveryPending,
};
pub use types::{AuthorizedKeyStatus, KeyLabel};