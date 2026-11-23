use dioxus::prelude::*;
use serde_json::Value;

use crate::style::CSS;

#[derive(Clone, PartialEq)]
pub struct Page {
	pub title: String,
	pub active: String,
	pub signed_in: bool,
	pub flash: Option<String>,
	pub body: Body,
}

#[derive(Clone, PartialEq)]
pub enum Body {
	Login { error: Option<String> },
	Characters { rows: Value },
	Character { id: String, detail: Value, vocab: Value, preview: Value },
	Tiles { rows: Value, character: String, core: String, tile_type: String },
	Pass { pass: Value },
}

#[component]
fn App(page: Page) -> Element {
	let title = page.title.clone();
	let active = page.active.clone();
	let flash_error = page.flash.as_deref().is_some_and(|s| s.starts_with('!'));
	let flash_text = page.flash.clone().map(|s| s.trim_start_matches('!').to_string());
	rsx! {
		head {
			meta { charset: "utf-8" }
			meta { name: "viewport", content: "width=device-width, initial-scale=1" }
			title { "Apa Kan · {title}" }
			style { dangerous_inner_html: CSS }
		}
		body {
			div { class: "shell",
				div { class: "top",
					a { class: "brand", href: "/", "Apa Kan admin" }
					nav {
						if page.signed_in {
							a { href: "/characters", class: if active == "characters" { "active" } else { "" }, "Characters" }
							a { href: "/tiles", class: if active == "tiles" { "active" } else { "" }, "Tiles" }
							a { href: "/pass", class: if active == "pass" { "active" } else { "" }, "Version" }
							a { href: "/logout", "Sign out" }
						}
					}
				}
				if let Some(flash) = flash_text {
					p { class: if flash_error { "err" } else { "ok" }, "{flash}" }
				}
				match page.body.clone() {
					Body::Login { error } => rsx! { Login { error } },
					Body::Characters { rows } => rsx! { Characters { rows } },
					Body::Character { id, detail, vocab, preview } => rsx! { CharacterEdit { id, detail, vocab, preview } },
					Body::Tiles { rows, character, core, tile_type } => rsx! { Tiles { rows, character, core, tile_type } },
					Body::Pass { pass } => rsx! { PassPanel { pass } },
				}
			}
		}
	}
}

#[component]
fn Login(error: Option<String>) -> Element {
	rsx! {
		div { class: "paper",
			h1 { "Sign in" }
			p { class: "muted",
				"Admin is an ordinary account. Paste the 32-byte ML-DSA genesis seed as hex. No password, no SSO. The seed stays in this admin process and never goes to the game server."
			}
			form { method: "post", action: "/login",
				label { "Seed hex" }
				textarea { name: "seed_hex", rows: "3", style: "width:100%", }
				if let Some(err) = error {
					p { class: "err", "{err}" }
				}
				p { class: "row", button { "Sign a challenge" } }
			}
		}
	}
}

#[component]
fn Characters(rows: Value) -> Element {
	let list = rows.as_array().cloned().unwrap_or_default();
	rsx! {
		div { class: "paper",
			h1 { "Characters" }
			form { class: "row", method: "post", action: "/characters",
				input { r#type: "text", name: "character_id", placeholder: "kebab-id" }
				input { r#type: "text", name: "display_name", placeholder: "Display name" }
				button { "Add character" }
			}
		}
		div { class: "grid",
			for ch in list {
				{
					let id = ch["character_id"].as_str().unwrap_or("").to_string();
					let name = ch["display_name"].as_str().unwrap_or("").to_string();
					rsx! {
						a { class: "card", href: "/characters/{id}",
							if ch["has_image"] == true {
								img { src: "/proxy/characters/{id}/image", alt: "{name}" }
							}
							strong { "{name}" }
							div { class: "muted mono", "{id}" }
						}
					}
				}
			}
		}
	}
}

#[component]
fn CharacterEdit(id: String, detail: Value, vocab: Value, preview: Value) -> Element {
	let name = detail["display_name"].as_str().unwrap_or(&id).to_string();
	let embedding = detail["embedding"].clone();
	let vocab_list = vocab.as_array().cloned().unwrap_or_default();
	let neighbors = preview.as_array().cloned().unwrap_or_default();
	rsx! {
		div { class: "paper",
			h1 { "{name}" }
			p { class: "muted mono", "character id {id}" }
			form { method: "post", action: "/characters/{id}",
				label { "Display name" }
				input { r#type: "text", name: "display_name", value: "{name}" }
				button { class: "secondary", "Save name" }
			}
			form { method: "post", action: "/characters/{id}/image", enctype: "multipart/form-data", class: "row",
				input { r#type: "file", name: "image", accept: "image/png" }
				button { class: "secondary", "Upload PNG" }
			}
		}
		div { class: "paper",
			h2 { "Attributes" }
			p { class: "muted",
				"Omitted is no opinion. Negative values repel. An all-zero row is rejected."
			}
			form { method: "post", action: "/characters/{id}/embedding",
				div { class: "slider-grid",
					for entry in vocab_list {
						{
							let key = entry["key"].as_str().unwrap_or("").to_string();
							let label = entry["label"].as_str().unwrap_or("").to_string();
							let current = weight_for(&embedding, &key);
							rsx! {
								label { "{label}" }
								input {
									r#type: "range",
									name: "{key}",
									min: "-2",
									max: "2",
									step: "0.1",
									value: "{current}",
								}
								span { class: "mono muted", "{current}" }
							}
						}
					}
				}
				p { class: "row",
					button { "Save draft attributes" }
				}
			}
		}
		div { class: "paper",
			h2 { "Nearest (draft)" }
			if neighbors.is_empty() {
				p { class: "muted", "No neighborhood yet." }
			} else {
				ul {
					for hit in neighbors {
						{
							let cid = hit["character_id"].as_str().unwrap_or("").to_string();
							let cosine = hit["cosine"].as_f64().unwrap_or(0.0);
							let cosine_s = format!("{cosine:.3}");
							rsx! { li { a { href: "/characters/{cid}", "{cid}" } " · cosine {cosine_s}" } }
						}
					}
				}
			}
		}
	}
}

fn weight_for(embedding: &Value, key: &str) -> f32 {
	if embedding.is_null() {
		return 0.0;
	}
	let parts: Vec<&str> = key.split('.').collect();
	match parts.as_slice() {
		["color", c] => embedding["color"][c].as_f64().unwrap_or(0.0) as f32,
		["type", t] => embedding["type"][t].as_f64().unwrap_or(0.0) as f32,
		["emblem", e] => embedding["emblem"][e].as_f64().unwrap_or(0.0) as f32,
		["marks", "n_rings"] => embedding["marks"]["n_rings"].as_f64().unwrap_or(0.0) as f32,
		["marks", "dots"] => embedding["marks"]["dots"].as_f64().unwrap_or(0.0) as f32,
		["marks", "capture", c] => embedding["marks"]["capture"][c].as_f64().unwrap_or(0.0) as f32,
		_ => 0.0,
	}
}

#[component]
fn Tiles(rows: Value, character: String, core: String, tile_type: String) -> Element {
	let list = rows.as_array().cloned().unwrap_or_default();
	rsx! {
		div { class: "paper",
			h1 { "Issued tiles" }
			form { class: "row", method: "get", action: "/tiles",
				input { r#type: "text", name: "character", value: "{character}", placeholder: "character id" }
				input { r#type: "text", name: "core_tile", value: "{core}", placeholder: "core tile id" }
				input { r#type: "text", name: "tile_type", value: "{tile_type}", placeholder: "tile type id" }
				button { class: "secondary", "Filter" }
			}
		}
		div { class: "paper",
			table {
				thead {
					tr {
						th { "issued id" }
						th { "character" }
						th { "core tile" }
						th { "tile type" }
						th { "pass" }
					}
				}
				tbody {
					for t in list {
						{
							let instance = t["instance_id"].as_u64().unwrap_or(0);
							let ch = t["character_id"].as_str().unwrap_or("").to_string();
							let core_id = t["core_tile_id"].as_str().unwrap_or("").to_string();
							let type_id = t["tile_type_id"].as_str().unwrap_or("").to_string();
							let ver = t["pass_version"].as_u64().unwrap_or(0);
							let core_short = short_id(&core_id);
							let type_short = short_id(&type_id);
							rsx! {
								tr {
									td { class: "mono", "{instance}" }
									td { a { href: "/tiles?character={ch}", "{ch}" } }
									td { a { href: "/tiles?core_tile={core_id}", class: "mono", "{core_short}" } }
									td { a { href: "/tiles?tile_type={type_id}", class: "mono", "{type_short}" } }
									td { "{ver}" }
								}
							}
						}
					}
				}
			}
		}
	}
}

#[component]
fn PassPanel(pass: Value) -> Element {
	let draft = pass["draft_version"].as_u64().unwrap_or(0);
	let published = pass["published_version"].as_u64().unwrap_or(0);
	let tau = pass["tau"].as_f64().unwrap_or(0.32);
	rsx! {
		div { class: "paper",
			h1 { "Pass version" }
			p { "Draft {draft} · published {published}" }
			p { class: "muted",
				"Edits write a new draft membership. Publish points generation at the draft and bumps the pass version. Rows are not copied."
			}
			form { method: "post", action: "/pass/config", class: "row",
				label { "τ" }
				input { r#type: "number", name: "tau", step: "0.01", min: "0", max: "1", value: "{tau}" }
				button { class: "secondary", "Save τ" }
			}
			form { method: "post", action: "/pass/publish",
				button { class: "green", "Publish draft" }
			}
		}
	}
}

fn short_id(id: &str) -> String {
	if id.len() <= 12 {
		id.to_string()
	} else {
		format!("{}…", &id[..12])
	}
}

pub fn render(page: Page) -> String {
	let mut vdom = VirtualDom::new_with_props(App, AppProps { page });
	vdom.rebuild_in_place();
	format!("<!doctype html>\n<html lang=\"en\">\n{}</html>", dioxus_ssr::render(&vdom))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn login_page_renders_symbolic_markup() {
		let html = render(Page {
			title: "Sign in".into(),
			active: "login".into(),
			signed_in: false,
			flash: None,
			body: Body::Login { error: None },
		});
		assert!(html.contains("Apa Kan admin"));
		assert!(html.contains("Sign a challenge"));
		assert!(html.contains("#f4eee2"));
		assert!(!html.contains("Berkeley Mono"));
	}
}
