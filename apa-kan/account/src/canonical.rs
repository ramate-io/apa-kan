use serde::Serialize;

/// Serialize JSON with sorted object keys for stable hashing (simple recursive sort).
pub fn to_canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
	let v = serde_json::to_value(value)?;
	let sorted = sort_value(v);
	serde_json::to_string(&sorted)
}

fn sort_value(v: serde_json::Value) -> serde_json::Value {
	match v {
		serde_json::Value::Object(map) => {
			let mut keys: Vec<_> = map.keys().cloned().collect();
			keys.sort();
			let mut out = serde_json::Map::new();
			for k in keys {
				if let Some(child) = map.get(&k) {
					out.insert(k, sort_value(child.clone()));
				}
			}
			serde_json::Value::Object(out)
		}
		serde_json::Value::Array(arr) => {
			serde_json::Value::Array(arr.into_iter().map(sort_value).collect())
		}
		other => other,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Serialize;

	#[derive(Serialize)]
	struct Sample {
		z: u8,
		a: u8,
	}

	#[test]
	fn keys_sorted() {
		let s = Sample { z: 1, a: 2 };
		let j = to_canonical_json(&s).unwrap();
		assert_eq!(j, r#"{"a":2,"z":1}"#);
	}
}