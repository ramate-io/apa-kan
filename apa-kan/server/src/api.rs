use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{LinkApproval, LinkRequest};
use apa_kan_store::StoreError;
use apa_kan_tx::{Transaction, TxError};
use apa_kan_view::GameView;

use crate::games::{CreatedGame, NewGame, TxResponse};
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
	pub status: &'static str,
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
	pub genesis_vk_hex: String,
}

#[derive(Serialize)]
pub struct CreateAccountResponse {
	pub account_id: String,
}

#[derive(Serialize)]
pub struct TileView {
	pub instance_id: u64,
	pub tile_kind_id: u32,
	pub origin: String,
	pub trade_bind_until_unix: Option<i64>,
}

#[derive(Serialize)]
pub struct AccountProfileResponse {
	pub account_id: String,
	pub tiles: Vec<TileView>,
	pub elo_milli: i32,
}

#[derive(Deserialize)]
pub struct LinkRequestBody {
	pub request: LinkRequest,
}

#[derive(Deserialize)]
pub struct LinkApprovalBody {
	pub approval: LinkApproval,
}

#[derive(Serialize)]
pub struct LinkRequestResponse {
	pub request_id: String,
}

#[derive(Deserialize)]
pub struct RecoveryStartBody {
	pub pending_vk_hex: String,
	pub email: String,
}

#[derive(Serialize)]
pub struct OkResponse {
	pub ok: bool,
}

#[derive(Deserialize)]
pub struct RecoveryTokenBody {
	pub token: String,
}

#[derive(Deserialize)]
pub struct SetEmailBody {
	pub email: String,
	pub recovery_opt_in: bool,
	pub approve_via_email: bool,
}

#[derive(Serialize)]
pub struct KeyView {
	pub vk_hex: String,
	pub label: String,
	pub status: String,
}

fn vk_from_hex(s: &str) -> Result<VerifyingKeyBytes, (StatusCode, String)> {
	let bytes =
		hex::decode(s.trim()).map_err(|e| (StatusCode::BAD_REQUEST, format!("bad hex: {e}")))?;
	Ok(VerifyingKeyBytes(bytes))
}

fn account_id_from_path(s: &str) -> Result<AccountId, (StatusCode, String)> {
	AccountId::from_hex(s.trim()).map_err(|e| (StatusCode::BAD_REQUEST, format!("bad id: {e}")))
}

fn store_error(e: StoreError) -> (StatusCode, String) {
	match e {
		StoreError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
		StoreError::Conflict(msg) => (StatusCode::CONFLICT, msg),
		StoreError::BadNonce => (StatusCode::CONFLICT, "bad nonce".into()),
		StoreError::Expired => (StatusCode::GONE, "expired".into()),
		StoreError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
		StoreError::Account(err) => (StatusCode::UNAUTHORIZED, err.to_string()),
		StoreError::Storage(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
	}
}

pub fn router(state: AppState) -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/v1/accounts", post(create_account))
		.route("/v1/accounts/{account_id}", get(get_account))
		.route("/v1/accounts/{account_id}/link-requests", post(link_request))
		.route("/v1/accounts/{account_id}/link-requests/approve", post(link_approve))
		.route("/v1/accounts/{account_id}/recovery/start", post(recovery_start))
		.route("/v1/accounts/{account_id}/recovery/confirm", post(recovery_confirm))
		.route("/v1/accounts/{account_id}/recovery/cancel", post(recovery_cancel))
		.route("/v1/accounts/{account_id}/recovery/finish", post(recovery_finish))
		.route("/v1/accounts/{account_id}/email", post(set_email))
		.route("/v1/accounts/{account_id}/keys", get(list_keys))
		.route("/v1/games", post(create_game))
		.route("/v1/games/{game_id}", get(get_game))
		.route("/v1/games/{game_id}/transactions", post(submit_transaction))
		.layer(cors_layer())
		.with_state(state)
}

/// The web client is served from its own domain, so the API allows
/// cross-origin GET/POST with JSON bodies. Auth travels in the body (seat
/// tokens), never cookies, so no credentials are allowed.
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

/// JSON error body plus status for game routes.
pub type GameApiError = (StatusCode, Json<TxError>);

pub fn tx_error(e: TxError) -> GameApiError {
	let status = match &e {
		TxError::NotFound => StatusCode::NOT_FOUND,
		TxError::WrongGame { .. } | TxError::BadSeat { .. } => StatusCode::BAD_REQUEST,
		TxError::Unauthorized => StatusCode::UNAUTHORIZED,
		TxError::StaleSeq { .. } | TxError::NotYourTurn { .. } | TxError::GameOver => {
			StatusCode::CONFLICT
		}
		TxError::Illegal { .. } => StatusCode::UNPROCESSABLE_ENTITY,
		TxError::Host { .. } => StatusCode::SERVICE_UNAVAILABLE,
	};
	(status, Json(e))
}

async fn create_game(
	State(state): State<AppState>,
	body: Option<Json<NewGame>>,
) -> Result<Json<CreatedGame>, GameApiError> {
	let req = body.map(|Json(b)| b).unwrap_or_default();
	let games = state.games.clone();
	let created = tokio::task::spawn_blocking(move || games.create(req))
		.await
		.map_err(|e| tx_error(TxError::Host { reason: e.to_string() }))?
		.map_err(tx_error)?;
	Ok(Json(created))
}

async fn get_game(
	State(state): State<AppState>,
	Path(game_id): Path<String>,
) -> Result<Json<GameView>, GameApiError> {
	state.games.view(&game_id).map(Json).map_err(tx_error)
}

async fn submit_transaction(
	State(state): State<AppState>,
	Path(game_id): Path<String>,
	Json(tx): Json<Transaction>,
) -> Result<Json<TxResponse>, GameApiError> {
	state.games.submit(&game_id, &tx).map(Json).map_err(tx_error)
}

async fn health() -> Json<HealthResponse> {
	Json(HealthResponse { status: "ok" })
}

async fn create_account(
	State(state): State<AppState>,
	Json(body): Json<CreateAccountRequest>,
) -> Result<Json<CreateAccountResponse>, (StatusCode, String)> {
	let service = state.service();
	let vk = vk_from_hex(&body.genesis_vk_hex)?;
	let row = service.create_account(vk).await.map_err(store_error)?;
	Ok(Json(CreateAccountResponse { account_id: row.account_id.to_hex() }))
}

async fn get_account(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
) -> Result<Json<AccountProfileResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	let (_acc, tiles, rating) = service.get_profile(&id).await.map_err(store_error)?;
	Ok(Json(AccountProfileResponse {
		account_id: id.to_hex(),
		tiles: tiles
			.into_iter()
			.map(|t| TileView {
				instance_id: t.instance_id,
				tile_kind_id: t.tile_kind_id,
				origin: t.origin,
				trade_bind_until_unix: t.trade_bind_until_unix,
			})
			.collect(),
		elo_milli: rating.elo_milli,
	}))
}

async fn link_request(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
	Json(body): Json<LinkRequestBody>,
) -> Result<Json<LinkRequestResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	if body.request.payload.account_id != id {
		return Err((StatusCode::BAD_REQUEST, "account_id mismatch".into()));
	}
	let request_id = service.submit_link_request(body.request).await.map_err(store_error)?;
	Ok(Json(LinkRequestResponse { request_id: hex::encode(request_id) }))
}

async fn link_approve(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
	Json(body): Json<LinkApprovalBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	if body.approval.payload.account_id != id {
		return Err((StatusCode::BAD_REQUEST, "account_id mismatch".into()));
	}
	service.approve_link(body.approval).await.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn recovery_start(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
	Json(body): Json<RecoveryStartBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	let vk = vk_from_hex(&body.pending_vk_hex)?;
	service.start_recovery(&id, vk, &body.email).await.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn recovery_confirm(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
	Json(body): Json<RecoveryTokenBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	service.confirm_recovery_token(&id, &body.token).await.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn recovery_cancel(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	service.cancel_recovery(&id).await.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn recovery_finish(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	service.finish_recovery(&id).await.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn set_email(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
	Json(body): Json<SetEmailBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	service
		.set_recovery_email(&id, body.email, body.recovery_opt_in, body.approve_via_email)
		.await
		.map_err(store_error)?;
	Ok(Json(OkResponse { ok: true }))
}

async fn list_keys(
	State(state): State<AppState>,
	Path(account_id): Path<String>,
) -> Result<Json<Vec<KeyView>>, (StatusCode, String)> {
	let service = state.service();
	let id = account_id_from_path(&account_id)?;
	let keys = service.list_keys(&id).await.map_err(store_error)?;
	Ok(Json(
		keys.into_iter()
			.map(|k| KeyView {
				vk_hex: hex::encode(&k.vk.0),
				label: k.label,
				status: format!("{:?}", k.status).to_lowercase(),
			})
			.collect(),
	))
}
