//! Admin HTTP API: catalog, sparse attributes, draft/publish, issued tiles.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use apa_kan_account::auth::{verify_auth_challenge, AuthChallengePayload, AuthChallengeResponse};
use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_core::character_pass::{
	encode_character, encode_tile, neighborhood, CharacterTables, SparseCharacterRow, EMBED_DIM,
};
use apa_kan_core::characters::PNG_MAGIC;
use apa_kan_core::core_pass::CoreTile;
use apa_kan_core::game::{CaptureMark, Emblem, RingType, TileColor};
use apa_kan_core::vocabulary::VOCABULARY;
use apa_kan_store::pass::ROLE_ADMIN;
use apa_kan_store::{seed_bundled_v1, CharacterRecord, StoreError, TileFilter, TileInstanceRow};

use crate::api::store_error;
use crate::session::Session;
use crate::state::AppState;

const MAX_PNG_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ChallengeRequest {
	pub account_id: String,
	pub vk_hex: String,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
	pub payload: AuthChallengePayload,
}

#[derive(Serialize)]
pub struct SessionView {
	pub token: String,
	pub account_id: String,
	pub is_admin: bool,
	pub expires_at_unix: i64,
}

#[derive(Serialize)]
pub struct MeView {
	pub account_id: String,
	pub is_admin: bool,
}

#[derive(Deserialize)]
pub struct NewCharacterBody {
	pub character_id: String,
	pub display_name: String,
}

#[derive(Serialize)]
pub struct CharacterView {
	pub character_id: String,
	pub display_name: String,
	pub has_image: bool,
	pub embedding: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct EmbeddingBody {
	pub row: serde_json::Value,
}

#[derive(Serialize)]
pub struct PassView {
	pub draft_version: u16,
	pub published_version: Option<u16>,
	pub tau: f32,
	pub k: Option<usize>,
	pub fallback: Vec<String>,
}

#[derive(Deserialize)]
pub struct ConfigBody {
	pub tau: f32,
	pub k: Option<usize>,
}

#[derive(Deserialize)]
pub struct FallbackBody {
	pub ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct SpecificBody {
	pub maps: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct TileQuery {
	pub character: Option<String>,
	pub core_tile: Option<String>,
	pub tile_type: Option<String>,
	pub limit: Option<u32>,
	pub offset: Option<u32>,
}

#[derive(Serialize)]
pub struct TileView {
	pub instance_id: u64,
	pub account_id: String,
	pub tile_kind_id: u32,
	pub origin: String,
	pub generation_id: u32,
	pub core_tile_id: String,
	pub tile_type_id: String,
	pub character_id: String,
	pub pass_version: u16,
}

#[derive(Deserialize)]
pub struct PreviewBody {
	pub row: Option<serde_json::Value>,
	pub core_tile: Option<CoreTilePreview>,
	pub tau: Option<f32>,
	pub k: Option<usize>,
	pub version: Option<u16>,
}

#[derive(Deserialize)]
pub struct CoreTilePreview {
	pub rings: Vec<(String, String)>,
	pub dots: u8,
	pub capture: String,
	pub emblem: String,
}

#[derive(Serialize)]
pub struct NeighborView {
	pub character_id: String,
	pub cosine: f32,
}

#[derive(Serialize)]
pub struct VocabView {
	pub key: &'static str,
	pub label: &'static str,
	pub group: &'static str,
}

pub fn router() -> Router<AppState> {
	Router::new()
		.route("/v1/auth/challenge", post(challenge))
		.route("/v1/auth/session", post(session))
		.route("/v1/auth/me", get(me))
		.route("/v1/admin/vocabulary", get(vocabulary))
		.route("/v1/admin/pass", get(get_pass))
		.route("/v1/admin/pass/publish", post(publish))
		.route("/v1/admin/pass/config", put(set_config))
		.route("/v1/admin/pass/fallback", put(set_fallback))
		.route("/v1/admin/pass/specific", put(set_specific))
		.route("/v1/admin/preview", post(preview))
		.route("/v1/admin/characters", get(list_characters).post(create_character))
		.route("/v1/admin/characters/{id}", get(get_character).put(update_character))
		.route("/v1/admin/characters/{id}/embedding", put(set_embedding))
		.route("/v1/admin/characters/{id}/image", get(get_image).put(put_image))
		.route("/v1/admin/tiles", get(list_tiles))
}

fn vk_from_hex(s: &str) -> Result<VerifyingKeyBytes, (StatusCode, String)> {
	let bytes =
		hex::decode(s.trim()).map_err(|e| (StatusCode::BAD_REQUEST, format!("bad hex: {e}")))?;
	Ok(VerifyingKeyBytes(bytes))
}

async fn require_session(
	state: &AppState,
	headers: &HeaderMap,
) -> Result<Session, (StatusCode, String)> {
	let auth = headers
		.get(header::AUTHORIZATION)
		.and_then(|v| v.to_str().ok())
		.ok_or((StatusCode::UNAUTHORIZED, "missing authorization".into()))?;
	let token = auth
		.strip_prefix("Bearer ")
		.ok_or((StatusCode::UNAUTHORIZED, "expected bearer token".into()))?;
	state
		.sessions
		.get(token, state.clock.now_unix())
		.ok_or((StatusCode::UNAUTHORIZED, "invalid session".into()))
}

async fn require_admin(
	state: &AppState,
	headers: &HeaderMap,
) -> Result<Session, (StatusCode, String)> {
	let session = require_session(state, headers).await?;
	if !session.is_admin {
		return Err((StatusCode::FORBIDDEN, "admin role required".into()));
	}
	Ok(session)
}

async fn challenge(
	State(state): State<AppState>,
	Json(body): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, (StatusCode, String)> {
	let account_id = AccountId::from_hex(body.account_id.trim())
		.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
	let vk = vk_from_hex(&body.vk_hex)?;
	state
		.store
		.get_account(&account_id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
	if !state.store.is_active_key(&account_id, &vk).await.map_err(store_error)? {
		return Err((StatusCode::UNAUTHORIZED, "key is not active".into()));
	}
	let mut nonce = [0u8; 32];
	use rand::RngCore;
	rand::thread_rng().fill_bytes(&mut nonce);
	let challenge_id = hex::encode(nonce);
	let payload =
		AuthChallengePayload::new(account_id, vk, challenge_id, nonce, state.clock.now_unix());
	state
		.challenges
		.lock()
		.expect("challenges")
		.insert(payload.challenge_id.clone(), payload.clone());
	Ok(Json(ChallengeResponse { payload }))
}

async fn session(
	State(state): State<AppState>,
	Json(body): Json<AuthChallengeResponse>,
) -> Result<Json<SessionView>, (StatusCode, String)> {
	let issued = {
		let mut map = state.challenges.lock().expect("challenges");
		map.remove(&body.payload.challenge_id)
			.ok_or((StatusCode::UNAUTHORIZED, "unknown challenge".into()))?
	};
	verify_auth_challenge(&issued, &body, state.clock.now_unix())
		.map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
	if !state
		.store
		.is_active_key(&issued.account_id, &issued.vk)
		.await
		.map_err(store_error)?
	{
		return Err((StatusCode::UNAUTHORIZED, "key is not active".into()));
	}
	let mut is_admin = state
		.store
		.has_role(&issued.account_id, ROLE_ADMIN)
		.await
		.map_err(store_error)?;
	if !is_admin {
		if let Some(hex_id) = &state.bootstrap_admin {
			if issued.account_id.to_hex() == *hex_id {
				state
					.store
					.grant_role(&issued.account_id, ROLE_ADMIN, state.clock.now_unix())
					.await
					.map_err(store_error)?;
				is_admin = true;
			}
		} else if state.allow_first_admin
			&& state.store.admin_count().await.map_err(store_error)? == 0
		{
			state
				.store
				.grant_role(&issued.account_id, ROLE_ADMIN, state.clock.now_unix())
				.await
				.map_err(store_error)?;
			is_admin = true;
		}
	}
	let session =
		state
			.sessions
			.issue(issued.account_id, issued.vk, is_admin, state.clock.now_unix());
	Ok(Json(SessionView {
		token: session.token,
		account_id: session.account_id.to_hex(),
		is_admin: session.is_admin,
		expires_at_unix: session.expires_at_unix,
	}))
}

async fn me(
	State(state): State<AppState>,
	headers: HeaderMap,
) -> Result<Json<MeView>, (StatusCode, String)> {
	let session = require_session(&state, &headers).await?;
	Ok(Json(MeView { account_id: session.account_id.to_hex(), is_admin: session.is_admin }))
}

async fn vocabulary(
	State(state): State<AppState>,
	headers: HeaderMap,
) -> Result<Json<Vec<VocabView>>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	Ok(Json(
		VOCABULARY
			.iter()
			.map(|e| VocabView {
				key: e.key,
				label: e.label,
				group: match e.group {
					apa_kan_core::vocabulary::VocabGroup::Color => "color",
					apa_kan_core::vocabulary::VocabGroup::RingType => "type",
					apa_kan_core::vocabulary::VocabGroup::Emblem => "emblem",
					apa_kan_core::vocabulary::VocabGroup::Marks => "marks",
				},
			})
			.collect(),
	))
}

fn character_view(rec: CharacterRecord, embedding: Option<SparseCharacterRow>) -> CharacterView {
	CharacterView {
		character_id: rec.character_id,
		display_name: rec.display_name,
		has_image: rec.image_png.is_some(),
		embedding: embedding.map(|row| row.to_json_value()),
	}
}

async fn live_embedding(
	state: &AppState,
	id: &str,
	version: u16,
) -> Result<Option<SparseCharacterRow>, (StatusCode, String)> {
	let tables = state.store.load_tables(version).await.ok();
	Ok(tables.and_then(|t| t.embedding_rows().iter().find(|r| r.id == id).cloned()))
}

async fn list_characters(
	State(state): State<AppState>,
	headers: HeaderMap,
) -> Result<Json<Vec<CharacterView>>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let version = pointer.draft_version;
	let rows = state.store.list_characters().await.map_err(store_error)?;
	let mut out = Vec::new();
	for rec in rows {
		let emb = live_embedding(&state, &rec.character_id, version).await?;
		out.push(character_view(rec, emb));
	}
	Ok(Json(out))
}

async fn create_character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Json(body): Json<NewCharacterBody>,
) -> Result<Json<CharacterView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rec = CharacterRecord {
		character_id: body.character_id,
		display_name: body.display_name,
		image_png: None,
		created_at_unix: state.clock.now_unix(),
	};
	state.store.upsert_character(rec.clone()).await.map_err(store_error)?;
	Ok(Json(character_view(rec, None)))
}

async fn get_character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
) -> Result<Json<CharacterView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rec = state
		.store
		.get_character(&id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let emb = live_embedding(&state, &id, pointer.draft_version).await?;
	Ok(Json(character_view(rec, emb)))
}

async fn update_character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	Json(body): Json<NewCharacterBody>,
) -> Result<Json<CharacterView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	if id != body.character_id {
		return Err((StatusCode::BAD_REQUEST, "character_id mismatch".into()));
	}
	let existing = state
		.store
		.get_character(&id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
	let rec = CharacterRecord {
		character_id: id.clone(),
		display_name: body.display_name,
		image_png: existing.image_png,
		created_at_unix: existing.created_at_unix,
	};
	state.store.upsert_character(rec.clone()).await.map_err(store_error)?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let emb = live_embedding(&state, &id, pointer.draft_version).await?;
	Ok(Json(character_view(rec, emb)))
}

async fn set_embedding(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	Json(body): Json<EmbeddingBody>,
) -> Result<Json<CharacterView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rec = state
		.store
		.get_character(&id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
	let mut row =
		SparseCharacterRow::from_json_value(&body.row).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
	row.id = id.clone();
	row.validate().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
	state.store.upsert_draft_embedding(row.clone()).await.map_err(store_error)?;
	Ok(Json(character_view(rec, Some(row))))
}

async fn get_image(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
) -> Result<(HeaderMap, Vec<u8>), (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rec = state
		.store
		.get_character(&id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
	let png = rec.image_png.ok_or((StatusCode::NOT_FOUND, "no image".into()))?;
	let mut headers = HeaderMap::new();
	headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
	Ok((headers, png))
}

async fn put_image(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	mut multipart: Multipart,
) -> Result<Json<CharacterView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rec = state
		.store
		.get_character(&id)
		.await
		.map_err(store_error)?
		.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
	let mut png = None;
	while let Some(field) = multipart
		.next_field()
		.await
		.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
	{
		if field.name() == Some("image") || field.name() == Some("file") || png.is_none() {
			let bytes =
				field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
			png = Some(bytes.to_vec());
		}
	}
	let png = png.ok_or((StatusCode::BAD_REQUEST, "missing image field".into()))?;
	if png.len() > MAX_PNG_BYTES {
		return Err((StatusCode::BAD_REQUEST, "image too large".into()));
	}
	if !png.starts_with(PNG_MAGIC) {
		return Err((StatusCode::BAD_REQUEST, "not a PNG".into()));
	}
	state.store.set_character_image(&id, png).await.map_err(store_error)?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let emb = live_embedding(&state, &id, pointer.draft_version).await?;
	Ok(Json(character_view(rec, emb)))
}

async fn get_pass(
	State(state): State<AppState>,
	headers: HeaderMap,
) -> Result<Json<PassView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let tables = state.store.load_tables(pointer.draft_version).await.map_err(store_error)?;
	Ok(Json(PassView {
		draft_version: pointer.draft_version,
		published_version: pointer.published_version,
		tau: tables.tau(),
		k: tables.k(),
		fallback: tables.fallback_ids().to_vec(),
	}))
}

async fn publish(
	State(state): State<AppState>,
	headers: HeaderMap,
) -> Result<Json<PassView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	state.store.publish_draft().await.map_err(store_error)?;
	get_pass(State(state), headers).await
}

async fn set_config(
	State(state): State<AppState>,
	headers: HeaderMap,
	Json(body): Json<ConfigBody>,
) -> Result<Json<PassView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	state.store.set_draft_config(body.tau, body.k).await.map_err(store_error)?;
	get_pass(State(state), headers).await
}

async fn set_fallback(
	State(state): State<AppState>,
	headers: HeaderMap,
	Json(body): Json<FallbackBody>,
) -> Result<Json<PassView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	state.store.set_draft_fallback(body.ids).await.map_err(store_error)?;
	get_pass(State(state), headers).await
}

async fn set_specific(
	State(state): State<AppState>,
	headers: HeaderMap,
	Json(body): Json<SpecificBody>,
) -> Result<Json<PassView>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let mut maps = std::collections::BTreeMap::new();
	for v in body.maps {
		let (key, chars) = apa_kan_core::character_pass::CoreKey::from_map_json(&v)
			.map_err(|e| (StatusCode::BAD_REQUEST, e))?;
		maps.insert(key, chars);
	}
	state.store.set_draft_specific(maps).await.map_err(store_error)?;
	get_pass(State(state), headers).await
}

async fn preview(
	State(state): State<AppState>,
	headers: HeaderMap,
	Json(body): Json<PreviewBody>,
) -> Result<Json<Vec<NeighborView>>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let pointer = state.store.pass_pointer().await.map_err(store_error)?;
	let version = body.version.unwrap_or(pointer.draft_version);
	let tables = state.store.load_tables(version).await.map_err(store_error)?;
	let query = if let Some(row) = body.row {
		let parsed =
			SparseCharacterRow::from_json_value(&row).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
		encode_character(&parsed)
	} else if let Some(core) = body.core_tile {
		let tile = parse_core_preview(&core)?;
		encode_tile(&tile)
	} else {
		return Err((StatusCode::BAD_REQUEST, "row or core_tile required".into()));
	};
	if query.len() != EMBED_DIM {
		return Err((StatusCode::BAD_REQUEST, "bad vector".into()));
	}
	let tau = body.tau.unwrap_or(tables.tau());
	let mut hits = state
		.store
		.nearest(version, &query, tau, body.k.or(Some(8)))
		.await
		.map_err(store_error)?;
	if hits.is_empty() {
		hits = neighborhood(&tables, &query)
			.into_iter()
			.map(|h| apa_kan_store::AnnHit { character_id: h.id, cosine: h.cosine })
			.collect();
	}
	Ok(Json(
		hits.into_iter()
			.map(|h| NeighborView { character_id: h.character_id, cosine: h.cosine })
			.collect(),
	))
}

fn parse_core_preview(p: &CoreTilePreview) -> Result<CoreTile, (StatusCode, String)> {
	let mut rings = Vec::new();
	let mut ring_type = Vec::new();
	for (c, t) in &p.rings {
		rings.push(TileColor::parse(c).map_err(|e| (StatusCode::BAD_REQUEST, e))?);
		ring_type.push(RingType::parse(t).map_err(|e| (StatusCode::BAD_REQUEST, e))?);
	}
	Ok(CoreTile {
		ring_type,
		rings,
		dots: p.dots,
		capture: CaptureMark::parse(&p.capture).map_err(|e| (StatusCode::BAD_REQUEST, e))?,
		emblem: Emblem::parse(&p.emblem).map_err(|e| (StatusCode::BAD_REQUEST, e))?,
	})
}

async fn list_tiles(
	State(state): State<AppState>,
	headers: HeaderMap,
	Query(q): Query<TileQuery>,
) -> Result<Json<Vec<TileView>>, (StatusCode, String)> {
	require_admin(&state, &headers).await?;
	let rows = state
		.store
		.list_issued(TileFilter {
			character_id: q.character,
			core_tile_id: q.core_tile,
			tile_type_id: q.tile_type,
			account_id: None,
			limit: q.limit.or(Some(100)),
			offset: q.offset,
		})
		.await
		.map_err(store_error)?;
	Ok(Json(rows.into_iter().map(tile_view).collect()))
}

fn tile_view(t: TileInstanceRow) -> TileView {
	TileView {
		instance_id: t.instance_id,
		account_id: t.account_id.to_hex(),
		tile_kind_id: t.tile_kind_id,
		origin: t.origin,
		generation_id: t.generation_id,
		core_tile_id: t.core_tile_id,
		tile_type_id: t.tile_type_id,
		character_id: t.character_id,
		pass_version: t.pass_version,
	}
}

pub async fn seed_if_needed(state: &AppState) -> Result<(), StoreError> {
	seed_bundled_v1(state.store.as_ref(), state.clock.now_unix()).await
}
