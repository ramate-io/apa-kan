//! Compile the thin iOS AppDelegate into the Bevy binary.
//!
//! winit 0.30.13 calls `UIApplicationMain` without a principal/delegate class so
//! a custom AppDelegate can own URL delivery. The wrapper lives in
//! `packaging/ios/native` and is linked into `apa-kan` when targeting iOS.

fn main() {
	println!("cargo:rerun-if-changed=../../packaging/ios/native");
	let target = std::env::var("TARGET").unwrap_or_default();
	if !target.contains("apple-ios") {
		return;
	}

	let mut build = cc::Build::new();
	build
		.file("../../packaging/ios/native/ApaKanAppDelegate.m")
		.flag("-fobjc-arc")
		.flag("-fmodules")
		.define("APAKAN_IOS_GLUE", Some("1"));
	build.compile("apakan_ios_glue");
	println!("cargo:rustc-link-lib=framework=UIKit");
	println!("cargo:rustc-link-lib=framework=Foundation");
	// Keep the AppDelegate category's `+load` from being stripped.
	println!("cargo:rustc-link-arg=-ObjC");
}
