//! HTTP surface. Mutations: `POST /v1/tx` only (one signed transaction for
//! any stream). Everything else is a view.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use apa_kan_account::id::AccountId;
use apa_kan_account::PasskeyWrappedSeed;
use apa_kan_tx::{SignedTx, TxError};
use apa_kan_view::{AccountKeyView, AccountView, HostedGameView, RolesView};

use crate::games::{CommittedTx, LedgerInfo};
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
	pub status: &'static str,
}

#[derive(Serialize, Deserialize)]
pub struct TileView {
	pub instance_id: u64,
	pub tile_kind_id: u32,
	pub origin: String,
	pub trade_bind_until_unix: Option<i64>,
}

/// `GET /v1/accounts/{id}`: the account fold plus its store projection
/// (inventory and rating, granted at registration).
#[derive(Serialize, Deserialize)]
pub struct AccountProfileView {
	#[serde(flatten)]
	pub account: AccountView,
	pub tiles: Vec<TileView>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub elo_milli: Option<i32>,
}

pub fn router(state: AppState) -> Router {
	Router::new()
		.route("/health", get(health))
		// One write route for every signed transaction; reads are views.
		.route("/v1/ledger", get(ledger_info))
		.route("/v1/tx", post(submit_tx))
		.route("/v1/games/{game_id}", get(get_game))
		.route("/v1/games/{game_id}/txs", get(get_game_txs))
		.route("/v1/accounts/{account_id}", get(get_account))
		.route("/v1/accounts/{account_id}/keys", get(get_account_keys))
		.route("/v1/accounts/{account_id}/passkeys/{credential_id}", post(get_passkey_backup))
		.route("/v1/admin/roles", get(get_roles))
		.layer(cors_layer())
		.with_state(state)
}

/// The web client is served from its own domain, so the API allows
/// cross-origin GET/POST with JSON bodies. Auth travels in the body (signed
/// transactions), never cookies, so no credentials are allowed.
///
/// `APAKAN_CORS_ORIGINS` (comma-separated) restricts origins; unset allows any.
fn cors_layer() -> tower_http::cors::CorsLayer {
	use axum::http::{header, HeaderValue, Method};
	use tower_http::cors::{AllowOrigin, CorsLayer};
	let origins: Vec<HeaderValue> = std::env::var("APAKAN_CORS_ORIGINS")
		.unwrap_or_default()
		.split(',')
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.filter_map(|s| HeaderValue::from_str(s).ok())
		.collect();
	let allow = if origins.is_empty() { AllowOrigin::any() } else { AllowOrigin::list(origins) };
	CorsLayer::new()
		.allow_origin(allow)
		.allow_methods([Method::GET, Method::POST])
		.allow_headers([header::CONTENT_TYPE])
}

/// JSON error body plus status for ledger routes.
pub type GameApiError = (StatusCode, Json<TxError>);

pub fn tx_error(e: TxError) -> GameApiError {
	let status = match &e {
		TxError::NotFound => StatusCode::NOT_FOUND,
		TxError::BadEnvelope { .. }
		| TxError::WrongChain { .. }
		| TxError::UnsupportedStream { .. } => StatusCode::BAD_REQUEST,
		TxError::BadSignature | TxError::KeyNotAuthorized => StatusCode::UNAUTHORIZED,
		TxError::Unauthorized => StatusCode::FORBIDDEN,
		TxError::StaleSeq { .. }
		| TxError::PrevMismatch { .. }
		| TxError::AlreadySeated
		| TxError::WrongPhase { .. }
		| TxError::NotYourTurn { .. }
		| TxError::GameOver
		| TxError::Rejected { .. }
		| TxError::TooEarly { .. } => StatusCode::CONFLICT,
		TxError::Illegal { .. } | TxError::BadReveal => StatusCode::UNPROCESSABLE_ENTITY,
		TxError::Host { .. } => StatusCode::SERVICE_UNAVAILABLE,
	};
	(status, Json(e))
}

async fn ledger_info(State(state): State<AppState>) -> Json<LedgerInfo> {
	let mut info = state.games.info();
	info.operator = state.accounts.operator_hex();
	Json(info)
}

fn to_json<T: Serialize>(v: T) -> Result<Json<serde_json::Value>, GameApiError> {
	serde_json::to_value(v)
		.map(Json)
		.map_err(|e| tx_error(TxError::Host { reason: e.to_string() }))
}

/// The only mutation route: one signed transaction, routed by stream kind.
/// The reply is `{ receipt, view }` with that stream's view. Each branch is
/// boxed so the handler future stays small.
async fn submit_tx(
	State(state): State<AppState>,
	Json(tx): Json<SignedTx>,
) -> Result<Json<serde_json::Value>, GameApiError> {
	let stream = tx.body.stream.as_str();
	if stream.starts_with("game/") {
		to_json(Box::pin(state.games.submit(tx)).await.map_err(tx_error)?)
	} else if stream.starts_with("account/") {
		to_json(Box::pin(state.accounts.submit(tx)).await.map_err(tx_error)?)
	} else if stream.starts_with("admin/") {
		to_json(Box::pin(state.accounts.submit_admin(tx)).await.map_err(tx_error)?)
	} else {
		Err(tx_error(TxError::UnsupportedStream { stream: stream.to_string() }))
	}
}

async fn get_game(
	State(state): State<AppState>,
	Path(game_id): Path<String>,
) -> Result<Json<HostedGameView>, GameApiError> {
	state.games.view(&game_id).await.map(Json).map_err(tx_error)
}

#[derive(Deserialize)]
struct FromQuery {
	#[serde(default)]
	from: u64,
}

async fn get_game_txs(
	State(state): State<AppState>,
	Path(game_id): Path<String>,
	axum::extract::Query(q): axum::extract::Query<FromQuery>,
) -> Result<Json<Vec<CommittedTx>>, GameApiError> {
	state.games.log(&game_id, q.from).await.map(Json).map_err(tx_error)
}

fn account_id(s: &str) -> Result<AccountId, GameApiError> {
	AccountId::from_hex(s.trim()).map_err(|_| tx_error(TxError::NotFound))
}

async fn get_account(
	State(state): State<AppState>,
	Path(id): Path<String>,
) -> Result<Json<AccountProfileView>, GameApiError> {
	let id = account_id(&id)?;
	let account = state.accounts.view(&id).await.map_err(tx_error)?;
	// The projection may lag a just-committed registration; show what it has.
	let (tiles, elo_milli) = match state.service().get_profile(&id).await {
		Ok((_, tiles, rating)) => (tiles, Some(rating.elo_milli)),
		Err(_) => (Vec::new(), None),
	};
	Ok(Json(AccountProfileView {
		account,
		tiles: tiles
			.into_iter()
			.map(|t| TileView {
				instance_id: t.instance_id,
				tile_kind_id: t.tile_kind_id,
				origin: t.origin,
				trade_bind_until_unix: t.trade_bind_until_unix,
			})
			.collect(),
		elo_milli,
	}))
}

async fn get_account_keys(
	State(state): State<AppState>,
	Path(id): Path<String>,
) -> Result<Json<Vec<AccountKeyView>>, GameApiError> {
	let id = account_id(&id)?;
	state.accounts.view(&id).await.map(|v| Json(v.keys)).map_err(tx_error)
}

/// Body of the passkey-backup fetch: the PRF-derived access key (hex).
#[derive(Debug, serde::Deserialize)]
struct PasskeyFetch {
	access_key: String,
}

/// The passkey-wrapped seed for one credential: what a new device fetches
/// after its passkey (synced by the platform) names the account and
/// credential. A read (no ledger change), but POST because the caller proves
/// it holds the passkey with `apa_kan_account::access_key` (it has no account
/// key to sign with yet). 403 on a wrong key, 404 if there is no backup.
async fn get_passkey_backup(
	State(state): State<AppState>,
	Path((id, credential_id)): Path<(String, String)>,
	Json(req): Json<PasskeyFetch>,
) -> Result<Json<PasskeyWrappedSeed>, GameApiError> {
	let id = account_id(&id)?;
	let account = state.accounts.state(&id).await.map_err(tx_error)?.ok_or(TxError::NotFound);
	let account = account.map_err(tx_error)?;
	let wrapped = account
		.passkey_backup(&credential_id.to_lowercase())
		.ok_or_else(|| tx_error(TxError::NotFound))?;
	let key: Option<[u8; 32]> =
		hex::decode(req.access_key.trim()).ok().and_then(|b| b.try_into().ok());
	match key {
		Some(k) if wrapped.grants(&k) => Ok(Json(wrapped.clone())),
		_ => Err(tx_error(TxError::Unauthorized)),
	}
}

async fn get_roles(State(state): State<AppState>) -> Result<Json<RolesView>, GameApiError> {
	state.accounts.roles_view().await.map(Json).map_err(tx_error)
}

async fn health() -> Json<HealthResponse> {
	Json(HealthResponse { status: "ok" })
}
