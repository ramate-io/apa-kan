//! Platform-neutral deep-link routes.
//!
//! One parser covers browser paths (`/game/<id>`, `#/game/<id>`), `https://`
//! URLs, and native `apakan://` links so web, iOS, and Android can share the
//! same [`Route`] event payload.

use std::collections::BTreeMap;

/// Where the client should go. Named generically so native shells can emit
/// the same event they would for `apakan://h2h` or `apakan://game/<id>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
	Home,
	HeadToHead,
	Puzzles,
	Market,
	Settings,
	Game { id: String },
}

/// A parsed deep link: route plus leftover query pairs (`opponent=bot`, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeepLink {
	pub route: Route,
	pub query: BTreeMap<String, String>,
}

impl DeepLink {
	pub fn parse(input: &str) -> Self {
		let trimmed = input.trim();
		if trimmed.is_empty() {
			return Self { route: Route::Home, query: BTreeMap::new() };
		}
		let without_scheme = strip_scheme(trimmed);
		let (path_and_query, hash_part) = split_hash(without_scheme);
		let from_hash = hash_part.filter(|h| !h.is_empty());
		let raw = from_hash.unwrap_or(path_and_query);
		let (path, query) = split_query(raw);
		let route = Route::from_path(path);
		Self { route, query }
	}
}

impl Route {
	pub fn from_path(path: &str) -> Self {
		let path = path.trim().trim_start_matches('/');
		if path.is_empty() || path.eq_ignore_ascii_case("home") {
			return Self::Home;
		}
		let mut parts = path.split('/');
		let head = parts.next().unwrap_or("");
		match head.to_ascii_lowercase().as_str() {
			"h2h" | "head-to-head" | "headtohead" => Self::HeadToHead,
			"puzzles" | "puzzle" => Self::Puzzles,
			"market" => Self::Market,
			"settings" | "setting" => Self::Settings,
			"game" => {
				let id = parts.collect::<Vec<_>>().join("/");
				if id.is_empty() {
					Self::HeadToHead
				} else {
					Self::Game { id }
				}
			}
			_ => Self::Home,
		}
	}

	pub fn path(&self) -> String {
		match self {
			Self::Home => "/".into(),
			Self::HeadToHead => "/h2h".into(),
			Self::Puzzles => "/puzzles".into(),
			Self::Market => "/market".into(),
			Self::Settings => "/settings".into(),
			Self::Game { id } => format!("/game/{id}"),
		}
	}

	/// Hash form that static file servers can host without a rewrite rule.
	pub fn hash_path(&self) -> String {
		match self {
			Self::Home => "#/".into(),
			other => format!("#{}", other.path()),
		}
	}
}

fn strip_scheme(s: &str) -> &str {
	if let Some(rest) = s.strip_prefix("apakan://") {
		return rest.trim_start_matches('/');
	}
	if let Some(rest) = s.strip_prefix("apakan:") {
		return rest.trim_start_matches('/');
	}
	if let Some(idx) = s.find("://") {
		let rest = &s[idx + 3..];
		if let Some(hash) = rest.find('#') {
			if rest[..hash].find('/').is_none() {
				return &rest[hash..];
			}
		}
		if let Some(slash) = rest.find('/') {
			return &rest[slash..];
		}
		return "";
	}
	s
}

fn split_hash(s: &str) -> (&str, Option<&str>) {
	match s.find('#') {
		Some(i) => {
			let hash = s[i + 1..].trim_start_matches('/');
			(&s[..i], Some(hash))
		}
		None => (s, None),
	}
}

fn split_query(s: &str) -> (&str, BTreeMap<String, String>) {
	match s.find('?') {
		Some(i) => (&s[..i], parse_query(&s[i + 1..])),
		None => (s, BTreeMap::new()),
	}
}

fn parse_query(q: &str) -> BTreeMap<String, String> {
	let mut out = BTreeMap::new();
	for pair in q.split('&') {
		if pair.is_empty() {
			continue;
		}
		let (k, v) = match pair.split_once('=') {
			Some((k, v)) => (k, v),
			None => (pair, ""),
		};
		out.insert(percent_decode(k), percent_decode(v));
	}
	out
}

fn percent_decode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let bytes = s.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		if bytes[i] == b'%' && i + 2 < bytes.len() {
			if let (Ok(hi), Ok(lo)) =
				(u8::from_str_radix(&s[i + 1..i + 2], 16), u8::from_str_radix(&s[i + 2..i + 3], 16))
			{
				out.push((hi << 4 | lo) as char);
				i += 3;
				continue;
			}
		}
		if bytes[i] == b'+' {
			out.push(' ');
		} else {
			out.push(bytes[i] as char);
		}
		i += 1;
	}
	out
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

	#[test]
	fn empty_and_home() {
		assert_eq!(DeepLink::parse("").route, Route::Home);
		assert_eq!(DeepLink::parse("/").route, Route::Home);
		assert_eq!(DeepLink::parse("#/").route, Route::Home);
		assert_eq!(DeepLink::parse("apakan://home").route, Route::Home);
	}

	#[test]
	fn hash_and_path_h2h() {
		assert_eq!(DeepLink::parse("/h2h").route, Route::HeadToHead);
		assert_eq!(DeepLink::parse("#/h2h").route, Route::HeadToHead);
		assert_eq!(DeepLink::parse("apakan://h2h").route, Route::HeadToHead);
		assert_eq!(DeepLink::parse("apakan://head-to-head").route, Route::HeadToHead);
	}

	#[test]
	fn game_id_from_native_and_web() {
		assert_eq!(
			DeepLink::parse("apakan://game/abc123").route,
			Route::Game { id: "abc123".into() }
		);
		assert_eq!(DeepLink::parse("/game/abc123").route, Route::Game { id: "abc123".into() });
		assert_eq!(DeepLink::parse("#/game/abc123").route, Route::Game { id: "abc123".into() });
		assert_eq!(
			DeepLink::parse("https://play.example/game/abc123").route,
			Route::Game { id: "abc123".into() }
		);
		assert_eq!(
			DeepLink::parse("https://play.example/#/game/abc123").route,
			Route::Game { id: "abc123".into() }
		);
	}

	#[test]
	fn query_pairs_survive() {
		let link = DeepLink::parse("apakan://h2h?opponent=bot&bot=minimax&time=2%2B1");
		assert_eq!(link.route, Route::HeadToHead);
		assert_eq!(link.query.get("opponent").map(String::as_str), Some("bot"));
		assert_eq!(link.query.get("bot").map(String::as_str), Some("minimax"));
		assert_eq!(link.query.get("time").map(String::as_str), Some("2+1"));
	}

	#[test]
	fn market_settings_puzzles() {
		assert_eq!(DeepLink::parse("/market").route, Route::Market);
		assert_eq!(DeepLink::parse("#/settings").route, Route::Settings);
		assert_eq!(DeepLink::parse("apakan://puzzles").route, Route::Puzzles);
	}

	#[test]
	fn hash_path_round_trips() {
		assert_eq!(Route::Home.hash_path(), "#/");
		assert_eq!(Route::HeadToHead.hash_path(), "#/h2h");
		assert_eq!(
			DeepLink::parse(&Route::Game { id: "z".into() }.hash_path()).route,
			Route::Game { id: "z".into() }
		);
	}
}
