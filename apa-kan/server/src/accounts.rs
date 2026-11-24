//! Account host: `account/<id>` and `admin/roles` streams on the same ledger
//! and envelope as games.
//!
//! - Every account change is a signed transaction (`account.*`); reads are
//!   views folded from the stream. The fold is the key directory for game
//!   admission ([`AccountKeys`]).
//! - Email recovery: after committing a client's `account.recovery_request`,
//!   the host mints a token, commits an operator-signed
//!   `account.recovery_challenge` carrying only the token's hash and the
//!   cooldown end, and mails the token. The pending key then submits
//!   `account.recovery_confirm { token }` and, after the cooldown,
//!   `account.recovery_finish`.
//! - Registration projects into the account store (starter tiles, default
//!   rating) after commit; the store is a projection, not an authority.
//! - Raw account transactions are not served: `account.set_email` carries the
//!   address. Views show only `email_set`.

use std::sync::Arc;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::{
	AccountId, AuthorizedKeyStatus, RecoveryKind, Seed, COOLDOWN_EMAIL_LINK_SECS,
	COOLDOWN_RECOVERY_SECS,
};
use apa_kan_ledger::LedgerStore;
use apa_kan_store::Clock;
use apa_kan_tx::account::recovery_token_hash;
use apa_kan_tx::{
	account_id_of, account_stream, admit, AccountRules, AccountState, KeyStatus, RolesState,
	SignedTx, TxError, TxPayload, TxReceipt, TxResponse, TxSigner, ROLES_STREAM,
};
use apa_kan_view::{AccountView, RolesView};
use async_trait::async_trait;

use crate::games::KeyDirectory;
use crate::mailer::Mailer;
use crate::streams::{host_err, verify, Machine, Streams};

/// [`Machine`] for `account/<id>` streams.
pub struct AccountMachine(pub AccountRules);

impl Machine for AccountMachine {
	type State = AccountState;

	fn head(s: &AccountState) -> (u64, &str) {
		(s.next_seq, s.head.as_str())
	}

	fn apply(
		&self,
		state: Option<&AccountState>,
		tx: &SignedTx,
		id: &[u8; 32],
		now: Option<i64>,
	) -> Result<(AccountState, TxReceipt), TxError> {
		AccountState::apply(&self.0, state, tx, id, now)
	}
}

/// [`Machine`] for `admin/roles`.
pub struct RolesMachine(pub AccountRules);

impl Machine for RolesMachine {
	type State = RolesState;

	fn head(s: &RolesState) -> (u64, &str) {
		(s.next_seq, s.head.as_str())
	}

	fn apply(
		&self,
		state: Option<&RolesState>,
		tx: &SignedTx,
		id: &[u8; 32],
		_now: Option<i64>,
	) -> Result<(RolesState, TxReceipt), TxError> {
		RolesState::apply(&self.0, state, tx, id)
	}
}

/// Side effects of committed account transactions outside the ledger.
#[async_trait]
pub trait Projection: Send + Sync {
	/// `account.register` committed for `id`.
	async fn registered(&self, id: AccountId, genesis_vk: VerifyingKeyBytes) -> Result<(), String>;
}

pub struct NoProjection;

#[async_trait]
impl Projection for NoProjection {
	async fn registered(&self, _: AccountId, _: VerifyingKeyBytes) -> Result<(), String> {
		Ok(())
	}
}

/// Operator key from the environment: `APAKAN_OPERATOR_SEED` (64 hex), else
/// the seed file `APAKAN_OPERATOR_SEED_FILE` (default
/// `$TMP/apa-kan-operator.seed`), created with a random seed on first run.
/// The operator account must stay stable for a chain: its challenges are in
/// the ledger and only replay under the same operator.
pub fn operator_from_env() -> Result<TxSigner, String> {
	let parse = |s: &str| -> Option<Seed> {
		let b: [u8; 32] = hex::decode(s.trim()).ok()?.try_into().ok()?;
		Some(Seed::from_bytes(b))
	};
	if let Some(v) = std::env::var("APAKAN_OPERATOR_SEED").ok().filter(|v| !v.trim().is_empty()) {
		return parse(&v)
			.map(|s| TxSigner::from_seed(&s))
			.ok_or_else(|| "APAKAN_OPERATOR_SEED must be 64 hex chars".into());
	}
	let path = std::env::var_os("APAKAN_OPERATOR_SEED_FILE")
		.map(std::path::PathBuf::from)
		.unwrap_or_else(|| std::env::temp_dir().join("apa-kan-operator.seed"));
	if let Ok(text) = std::fs::read_to_string(&path) {
		return parse(&text)
			.map(|s| TxSigner::from_seed(&s))
			.ok_or_else(|| format!("{} is not a 64-hex seed", path.display()));
	}
	let seed = Seed::random();
	std::fs::write(&path, hex::encode(seed.0)).map_err(|e| format!("{}: {e}", path.display()))?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
	}
	Ok(TxSigner::from_seed(&seed))
}

fn cooldown(kind: RecoveryKind) -> i64 {
	match kind {
		RecoveryKind::FullRecovery => COOLDOWN_RECOVERY_SECS,
		RecoveryKind::EmailLinkApproval => COOLDOWN_EMAIL_LINK_SECS,
	}
}

pub struct AccountHost {
	accounts: Streams<AccountMachine>,
	roles: Streams<RolesMachine>,
	rules: AccountRules,
	operator: Option<TxSigner>,
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
	projection: Arc<dyn Projection>,
}

impl AccountHost {
	pub fn new(
		ledger: Arc<dyn LedgerStore>,
		chain: impl Into<String>,
		operator: Option<TxSigner>,
		clock: Arc<dyn Clock>,
		mailer: Arc<dyn Mailer>,
	) -> Self {
		let chain = chain.into();
		let rules = AccountRules { operator: operator.as_ref().map(TxSigner::account) };
		Self {
			accounts: Streams::new(ledger.clone(), chain.clone(), AccountMachine(rules.clone())),
			roles: Streams::new(ledger, chain, RolesMachine(rules.clone())),
			rules,
			operator,
			clock,
			mailer,
			projection: Arc::new(NoProjection),
		}
	}

	pub fn with_projection(mut self, projection: Arc<dyn Projection>) -> Self {
		self.projection = projection;
		self
	}

	pub fn rules(&self) -> &AccountRules {
		&self.rules
	}

	pub fn chain(&self) -> &str {
		self.accounts.chain()
	}

	/// Hex operator account, if this chain has one.
	pub fn operator_hex(&self) -> Option<String> {
		self.rules.operator.map(|a| a.to_hex())
	}

	pub async fn state(&self, id: &AccountId) -> Result<Option<AccountState>, TxError> {
		self.accounts.current(&account_stream(id)).await
	}

	pub async fn view(&self, id: &AccountId) -> Result<AccountView, TxError> {
		self.state(id).await?.as_ref().map(AccountState::view).ok_or(TxError::NotFound)
	}

	pub async fn roles(&self) -> Result<Option<RolesState>, TxError> {
		self.roles.current(ROLES_STREAM).await
	}

	pub async fn roles_view(&self) -> Result<RolesView, TxError> {
		Ok(RolesState::view(self.roles().await?.as_ref()))
	}

	/// Status of `vk` for `account` per the account fold.
	pub async fn key_status(
		&self,
		account: &AccountId,
		vk: &VerifyingKeyBytes,
	) -> Result<KeyStatus, TxError> {
		Ok(match self.state(account).await? {
			None => KeyStatus::UnknownAccount,
			Some(s) => match s.key_status(vk) {
				Some(AuthorizedKeyStatus::Active) => KeyStatus::Active,
				Some(_) => KeyStatus::Inactive,
				None => KeyStatus::NotOnAccount,
			},
		})
	}

	/// Commit one `account/<id>` transaction, then run its side effects.
	pub async fn submit(&self, tx: SignedTx) -> Result<TxResponse<AccountView>, TxError> {
		let account = account_id_of(&tx.body.stream)
			.ok_or_else(|| TxError::UnsupportedStream { stream: tx.body.stream.clone() })?;
		let (tx, id) = verify(tx, self.chain()).await?;
		let now = self.clock.now_unix();
		let (mut state, receipt) = self.accounts.commit(&tx, &id, Some(now)).await?;
		match &tx.body.payload {
			TxPayload::AccountRegister { .. } => {
				if let Err(e) = self.projection.registered(account, tx.body.signer_vk.clone()).await
				{
					tracing::warn!(account = %account.to_hex(), error = %e, "register projection failed");
				}
			}
			TxPayload::AccountRecoveryRequest { kind, .. } => {
				match self.challenge(&state, *kind, now).await {
					Ok(next) => state = next,
					Err(e) => {
						tracing::warn!(account = %account.to_hex(), error = %e, "recovery challenge failed");
					}
				}
			}
			_ => {}
		}
		Ok(TxResponse { receipt, view: state.view() })
	}

	/// Mint and mail a recovery token; commit its hash as the operator.
	async fn challenge(
		&self,
		state: &AccountState,
		kind: RecoveryKind,
		now: i64,
	) -> Result<AccountState, TxError> {
		let op = self.operator.as_ref().ok_or_else(|| host_err("no operator key"))?;
		let email = state.email.clone().ok_or_else(|| host_err("no email on file"))?;
		let token = format!("tok_{}", hex::encode(rand::random::<[u8; 16]>()));
		let tx = op.sign(
			self.chain(),
			&account_stream(&state.id),
			state.next_seq,
			&state.head,
			TxPayload::AccountRecoveryChallenge {
				token_hash: recovery_token_hash(&token),
				usable_at_unix: now + cooldown(kind),
			},
		)?;
		let id = tx.id()?;
		let (next, _) = self.accounts.commit(&tx, &id, Some(now)).await?;
		self.mailer.send_recovery_token(&email, &state.id.to_hex(), &token);
		Ok(next)
	}

	/// Commit one `admin/roles` transaction (operator or `admin` signer).
	pub async fn submit_admin(&self, tx: SignedTx) -> Result<TxResponse<RolesView>, TxError> {
		if tx.body.stream != ROLES_STREAM {
			return Err(TxError::UnsupportedStream { stream: tx.body.stream.clone() });
		}
		let key = self.key_status(&tx.body.signer, &tx.body.signer_vk).await?;
		let chain = self.chain().to_string();
		let (tx, id) =
			tokio::task::spawn_blocking(move || admit(&tx, &chain, key).map(|id| (tx, id)))
				.await
				.map_err(|e| host_err(e.to_string()))??;
		let (next, receipt) = self.roles.commit(&tx, &id, Some(self.clock.now_unix())).await?;
		Ok(TxResponse { receipt, view: RolesState::view(Some(&next)) })
	}

	/// Forget cached folds (tests).
	pub async fn clear_cache(&self) {
		self.accounts.clear_cache().await;
		self.roles.clear_cache().await;
	}
}

/// Game admission's key directory: the account folds.
pub struct AccountKeys(pub Arc<AccountHost>);

#[async_trait]
impl KeyDirectory for AccountKeys {
	async fn status(
		&self,
		account: &AccountId,
		vk: &VerifyingKeyBytes,
	) -> Result<KeyStatus, String> {
		self.0.key_status(account, vk).await.map_err(|e| e.to_string())
	}
}
