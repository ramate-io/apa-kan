//! Protecting the account key: passkey wrap (web), recovery phrase, and the
//! prompt after a guest's first game.
//!
//! **Passkey** (web, WebAuthn PRF): `protect` creates a discoverable passkey
//! for the account, evaluates its PRF on [`apa_kan_account::prf_input`],
//! wraps the seed ([`apa_kan_account::wrap_seed`]: HKDF-SHA256 → AES-256-GCM),
//! stores the wrapped seed in IndexedDB, drops the plaintext from
//! `localStorage`, and, when a server is configured, registers the account
//! if needed and commits `account.set_passkey_backup`. The passkey syncs via
//! the platform password manager, so on a new device (or after clearing the
//! browser) `restore_with_passkey` lets the passkey name the account
//! (`userHandle`) and credential, fetches the wrapped seed from IndexedDB or
//! `POST /v1/accounts/{id}/passkeys/{credential}` (which serves it only for
//! the PRF-derived [`apa_kan_account::access_key`]), and unwraps it.
//!
//! **No PRF** (older browsers / authenticators): the key stays in plaintext
//! local storage and the user is shown the recovery phrase instead.
//!
//! **Recovery phrase:** 24 BIP39 words for the seed, shown and entered with
//! the browser's `prompt()` (Bevy text is not selectable); on desktop it is
//! printed and `APAKAN_RECOVERY_PHRASE` restores it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::host::HostConfig;
use crate::identity::{self, Protection};

/// `localStorage` flag: the post-game passkey prompt was shown.
pub const PROMPTED_KEY: &str = "apakan.passkey.prompted";

/// Lobby text after a guest's first finished game.
pub const FIRST_GAME_PROMPT: &str = "Keep this account: Settings → Protect with passkey (or save the recovery phrase). Clearing the browser otherwise loses it.";

static REVISION: AtomicU64 = AtomicU64::new(0);

fn notices() -> &'static Mutex<VecDeque<String>> {
	static Q: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
	&Q
}

/// Report an async result to the lobby and bump [`revision`].
pub fn notify(msg: impl Into<String>) {
	if let Ok(mut q) = notices().lock() {
		q.push_back(msg.into());
	}
	REVISION.fetch_add(1, Ordering::Relaxed);
}

/// Pending notices from async passkey work.
pub fn drain_notices() -> Vec<String> {
	notices().lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
}

/// Changes whenever the identity may have changed (for UI rebuilds).
pub fn revision() -> u64 {
	REVISION.load(Ordering::Relaxed)
}

/// Can this browser try a passkey at all? (PRF support is only known after
/// the authenticator answers.)
pub fn passkey_supported() -> bool {
	#[cfg(target_arch = "wasm32")]
	{
		web::supported()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		false
	}
}

/// One line for Settings: account, protection, lock state.
pub fn status_line() -> String {
	let acct = identity::account_hex();
	let short = acct.get(..12).unwrap_or(&acct);
	match (identity::protection(), identity::is_locked()) {
		(Protection::Passkey { .. }, true) => {
			format!("Account {short}… is protected by a passkey and locked.")
		}
		(Protection::Passkey { .. }, false) => {
			format!("Account {short}… is protected by a passkey (unlocked).")
		}
		(Protection::Plain, _) => format!(
			"Account {short}… is stored unprotected in this browser. Protect it with a passkey or save the recovery phrase."
		),
	}
}

/// Should the post-game prompt show? (Web only: an unprotected browser key
/// that was not prompted before. Desktop keys live in a file.)
pub fn should_prompt() -> bool {
	cfg!(target_arch = "wasm32")
		&& identity::protection() == Protection::Plain
		&& identity::local_get(PROMPTED_KEY).is_none()
}

pub fn mark_prompted() {
	identity::local_set(PROMPTED_KEY, Some("1"));
}

/// Show the recovery phrase (selectable, via `prompt()` on web).
pub fn show_phrase() -> String {
	let Some(seed) = identity::seed() else {
		return "Unlock the account first.".into();
	};
	let phrase = apa_kan_account::seed_to_phrase(&seed);
	#[cfg(target_arch = "wasm32")]
	{
		if let Some(w) = web_sys::window() {
			let _ = w.prompt_with_message_and_default(
				"Your recovery phrase. Copy these 24 words and keep them somewhere safe; anyone with them controls the account.",
				&phrase,
			);
		}
		"Recovery phrase shown. Keep it offline.".into()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		println!("Apa Kan recovery phrase: {phrase}");
		"Recovery phrase printed to the terminal. Restore with APAKAN_RECOVERY_PHRASE.".into()
	}
}

/// Ask for a phrase and switch this device to that key (web).
/// After a successful restore the user is offered passkey protection right
/// away (when the browser has passkeys).
pub fn restore_from_phrase(host: &HostConfig) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		let Some(w) = web_sys::window() else {
			return "No window.".into();
		};
		let entered = w
			.prompt_with_message(
				"Enter your 24-word recovery phrase. This replaces the key in this browser.",
			)
			.ok()
			.flatten()
			.unwrap_or_default();
		if entered.trim().is_empty() {
			return String::new();
		}
		match apa_kan_account::phrase_to_seed(&entered) {
			Ok(seed) => {
				identity::install(seed, None, Protection::Plain);
				REVISION.fetch_add(1, Ordering::Relaxed);
				let offer = web::supported()
					&& w.confirm_with_message(
						"Account restored. Protect it with a passkey now, so clearing the browser can't lose it?",
					)
					.unwrap_or(false);
				if offer {
					protect(host)
				} else {
					"Account restored from the recovery phrase. Protect it with a passkey (Settings).".into()
				}
			}
			Err(e) => format!("{e}"),
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = host;
		"Set APAKAN_RECOVERY_PHRASE and restart to restore on desktop.".into()
	}
}

/// Protect the key with a new passkey (web). Results arrive as notices.
pub fn protect(host: &HostConfig) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		if !web::supported() {
			return format!("This browser has no passkeys. {}", show_phrase());
		}
		let host = host.clone();
		ehttp::spawn_future(async move {
			match web::protect(&host).await {
				Ok(msg) => notify(msg),
				Err(web::Failure::NoPrf) => notify(format!(
					"This passkey can't protect keys (no PRF support), so the key stays in this browser. {}",
					show_phrase()
				)),
				Err(web::Failure::Other(e)) => notify(format!("Passkey setup failed: {e}")),
			}
		});
		"Follow your browser's passkey prompt…".into()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = host;
		"Passkeys are web-only; save the recovery phrase instead.".into()
	}
}

/// Unlock a locked key with its passkey (web).
pub fn unlock(host: &HostConfig) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		let host = host.clone();
		ehttp::spawn_future(async move {
			match web::unlock(&host).await {
				Ok(()) => notify("Unlocked."),
				Err(e) => notify(format!("Unlock failed: {}", e.message())),
			}
		});
		"Follow your browser's passkey prompt…".into()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = host;
		"Passkeys are web-only.".into()
	}
}

/// Restore an account on this device from a synced passkey (web).
pub fn restore_with_passkey(host: &HostConfig) -> String {
	#[cfg(target_arch = "wasm32")]
	{
		let host = host.clone();
		ehttp::spawn_future(async move {
			match web::restore(&host).await {
				Ok(acct) => notify(format!(
					"Restored account {}… from your passkey.",
					acct.get(..12).unwrap_or(&acct)
				)),
				Err(e) => notify(format!("Restore failed: {}", e.message())),
			}
		});
		"Pick your Apa Kan passkey…".into()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = host;
		"Passkeys are web-only.".into()
	}
}

#[cfg(target_arch = "wasm32")]
mod web {
	//! WebAuthn PRF, IndexedDB, and the server calls, all async in the
	//! browser. Options objects are built with `Reflect` (web-sys has no
	//! typed PRF extension yet).

	use apa_kan_account::{prf_input, wrap_seed, AccountId, PasskeyWrappedSeed};
	use apa_kan_tx::{account_stream, TxPayload, GENESIS_PREV};
	use js_sys::{Array, Object, Reflect, Uint8Array};
	use rand::RngCore;
	use wasm_bindgen::{JsCast, JsValue};
	use wasm_bindgen_futures::JsFuture;

	use super::*;

	pub enum Failure {
		NoPrf,
		Other(String),
	}

	impl Failure {
		pub fn message(&self) -> String {
			match self {
				Failure::NoPrf => "this passkey has no PRF support".into(),
				Failure::Other(e) => e.clone(),
			}
		}
	}

	impl From<String> for Failure {
		fn from(s: String) -> Self {
			Failure::Other(s)
		}
	}

	impl From<&str> for Failure {
		fn from(s: &str) -> Self {
			Failure::Other(s.to_string())
		}
	}

	fn js_err(v: JsValue) -> String {
		v.as_string()
			.or_else(|| Reflect::get(&v, &"message".into()).ok().and_then(|m| m.as_string()))
			.unwrap_or_else(|| format!("{v:?}"))
	}

	fn set(obj: &Object, key: &str, value: impl Into<JsValue>) {
		let _ = Reflect::set(obj, &key.into(), &value.into());
	}

	fn get(obj: &JsValue, key: &str) -> JsValue {
		Reflect::get(obj, &key.into()).unwrap_or(JsValue::UNDEFINED)
	}

	fn bytes(v: &JsValue) -> Option<Vec<u8>> {
		if v.is_undefined() || v.is_null() {
			return None;
		}
		Some(Uint8Array::new(v).to_vec())
	}

	fn random<const N: usize>() -> [u8; N] {
		let mut b = [0u8; N];
		rand::thread_rng().fill_bytes(&mut b);
		b
	}

	pub fn supported() -> bool {
		web_sys::window()
			.map(|w| Reflect::has(&w, &"PublicKeyCredential".into()).unwrap_or(false))
			.unwrap_or(false)
	}

	fn prf_ext() -> Object {
		let first = Uint8Array::from(prf_input().as_slice());
		let eval = Object::new();
		set(&eval, "first", first);
		let prf = Object::new();
		set(&prf, "eval", eval);
		let ext = Object::new();
		set(&ext, "prf", prf);
		ext
	}

	fn prf_result(cred: &JsValue) -> Result<Option<[u8; 32]>, Failure> {
		let f = get(cred, "getClientExtensionResults");
		let results = f
			.dyn_ref::<js_sys::Function>()
			.and_then(|f| f.call0(cred).ok())
			.unwrap_or(JsValue::UNDEFINED);
		let prf = get(&results, "prf");
		if prf.is_undefined() {
			return Err(Failure::NoPrf);
		}
		let first = get(&get(&prf, "results"), "first");
		match bytes(&first) {
			Some(b) => b
				.try_into()
				.map(Some)
				.map_err(|_| Failure::Other("PRF output is not 32 bytes".into())),
			None if get(&prf, "enabled").as_bool() == Some(false) => Err(Failure::NoPrf),
			None => Ok(None),
		}
	}

	fn credentials() -> Result<web_sys::CredentialsContainer, String> {
		Ok(web_sys::window().ok_or("no window")?.navigator().credentials())
	}

	struct Created {
		id: Vec<u8>,
		prf: Option<[u8; 32]>,
	}

	async fn create(account: &AccountId) -> Result<Created, Failure> {
		let short = account.to_hex()[..8].to_string();
		let pk = Object::new();
		set(&pk, "challenge", Uint8Array::from(random::<32>().as_slice()));
		let rp = Object::new();
		set(&rp, "name", "Apa Kan");
		set(&pk, "rp", rp);
		let user = Object::new();
		set(&user, "id", Uint8Array::from(account.as_bytes().as_slice()));
		set(&user, "name", format!("apa-kan-{short}"));
		set(&user, "displayName", format!("Apa Kan {short}"));
		set(&pk, "user", user);
		let params = Array::new();
		for alg in [-8, -7, -257] {
			let p = Object::new();
			set(&p, "type", "public-key");
			set(&p, "alg", alg);
			params.push(&p);
		}
		set(&pk, "pubKeyCredParams", params);
		let sel = Object::new();
		set(&sel, "residentKey", "required");
		set(&sel, "requireResidentKey", true);
		set(&sel, "userVerification", "required");
		set(&pk, "authenticatorSelection", sel);
		set(&pk, "extensions", prf_ext());
		let opts = Object::new();
		set(&opts, "publicKey", pk);
		let promise = credentials()?
			.create_with_options(opts.unchecked_ref())
			.map_err(|e| Failure::Other(js_err(e)))?;
		let cred = JsFuture::from(promise).await.map_err(|e| Failure::Other(js_err(e)))?;
		let id = bytes(&get(&cred, "rawId")).ok_or("no credential id")?;
		Ok(Created { id, prf: prf_result(&cred)? })
	}

	struct Asserted {
		id: Vec<u8>,
		user: Option<Vec<u8>>,
		prf: [u8; 32],
	}

	async fn assert(allow: Option<&[u8]>) -> Result<Asserted, Failure> {
		let pk = Object::new();
		set(&pk, "challenge", Uint8Array::from(random::<32>().as_slice()));
		let list = Array::new();
		if let Some(id) = allow {
			let d = Object::new();
			set(&d, "type", "public-key");
			set(&d, "id", Uint8Array::from(id));
			list.push(&d);
		}
		set(&pk, "allowCredentials", list);
		set(&pk, "userVerification", "required");
		set(&pk, "extensions", prf_ext());
		let opts = Object::new();
		set(&opts, "publicKey", pk);
		let promise = credentials()?
			.get_with_options(opts.unchecked_ref())
			.map_err(|e| Failure::Other(js_err(e)))?;
		let cred = JsFuture::from(promise).await.map_err(|e| Failure::Other(js_err(e)))?;
		if cred.is_null() {
			return Err(Failure::Other("no passkey chosen".into()));
		}
		let id = bytes(&get(&cred, "rawId")).ok_or("no credential id")?;
		let user = bytes(&get(&get(&cred, "response"), "userHandle"));
		let prf = prf_result(&cred)?.ok_or(Failure::NoPrf)?;
		Ok(Asserted { id, user, prf })
	}

	// --- IndexedDB: db `apakan`, store `keys`, key = account hex ---------

	async fn request(req: &web_sys::IdbRequest) -> Result<JsValue, String> {
		let r = req.clone();
		let promise = js_sys::Promise::new(&mut |resolve, reject| {
			let r2 = r.clone();
			let ok = wasm_bindgen::closure::Closure::once_into_js(move || {
				let _ = resolve.call1(&JsValue::NULL, &r2.result().unwrap_or(JsValue::UNDEFINED));
			});
			let err = wasm_bindgen::closure::Closure::once_into_js(move || {
				let _ = reject.call1(&JsValue::NULL, &"IndexedDB request failed".into());
			});
			r.set_onsuccess(Some(ok.unchecked_ref()));
			r.set_onerror(Some(err.unchecked_ref()));
		});
		JsFuture::from(promise).await.map_err(js_err)
	}

	async fn db() -> Result<web_sys::IdbDatabase, String> {
		let factory = web_sys::window()
			.ok_or("no window")?
			.indexed_db()
			.map_err(js_err)?
			.ok_or("IndexedDB unavailable")?;
		let open = factory.open_with_u32("apakan", 1).map_err(js_err)?;
		let o2 = open.clone();
		let upgrade = wasm_bindgen::closure::Closure::once_into_js(move || {
			if let Ok(db) = o2.result().and_then(|d| d.dyn_into::<web_sys::IdbDatabase>()) {
				if !db.object_store_names().contains("keys") {
					let _ = db.create_object_store("keys");
				}
			}
		});
		open.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
		let db = request(&open).await?;
		db.dyn_into::<web_sys::IdbDatabase>().map_err(js_err)
	}

	async fn idb_get(key: &str) -> Result<Option<String>, String> {
		let db = db().await?;
		let tx = db
			.transaction_with_str_and_mode("keys", web_sys::IdbTransactionMode::Readonly)
			.map_err(js_err)?;
		let req = tx.object_store("keys").map_err(js_err)?.get(&key.into()).map_err(js_err)?;
		Ok(request(&req).await?.as_string())
	}

	async fn idb_put(key: &str, value: &str) -> Result<(), String> {
		let db = db().await?;
		let tx = db
			.transaction_with_str_and_mode("keys", web_sys::IdbTransactionMode::Readwrite)
			.map_err(js_err)?;
		let store = tx.object_store("keys").map_err(js_err)?;
		let req = store.put_with_key(&value.into(), &key.into()).map_err(js_err)?;
		request(&req).await.map(|_| ())
	}

	// --- Server ------------------------------------------------------------

	async fn fetch(
		host: &HostConfig,
		path: &str,
		body: Option<Vec<u8>>,
	) -> Result<(u16, Vec<u8>), String> {
		let url = host.api(path).ok_or("no API configured")?;
		let mut req = match body {
			Some(b) => ehttp::Request::post(url, b),
			None => ehttp::Request::get(url),
		};
		req.headers = ehttp::Headers::new(&[
			("Content-Type", "application/json"),
			("Accept", "application/json"),
		]);
		let res = ehttp::fetch_async(req).await?;
		Ok((res.status, res.bytes))
	}

	/// Register the account if the server doesn't know it, then commit the
	/// passkey backup. Signed with the (unlocked) identity.
	async fn backup_on_server(
		host: &HostConfig,
		wrapped: PasskeyWrappedSeed,
	) -> Result<(), String> {
		let signer = identity::signer().ok_or("account locked")?;
		let acct = signer.account();
		let (_, info) = fetch(host, "/v1/ledger", None).await?;
		let info: serde_json::Value = serde_json::from_slice(&info).map_err(|e| e.to_string())?;
		let chain = info["chain"].as_str().ok_or("no chain")?.to_string();
		let stream = account_stream(&acct);
		let head = |v: &serde_json::Value| {
			(
				v["ledger"]["seq"].as_u64().unwrap_or(0),
				v["ledger"]["head"].as_str().unwrap_or(GENESIS_PREV).to_string(),
			)
		};
		let post = |tx: apa_kan_tx::SignedTx| async move {
			let body = serde_json::to_vec(&tx).map_err(|e| e.to_string())?;
			let (status, reply) = fetch(host, "/v1/tx", Some(body)).await?;
			let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap_or_default();
			if status == 200 {
				Ok(reply["view"].clone())
			} else {
				Err(reply["error"].as_str().unwrap_or("rejected").to_string())
			}
		};
		let (status, view) = fetch(host, &format!("/v1/accounts/{}", acct.to_hex()), None).await?;
		let mut view: serde_json::Value = serde_json::from_slice(&view).unwrap_or_default();
		if status == 404 {
			let reg = signer
				.sign(
					&chain,
					&stream,
					0,
					GENESIS_PREV,
					TxPayload::AccountRegister { label: String::new() },
				)
				.map_err(|e| e.to_string())?;
			view = post(reg).await?;
		}
		let (seq, prev) = head(&view);
		let tx = signer
			.sign(&chain, &stream, seq, &prev, TxPayload::AccountSetPasskeyBackup { wrapped })
			.map_err(|e| e.to_string())?;
		post(tx).await.map(|_| ())
	}

	pub async fn protect(host: &HostConfig) -> Result<String, Failure> {
		let seed = identity::seed().ok_or("unlock the account first")?;
		let account = identity::account();
		let created = create(&account).await?;
		let prf = match created.prf {
			Some(p) => p,
			// Some authenticators only evaluate PRF on get().
			None => assert(Some(&created.id)).await?.prf,
		};
		let wrapped = wrap_seed(&seed, &prf, &account, &created.id, random(), random());
		let json = serde_json::to_string(&wrapped).map_err(|e| e.to_string())?;
		idb_put(&account.to_hex(), &json).await?;
		let credential_id = wrapped.credential_id.clone();
		let server =
			if host.api_url.is_some() {
				match backup_on_server(host, wrapped).await {
				Ok(()) => " and backed up on the server, so it restores on any device with this passkey",
				Err(e) => {
					web_sys::console::warn_1(&format!("passkey backup: {e}").into());
					"; the server backup failed, so keep the recovery phrase too"
				}
			}
			} else {
				"; no server configured, so it restores only in this browser"
			};
		identity::install(seed, Some(account), Protection::Passkey { credential_id });
		Ok(format!("Account protected by your passkey{server}."))
	}

	/// The wrapped seed for this passkey: IndexedDB first, else the server,
	/// which serves it only for the PRF-derived access key.
	async fn wrapped_for(
		host: &HostConfig,
		account: &AccountId,
		cred_hex: &str,
		prf: &[u8; 32],
	) -> Result<PasskeyWrappedSeed, String> {
		if let Some(json) = idb_get(&account.to_hex()).await.ok().flatten() {
			if let Ok(w) = serde_json::from_str::<PasskeyWrappedSeed>(&json) {
				if w.credential_id == cred_hex {
					return Ok(w);
				}
			}
		}
		let cred = hex::decode(cred_hex).map_err(|e| e.to_string())?;
		let access = apa_kan_account::access_key(prf, account, &cred);
		let body = serde_json::to_vec(&serde_json::json!({ "access_key": hex::encode(access) }))
			.map_err(|e| e.to_string())?;
		let (status, body) = fetch(
			host,
			&format!("/v1/accounts/{}/passkeys/{cred_hex}", account.to_hex()),
			Some(body),
		)
		.await?;
		if status != 200 {
			return Err("no backup for this passkey here or on the server".into());
		}
		let w: PasskeyWrappedSeed = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
		if let Ok(json) = serde_json::to_string(&w) {
			let _ = idb_put(&account.to_hex(), &json).await;
		}
		Ok(w)
	}

	pub async fn unlock(host: &HostConfig) -> Result<(), Failure> {
		let Protection::Passkey { credential_id } = identity::protection() else {
			return Err("this account has no passkey".to_string().into());
		};
		let account = identity::account();
		let cred = hex::decode(&credential_id).map_err(|e| e.to_string())?;
		let a = assert(Some(&cred)).await?;
		let wrapped = wrapped_for(host, &account, &credential_id, &a.prf).await?;
		let seed = wrapped.unwrap_seed(&a.prf, &account).map_err(|e| e.to_string())?;
		identity::unlock(seed)?;
		REVISION.fetch_add(1, Ordering::Relaxed);
		Ok(())
	}

	pub async fn restore(host: &HostConfig) -> Result<String, Failure> {
		let a = assert(None).await?;
		let user = a.user.ok_or("the passkey did not name an account")?;
		let account = AccountId::from_bytes(user.try_into().map_err(|_| "not an Apa Kan passkey")?);
		let cred_hex = hex::encode(&a.id);
		let wrapped = wrapped_for(host, &account, &cred_hex, &a.prf).await?;
		let seed = wrapped.unwrap_seed(&a.prf, &account).map_err(|e| e.to_string())?;
		identity::install(seed, Some(account), Protection::Passkey { credential_id: cred_hex });
		REVISION.fetch_add(1, Ordering::Relaxed);
		Ok(account.to_hex())
	}
}
