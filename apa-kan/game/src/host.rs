//! Deploy-time host configuration: where this client is served and where its
//! API lives.
//!
//! The web game gets its own domain (not chosen yet), so nothing here
//! hardcodes a host. Values resolve in this order:
//!
//! 1. Native: `--public-url` / `--api-url`, then `APAKAN_PUBLIC_URL` /
//!    `APAKAN_API_URL`.
//! 2. Web: `<meta name="apakan:public-url">` / `<meta name="apakan:api-url">`
//!    in the host page (`build-web.sh` fills them from the same env vars).
//! 3. Web fallback: `window.location.origin` for the public URL. The API URL
//!    stays unset, which keeps `/game/<id>` on the offline seeded stub.
//!
//! External links (market, payment, legal) have defaults and the same
//! override path (`apakan:market-url`, `APAKAN_MARKET_URL`, …).

use crate::route::Route;

pub const DEFAULT_MARKET_URL: &str = "https://symbolic.software/";
pub const DEFAULT_PAYMENT_URL: &str = "https://symbolic.software/";
pub const DEFAULT_LEGAL_URL: &str = "https://github.com/ramate-io/apa-kan/blob/main/LICENSE.md";

/// Resolved host settings. Empty strings never survive [`HostConfig::from_overrides`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostConfig {
	/// Public origin of the web client, e.g. `https://play.example`. Used to
	/// build shareable links. `None` on native when nothing is configured.
	pub public_url: Option<String>,
	/// Base URL of the platform API (`apa-kan-server`), e.g.
	/// `https://api.play.example`. `None` means offline only.
	pub api_url: Option<String>,
	pub market_url: String,
	pub payment_url: String,
	pub legal_url: String,
}

impl Default for HostConfig {
	fn default() -> Self {
		Self {
			public_url: None,
			api_url: None,
			market_url: DEFAULT_MARKET_URL.into(),
			payment_url: DEFAULT_PAYMENT_URL.into(),
			legal_url: DEFAULT_LEGAL_URL.into(),
		}
	}
}

/// One raw source of overrides (CLI, env, or the host page's meta tags).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostOverrides {
	pub public_url: Option<String>,
	pub api_url: Option<String>,
	pub market_url: Option<String>,
	pub payment_url: Option<String>,
	pub legal_url: Option<String>,
}

impl HostOverrides {
	/// Fill unset fields from `other` (earlier sources win).
	pub fn or(self, other: HostOverrides) -> Self {
		Self {
			public_url: self.public_url.or(other.public_url),
			api_url: self.api_url.or(other.api_url),
			market_url: self.market_url.or(other.market_url),
			payment_url: self.payment_url.or(other.payment_url),
			legal_url: self.legal_url.or(other.legal_url),
		}
	}

	/// `APAKAN_*` environment variables.
	pub fn from_env_fn(get: impl Fn(&str) -> Option<String>) -> Self {
		Self {
			public_url: get("APAKAN_PUBLIC_URL"),
			api_url: get("APAKAN_API_URL"),
			market_url: get("APAKAN_MARKET_URL"),
			payment_url: get("APAKAN_PAYMENT_URL"),
			legal_url: get("APAKAN_LEGAL_URL"),
		}
	}
}

impl HostConfig {
	pub fn from_overrides(o: HostOverrides) -> Self {
		let d = Self::default();
		Self {
			public_url: clean_base(o.public_url),
			api_url: clean_base(o.api_url),
			market_url: clean_base(o.market_url).unwrap_or(d.market_url),
			payment_url: clean_base(o.payment_url).unwrap_or(d.payment_url),
			legal_url: clean_base(o.legal_url).unwrap_or(d.legal_url),
		}
	}

	/// Resolve from the running platform. `cli` comes from `GameConfig::parse`.
	pub fn detect(cli: HostOverrides) -> Self {
		let env = HostOverrides::from_env_fn(|k| std::env::var(k).ok());
		let page = page_meta_overrides();
		let mut cfg = Self::from_overrides(cli.or(env).or(page));
		if cfg.public_url.is_none() {
			cfg.public_url = clean_base(browser_origin());
		}
		cfg
	}

	/// Shareable link for `route`, hash form so any static host serves it.
	/// Falls back to the native `apakan://` scheme when no public URL is set.
	pub fn share_url(&self, route: &Route) -> String {
		match &self.public_url {
			Some(base) => format!("{base}/{}", route.hash_path()),
			None => format!("apakan:/{}", route.path()),
		}
	}

	/// `{api_url}{path}` when an API is configured.
	pub fn api(&self, path: &str) -> Option<String> {
		let base = self.api_url.as_ref()?;
		let path = path.trim_start_matches('/');
		Some(format!("{base}/{path}"))
	}

	/// Short human summary for the Settings page.
	pub fn summary(&self) -> String {
		let public = self.public_url.as_deref().unwrap_or("not set");
		let api = self.api_url.as_deref().unwrap_or("offline (no API configured)");
		format!("Host: {public}\nAPI: {api}")
	}
}

/// Trim whitespace and trailing slashes; drop empty and unresolved
/// `%PLACEHOLDER%` values.
fn clean_base(v: Option<String>) -> Option<String> {
	let v = v?;
	let t = v.trim().trim_end_matches('/');
	if t.is_empty() || (t.starts_with('%') && t.ends_with('%')) {
		None
	} else {
		Some(t.to_string())
	}
}

fn page_meta_overrides() -> HostOverrides {
	HostOverrides {
		public_url: meta_content("apakan:public-url"),
		api_url: meta_content("apakan:api-url"),
		market_url: meta_content("apakan:market-url"),
		payment_url: meta_content("apakan:payment-url"),
		legal_url: meta_content("apakan:legal-url"),
	}
}

#[cfg(target_arch = "wasm32")]
fn meta_content(name: &str) -> Option<String> {
	let doc = web_sys::window()?.document()?;
	let el = doc.query_selector(&format!("meta[name=\"{name}\"]")).ok()??;
	el.get_attribute("content")
}

#[cfg(not(target_arch = "wasm32"))]
fn meta_content(_name: &str) -> Option<String> {
	None
}

#[cfg(target_arch = "wasm32")]
fn browser_origin() -> Option<String> {
	web_sys::window()?.location().origin().ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn browser_origin() -> Option<String> {
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_are_offline_with_no_host() {
		let cfg = HostConfig::from_overrides(HostOverrides::default());
		assert_eq!(cfg.public_url, None);
		assert_eq!(cfg.api_url, None);
		assert_eq!(cfg.market_url, DEFAULT_MARKET_URL);
		assert_eq!(cfg.api("/v1/games/x"), None);
	}

	#[test]
	fn trailing_slash_and_placeholders_are_cleaned() {
		let cfg = HostConfig::from_overrides(HostOverrides {
			public_url: Some(" https://play.example/ ".into()),
			api_url: Some("%APAKAN_API_URL%".into()),
			market_url: Some(String::new()),
			..HostOverrides::default()
		});
		assert_eq!(cfg.public_url.as_deref(), Some("https://play.example"));
		assert_eq!(cfg.api_url, None);
		assert_eq!(cfg.market_url, DEFAULT_MARKET_URL);
	}

	#[test]
	fn earlier_sources_win() {
		let cli = HostOverrides { api_url: Some("http://cli".into()), ..Default::default() };
		let env = HostOverrides {
			api_url: Some("http://env".into()),
			public_url: Some("http://env-pub".into()),
			..Default::default()
		};
		let cfg = HostConfig::from_overrides(cli.or(env));
		assert_eq!(cfg.api_url.as_deref(), Some("http://cli"));
		assert_eq!(cfg.public_url.as_deref(), Some("http://env-pub"));
	}

	#[test]
	fn share_and_api_urls() {
		let cfg = HostConfig::from_overrides(HostOverrides {
			public_url: Some("https://play.example".into()),
			api_url: Some("https://api.play.example/".into()),
			..Default::default()
		});
		let route = Route::Game { id: "g1".into() };
		assert_eq!(cfg.share_url(&route), "https://play.example/#/game/g1");
		assert_eq!(
			cfg.api("/v1/games/g1").as_deref(),
			Some("https://api.play.example/v1/games/g1")
		);
		let native = HostConfig::default();
		assert_eq!(native.share_url(&route), "apakan://game/g1");
	}

	#[test]
	fn env_names() {
		let o = HostOverrides::from_env_fn(|k| (k == "APAKAN_API_URL").then(|| "http://x".into()));
		assert_eq!(o.api_url.as_deref(), Some("http://x"));
		assert_eq!(o.public_url, None);
	}
}
