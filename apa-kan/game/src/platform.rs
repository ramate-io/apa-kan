//! Platform-neutral hooks for the system browser and incoming links.
//!
//! Android (#95), iOS (#96), and web should match these names:
//! - [`open_url`] — open an external page (payments, legal, store) outside Bevy
//! - [`IncomingUrl`] — a custom-scheme or https app-link delivered into Bevy
//! - [`push_incoming_url`] — JNI / iOS / the web host enqueue here
//!
//! The wrapper stays thin: it forwards a VIEW intent / `UIApplication.open` /
//! `window.open` into these hooks, and Bevy never hosts an in-app browser.

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy::prelude::*;

use crate::route::Route;

/// Incoming deep link or App Link, forwarded from the platform wrapper.
///
/// `uri` is the raw URI (`apakan://…`, `/game/<id>`, `#/h2h`, or `https://…`).
/// Routing lives in Bevy.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct IncomingUrl {
	pub uri: String,
}

/// Failure from [`open_url`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenUrlError(pub String);

impl std::fmt::Display for OpenUrlError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::error::Error for OpenUrlError {}

/// Open `url` in the default system browser (or equivalent).
///
/// Web uses `window.open(url, "_blank")` so the OS/browser tab handles it.
/// Native uses the `webbrowser` crate (`ACTION_VIEW` / Safari / xdg-open).
/// Do not use an in-app WebView or Custom Tabs for payments or external links.
pub fn open_url(url: &str) -> Result<(), OpenUrlError> {
	if !is_external_url(url) {
		return Err(OpenUrlError("unsupported url scheme".into()));
	}
	#[cfg(target_arch = "wasm32")]
	{
		let window = web_sys::window().ok_or_else(|| OpenUrlError("no window".into()))?;
		window
			.open_with_url_and_target(url, "_blank")
			.map_err(|err| OpenUrlError(format!("{err:?}")))?;
		Ok(())
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		webbrowser::open(url).map_err(|err| OpenUrlError(err.to_string()))
	}
}

/// `http`, `https`, and `mailto` leave the app. Custom schemes are inbound only.
pub fn is_external_url(url: &str) -> bool {
	let lower = url.trim();
	lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
}

/// Queue a URI from the native wrapper (JNI / iOS / web). Safe to call before
/// the Bevy app exists; [`PlatformPlugin`] drains it onto [`IncomingUrl`].
pub fn push_incoming_url(uri: impl Into<String>) {
	let uri = uri.into();
	if uri.is_empty() {
		return;
	}
	if let Ok(mut q) = incoming_queue().lock() {
		q.push_back(IncomingUrl { uri });
	}
}

/// Take every pending URI. Used by tests and by [`PlatformPlugin`].
pub fn drain_incoming_urls() -> Vec<IncomingUrl> {
	incoming_queue().lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
}

fn incoming_queue() -> &'static Mutex<VecDeque<IncomingUrl>> {
	static QUEUE: Mutex<VecDeque<IncomingUrl>> = Mutex::new(VecDeque::new());
	&QUEUE
}

/// Last href we already pushed, so hash/path polling is edge-triggered.
#[derive(Resource, Default)]
pub struct BrowserLocation {
	pub last: String,
}

pub fn browser_href() -> Option<String> {
	#[cfg(target_arch = "wasm32")]
	{
		let window = web_sys::window()?;
		window.location().href().ok()
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		None
	}
}

/// Update the address bar to a hash route (static hosts need no rewrite).
pub fn replace_browser_route(route: &Route) {
	#[cfg(target_arch = "wasm32")]
	{
		let Some(window) = web_sys::window() else {
			return;
		};
		let Ok(history) = window.history() else {
			return;
		};
		let hash = route.hash_path();
		let _ =
			history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(hash.as_str()));
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = route;
	}
}

/// Drains native/web incoming URLs onto [`IncomingUrl`] messages.
pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
	fn build(&self, app: &mut App) {
		app.add_message::<IncomingUrl>()
			.init_resource::<BrowserLocation>()
			.add_systems(Startup, emit_startup_browser_url)
			.add_systems(Update, (poll_browser_location, poll_incoming_urls));
	}
}

fn emit_startup_browser_url(mut loc: ResMut<BrowserLocation>) {
	if let Some(href) = browser_href() {
		loc.last = href.clone();
		push_incoming_url(href);
	}
}

fn poll_browser_location(mut loc: ResMut<BrowserLocation>) {
	let Some(href) = browser_href() else {
		return;
	};
	if href == loc.last {
		return;
	}
	loc.last = href.clone();
	push_incoming_url(href);
}

fn poll_incoming_urls(mut writer: MessageWriter<IncomingUrl>) {
	for url in drain_incoming_urls() {
		writer.write(url);
	}
}

/// C ABI used by the iOS AppDelegate (`packaging/ios/native` on #96).
///
/// # Safety
/// `url` must be a valid, NUL-terminated UTF-8 C string, or null (ignored).
#[no_mangle]
pub unsafe extern "C" fn apa_kan_push_deep_link(url: *const std::os::raw::c_char) {
	if url.is_null() {
		return;
	}
	let Ok(s) = unsafe { std::ffi::CStr::from_ptr(url) }.to_str() else {
		return;
	};
	push_incoming_url(s);
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn external_url_allows_https_and_rejects_custom_scheme() {
		assert!(is_external_url("https://pay.example/checkout"));
		assert!(is_external_url("http://localhost/legal"));
		assert!(is_external_url("mailto:hello@example.com"));
		assert!(!is_external_url("apakan://match/1"));
		assert!(!is_external_url("ftp://files.example/a"));
		assert!(!is_external_url(""));
	}

	#[test]
	fn incoming_url_queue_round_trip() {
		let leftover = drain_incoming_urls();
		drop(leftover);
		push_incoming_url("apakan://settings");
		push_incoming_url("https://apakan.ramate.io/invite/abc");
		let got = drain_incoming_urls();
		assert_eq!(
			got,
			vec![
				IncomingUrl { uri: "apakan://settings".into() },
				IncomingUrl { uri: "https://apakan.ramate.io/invite/abc".into() },
			]
		);
		assert!(drain_incoming_urls().is_empty());
	}

	#[test]
	fn open_url_rejects_custom_scheme() {
		assert!(open_url("apakan://h2h").is_err());
	}
}
