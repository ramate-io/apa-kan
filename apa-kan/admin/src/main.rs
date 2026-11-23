mod client;
mod style;
mod ui;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use apa_kan_account::crypto::Seed;
use client::Api;
use ui::{Body, Page};

#[derive(Clone)]
struct AppState {
	api: Api,
	sessions: Arc<Mutex<HashMap<String, UserSession>>>,
}

struct UserSession {
	token: String,
	account_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
	let root =
		std::env::var("APA_KAN_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
	let state = AppState { api: Api::new(root), sessions: Arc::new(Mutex::new(HashMap::new())) };
	let app = Router::new()
		.route("/", get(home))
		.route("/login", post(login))
		.route("/logout", get(logout))
		.route("/characters", get(characters).post(add_character))
		.route("/characters/{id}", get(character).post(save_character))
		.route("/characters/{id}/embedding", post(save_embedding))
		.route("/characters/{id}/image", post(upload_image))
		.route("/proxy/characters/{id}/image", get(proxy_image))
		.route("/tiles", get(tiles))
		.route("/pass", get(pass))
		.route("/pass/config", post(save_tau))
		.route("/pass/publish", post(publish))
		.with_state(state);
	let port: u16 = std::env::var("ADMIN_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	let listener = TcpListener::bind(addr).await?;
	tracing::info!(%addr, "apa-kan-admin listening");
	axum::serve(listener, app).await?;
	Ok(())
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
	let raw = headers.get(header::COOKIE)?.to_str().ok()?;
	raw.split(';').find_map(|part| {
		let part = part.trim();
		part.strip_prefix("aka=").map(|s| s.to_string())
	})
}

fn session_of(state: &AppState, headers: &HeaderMap) -> Option<UserSession> {
	let token = cookie_token(headers)?;
	let map = state.sessions.lock().ok()?;
	map.get(&token)
		.map(|s| UserSession { token: s.token.clone(), account_id: s.account_id.clone() })
}

fn html(page: Page) -> Html<String> {
	Html(ui::render(page))
}

async fn home(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
	if session_of(&state, &headers).is_some() {
		return Redirect::to("/characters").into_response();
	}
	html(Page {
		title: "Sign in".into(),
		active: "login".into(),
		signed_in: false,
		flash: None,
		body: Body::Login { error: None },
	})
	.into_response()
}

#[derive(Deserialize)]
struct LoginForm {
	seed_hex: String,
}

async fn login(State(state): State<AppState>, Form(form): Form<LoginForm>) -> impl IntoResponse {
	let bytes = match hex::decode(form.seed_hex.trim()) {
		Ok(b) if b.len() == 32 => b,
		_ => {
			return html(Page {
				title: "Sign in".into(),
				active: "login".into(),
				signed_in: false,
				flash: None,
				body: Body::Login { error: Some("Seed must be 32 bytes hex.".into()) },
			})
			.into_response();
		}
	};
	let mut seed_bytes = [0u8; 32];
	seed_bytes.copy_from_slice(&bytes);
	let seed = Seed::from_bytes(seed_bytes);
	match state.api.sign_in(&seed).await {
		Ok((account_id, token, is_admin)) => {
			if !is_admin {
				return html(Page {
					title: "Sign in".into(),
					active: "login".into(),
					signed_in: false,
					flash: None,
					body: Body::Login {
						error: Some("Signed in, but this account is not an admin.".into()),
					},
				})
				.into_response();
			}
			let sid = uuid::Uuid::new_v4().to_string();
			state
				.sessions
				.lock()
				.expect("sessions")
				.insert(sid.clone(), UserSession { token, account_id });
			(
				[(header::SET_COOKIE, format!("aka={sid}; Path=/; HttpOnly; SameSite=Lax"))],
				Redirect::to("/characters"),
			)
				.into_response()
		}
		Err(err) => html(Page {
			title: "Sign in".into(),
			active: "login".into(),
			signed_in: false,
			flash: None,
			body: Body::Login { error: Some(err) },
		})
		.into_response(),
	}
}

async fn logout() -> impl IntoResponse {
	([(header::SET_COOKIE, "aka=; Path=/; Max-Age=0".to_string())], Redirect::to("/"))
}

fn need_session(state: &AppState, headers: &HeaderMap) -> Result<UserSession, Redirect> {
	session_of(state, headers).ok_or_else(|| Redirect::to("/"))
}

async fn characters(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	match state.api.get(&sess.token, "/v1/admin/characters").await {
		Ok(rows) => html(Page {
			title: "Characters".into(),
			active: "characters".into(),
			signed_in: true,
			flash: None,
			body: Body::Characters { rows },
		})
		.into_response(),
		Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
	}
}

#[derive(Deserialize)]
struct NewChar {
	character_id: String,
	display_name: String,
}

async fn add_character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Form(form): Form<NewChar>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let _ = state
		.api
		.send_json(
			&sess.token,
			reqwest::Method::POST,
			"/v1/admin/characters",
			&serde_json::json!({
				"character_id": form.character_id,
				"display_name": form.display_name,
			}),
		)
		.await;
	Redirect::to("/characters").into_response()
}

#[derive(Deserialize)]
struct FlashQ {
	flash: Option<String>,
}

async fn character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	Query(q): Query<FlashQ>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let detail = match state.api.get(&sess.token, &format!("/v1/admin/characters/{id}")).await {
		Ok(v) => v,
		Err(err) => return (StatusCode::BAD_GATEWAY, err).into_response(),
	};
	let vocab = state
		.api
		.get(&sess.token, "/v1/admin/vocabulary")
		.await
		.unwrap_or(serde_json::json!([]));
	let preview = if let Some(row) = detail.get("embedding") {
		state
			.api
			.send_json(
				&sess.token,
				reqwest::Method::POST,
				"/v1/admin/preview",
				&serde_json::json!({ "row": row, "k": 8 }),
			)
			.await
			.unwrap_or(serde_json::json!([]))
	} else {
		serde_json::json!([])
	};
	html(Page {
		title: id.clone(),
		active: "characters".into(),
		signed_in: true,
		flash: q.flash,
		body: Body::Character { id, detail, vocab, preview },
	})
	.into_response()
}

#[derive(Deserialize)]
struct NameForm {
	display_name: String,
}

async fn save_character(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	Form(form): Form<NameForm>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let _ = state
		.api
		.send_json(
			&sess.token,
			reqwest::Method::PUT,
			&format!("/v1/admin/characters/{id}"),
			&serde_json::json!({ "character_id": id, "display_name": form.display_name }),
		)
		.await;
	Redirect::to(&format!("/characters/{id}")).into_response()
}

async fn save_embedding(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let mut row = serde_json::json!({ "id": id });
	for (key, value) in form {
		let Ok(weight) = value.parse::<f32>() else { continue };
		if weight == 0.0 {
			continue;
		}
		let parts: Vec<&str> = key.split('.').collect();
		match parts.as_slice() {
			["color", c] => {
				row["color"][c] = serde_json::json!(weight);
			}
			["type", t] => {
				row["type"][t] = serde_json::json!(weight);
			}
			["emblem", e] => {
				row["emblem"][e] = serde_json::json!(weight);
			}
			["marks", "n_rings"] => row["marks"]["n_rings"] = serde_json::json!(weight),
			["marks", "dots"] => row["marks"]["dots"] = serde_json::json!(weight),
			["marks", "capture", c] => row["marks"]["capture"][c] = serde_json::json!(weight),
			_ => {}
		}
	}
	match state
		.api
		.send_json(
			&sess.token,
			reqwest::Method::PUT,
			&format!("/v1/admin/characters/{id}/embedding"),
			&serde_json::json!({ "row": row }),
		)
		.await
	{
		Ok(_) => Redirect::to(&format!("/characters/{id}")).into_response(),
		Err(err) => Redirect::to(&format!("/characters/{id}?flash=!{}", urlencoding_lite(&err)))
			.into_response(),
	}
}

fn urlencoding_lite(s: &str) -> String {
	let mut out = String::new();
	for b in s.as_bytes() {
		match b {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
				out.push(*b as char);
			}
			_ => out.push_str(&format!("%{b:02X}")),
		}
	}
	out
}

async fn upload_image(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
	mut multipart: Multipart,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	while let Ok(Some(field)) = multipart.next_field().await {
		if field.name() == Some("image") {
			if let Ok(bytes) = field.bytes().await {
				let _ = state.api.upload_png(&sess.token, &id, bytes.to_vec()).await;
			}
		}
	}
	Redirect::to(&format!("/characters/{id}")).into_response()
}

async fn proxy_image(
	State(state): State<AppState>,
	headers: HeaderMap,
	Path(id): Path<String>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return StatusCode::UNAUTHORIZED.into_response();
	};
	match reqwest::Client::new()
		.get(format!("{}/v1/admin/characters/{id}/image", state.api.root))
		.header("Authorization", format!("Bearer {}", sess.token))
		.send()
		.await
	{
		Ok(resp) if resp.status().is_success() => {
			let bytes = resp.bytes().await.unwrap_or_default();
			([(header::CONTENT_TYPE, "image/png")], bytes).into_response()
		}
		_ => StatusCode::NOT_FOUND.into_response(),
	}
}

#[derive(Deserialize)]
struct TileQ {
	character: Option<String>,
	core_tile: Option<String>,
	tile_type: Option<String>,
}

async fn tiles(
	State(state): State<AppState>,
	headers: HeaderMap,
	Query(q): Query<TileQ>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let mut path = "/v1/admin/tiles?".to_string();
	if let Some(c) = &q.character {
		path.push_str(&format!("character={c}&"));
	}
	if let Some(c) = &q.core_tile {
		path.push_str(&format!("core_tile={c}&"));
	}
	if let Some(c) = &q.tile_type {
		path.push_str(&format!("tile_type={c}&"));
	}
	match state.api.get(&sess.token, &path).await {
		Ok(rows) => html(Page {
			title: "Tiles".into(),
			active: "tiles".into(),
			signed_in: true,
			flash: None,
			body: Body::Tiles {
				rows,
				character: q.character.unwrap_or_default(),
				core: q.core_tile.unwrap_or_default(),
				tile_type: q.tile_type.unwrap_or_default(),
			},
		})
		.into_response(),
		Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
	}
}

async fn pass(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	match state.api.get(&sess.token, "/v1/admin/pass").await {
		Ok(pass) => html(Page {
			title: "Version".into(),
			active: "pass".into(),
			signed_in: true,
			flash: None,
			body: Body::Pass { pass },
		})
		.into_response(),
		Err(err) => (StatusCode::BAD_GATEWAY, err).into_response(),
	}
}

#[derive(Deserialize)]
struct TauForm {
	tau: f32,
}

async fn save_tau(
	State(state): State<AppState>,
	headers: HeaderMap,
	Form(form): Form<TauForm>,
) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let k = state
		.api
		.get(&sess.token, "/v1/admin/pass")
		.await
		.ok()
		.and_then(|p| p.get("k").and_then(|v| v.as_u64()).map(|n| n as usize));
	let _ = state
		.api
		.send_json(
			&sess.token,
			reqwest::Method::PUT,
			"/v1/admin/pass/config",
			&serde_json::json!({ "tau": form.tau, "k": k }),
		)
		.await;
	Redirect::to("/pass").into_response()
}

async fn publish(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
	let Ok(sess) = need_session(&state, &headers) else {
		return Redirect::to("/").into_response();
	};
	let _ = state
		.api
		.send_json(
			&sess.token,
			reqwest::Method::POST,
			"/v1/admin/pass/publish",
			&serde_json::json!({}),
		)
		.await;
	Redirect::to("/pass").into_response()
}
