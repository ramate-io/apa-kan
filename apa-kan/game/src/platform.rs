//! Platform-neutral hooks for the system browser and incoming links.
//!
//! Android, iOS, and web agents should match these names:
//! - [`open_url`] — open an external page (payments, legal, store) outside Bevy
//! - [`IncomingUrl`] — a custom-scheme or https app-link delivered into Bevy
//!
//! The native wrapper stays thin: it forwards an `ACTION_VIEW` / iOS
//! `UIApplication.open` / equivalent into [`push_incoming_url`], and Bevy never
//! hosts an in-app browser.

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy::prelude::*;

/// Incoming deep link or App Link, forwarded from the platform wrapper.
///
/// `uri` is the raw URI (`apakan://…` or `https://…`). Routing lives in Bevy.
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
/// On iOS this is Safari via `UIApplication.open`. On Android it is an
/// `ACTION_VIEW` intent. Do not use an in-app WebView or Custom Tabs for
/// payments or external links.
pub fn open_url(url: &str) -> Result<(), OpenUrlError> {
	if !is_external_url(url) {
		return Err(OpenUrlError("unsupported url scheme".into()));
	}
	webbrowser::open(url).map_err(|err| OpenUrlError(err.to_string()))
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

/// Drains native incoming URLs onto [`IncomingUrl`] messages.
pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
	fn build(&self, app: &mut App) {
		app.add_message::<IncomingUrl>().add_systems(Update, poll_incoming_urls);
	}
}

fn poll_incoming_urls(mut writer: MessageWriter<IncomingUrl>) {
	for url in drain_incoming_urls() {
		writer.write(url);
	}
}

/// C ABI used by the iOS AppDelegate (`packaging/ios/native`).
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

#[cfg(target_os = "android")]
mod android_jni {
	use jni::objects::{JClass, JString};
	use jni::JNIEnv;

	/// Called from `io.ramate.apakan.MainActivity` when a VIEW intent arrives.
	#[allow(non_snake_case)]
	#[no_mangle]
	pub extern "system" fn Java_io_ramate_apakan_MainActivity_nativeOnIncomingUrl(
		mut env: JNIEnv,
		_class: JClass,
		uri: JString,
	) {
		let Ok(s) = env.get_string(&uri) else {
			return;
		};
		super::push_incoming_url(s.to_string_lossy());
	}
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
}
