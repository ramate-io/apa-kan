//! Web smoke test as a Kestrel orchestration.
//!
//! ```text
//! cargo run -p apa-kan-smoke -- [--build] [--hosted] [--out DIR]
//! ```
//!
//! 1. Optional `--build`: run `packaging/scripts/build-web.sh` (pinned
//!    wasm-bindgen) to produce `dist/web`.
//! 2. Build `apa-kan-server` (release) and install the Playwright driver in
//!    `packaging/web/smoke` if it is missing.
//! 3. Spawn the API server and a static server for the bundle as Kestrel
//!    processes, each gated on a readiness line from its own output.
//! 4. Run the headless-browser smoke (`packaging/web/smoke/smoke.mjs`):
//!    offline bot game, Settings, and with `--hosted` a create/join/move flow
//!    across two browser contexts.
//! 5. Tear down: abort the process tasks, which kills the children.
//!
//! Screenshots and `console.log` land in `--out` (default `smoke-out/`).

use std::ffi::OsString;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use kestrel::fulfill::custom::{ContainsLine, Custom};
use kestrel::fulfill::Fulfill;
use kestrel::process::command::Command;
use kestrel::process::{Pipe, ProcessError, ProcessOperations};
use kestrel::State;

type Error = Box<dyn std::error::Error + Send + Sync>;

const READY_TIMEOUT: Duration = Duration::from_secs(60);

struct Args {
	build: bool,
	hosted: bool,
	out: PathBuf,
	bundle: PathBuf,
}

fn parse_args(root: &Path) -> Result<Args, Error> {
	let mut args = Args {
		build: false,
		hosted: false,
		out: root.join("smoke-out"),
		bundle: root.join("dist/web"),
	};
	let mut it = std::env::args().skip(1);
	while let Some(a) = it.next() {
		match a.as_str() {
			"--build" => args.build = true,
			"--hosted" => args.hosted = true,
			"--out" => args.out = root.join(it.next().ok_or("--out needs a directory")?),
			"--bundle" => args.bundle = root.join(it.next().ok_or("--bundle needs a directory")?),
			"-h" | "--help" => {
				println!("usage: apa-kan-smoke [--build] [--hosted] [--out DIR] [--bundle DIR]");
				std::process::exit(0);
			}
			other => return Err(format!("unknown argument {other}").into()),
		}
	}
	Ok(args)
}

fn repo_root() -> PathBuf {
	// apa-kan/smoke -> repo root
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.map(Path::to_path_buf)
		.unwrap_or_else(|| PathBuf::from("."))
}

fn free_port() -> Result<u16, Error> {
	Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

fn command<P, I, S>(program: P, args: I, dir: &Path) -> Command
where
	P: AsRef<std::ffi::OsStr> + Send,
	I: IntoIterator<Item = S> + Send,
	S: AsRef<std::ffi::OsStr>,
{
	Command::line(program, args, Some(dir), false, vec![], vec![])
}

/// Run a one-shot step to completion.
async fn step(name: &str, cmd: Command) -> Result<(), Error> {
	println!("[smoke] step: {name}");
	cmd.run().await.map_err(|e| format!("{name} failed: {e}"))?;
	Ok(())
}

type ProcTask = kestrel::Task<Result<String, ProcessError>>;

/// Spawn a long-running process and wait until its output contains `needle`.
async fn spawn_ready(name: &str, mut cmd: Command, needle: &str) -> Result<ProcTask, Error> {
	println!("[smoke] spawn: {name} (ready on {needle:?})");
	let ready = State::<String>::new();
	let fulfill = Custom::new(ready.write(), ContainsLine::new(needle));
	cmd.pipe(Pipe::STDOUT, fulfill.sender()?)?;
	cmd.pipe(Pipe::STDERR, fulfill.sender()?)?;
	let watcher = kestrel::task(fulfill.run());
	let mut proc = kestrel::task(cmd.run());
	let reader = ready.read();
	let outcome = tokio::select! {
		r = reader.wait_for(READY_TIMEOUT) => r.map(|_| ()).map_err(|e| format!("{name} not ready after {READY_TIMEOUT:?}: {e}").into()),
		exited = &mut proc.handle => Err(format!("{name} exited before it was ready: {exited:?}").into()),
	};
	watcher.abort();
	match outcome {
		Ok(()) => {
			println!("[smoke] ready: {name}");
			Ok(proc)
		}
		Err(e) => {
			proc.abort();
			Err(e)
		}
	}
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
	std::fs::create_dir_all(to)?;
	for entry in std::fs::read_dir(from)? {
		let entry = entry?;
		let dest = to.join(entry.file_name());
		if entry.file_type()?.is_dir() {
			copy_dir(&entry.path(), &dest)?;
		} else {
			std::fs::copy(entry.path(), dest)?;
		}
	}
	Ok(())
}

async fn orchestrate(root: &Path, args: &Args) -> Result<(), Error> {
	let smoke_dir = root.join("packaging/web/smoke");
	if args.build {
		step("build web bundle", command("bash", ["packaging/scripts/build-web.sh"], root)).await?;
	}
	if !args.bundle.join("index.html").is_file() {
		return Err(format!(
			"no bundle at {}; pass --build or run packaging/scripts/build-web.sh",
			args.bundle.display()
		)
		.into());
	}
	step(
		"build apa-kan-server",
		command("cargo", ["build", "--release", "--locked", "-p", "apa-kan-server"], root),
	)
	.await?;
	if !smoke_dir.join("node_modules/playwright").is_dir() {
		step("npm ci", command("npm", ["ci", "--no-audit", "--no-fund"], &smoke_dir)).await?;
	}
	if std::env::var_os("SMOKE_SKIP_BROWSER_INSTALL").is_none() {
		step(
			"playwright chromium",
			command("npx", ["--no-install", "playwright", "install", "chromium"], &smoke_dir),
		)
		.await?;
	}

	let _ = std::fs::remove_dir_all(&args.out);
	std::fs::create_dir_all(&args.out)?;
	let api_port = free_port()?;
	let web_port = free_port()?;
	let api_url = format!("http://127.0.0.1:{api_port}");
	let web_url = format!("http://127.0.0.1:{web_port}");

	// Stage the bundle with this run's API URL in the host page.
	let site = args.out.join("site");
	copy_dir(&args.bundle, &site)?;
	let mut configure = command(
		"sh",
		[
			OsString::from("packaging/scripts/configure-web-host.sh"),
			OsString::from("apa-kan/game/web/index.html"),
			site.join("index.html").into_os_string(),
		],
		root,
	);
	configure.env("APAKAN_API_URL", &api_url).env("APAKAN_PUBLIC_URL", &web_url);
	step("configure host page", configure).await?;

	let mut server = command(root.join("target/release/apa-kan-server"), [] as [&str; 0], root);
	server
		.env("PORT", api_port.to_string())
		.env("RUST_LOG", "info")
		.env("NO_COLOR", "1")
		.env("APA_KAN_DATABASE_URL", format!("sqlite:{}", args.out.join("server.db").display()));
	let server = spawn_ready("apa-kan-server", server, "apa-kan-server listening").await?;

	let web = command(
		"python3",
		[
			OsString::from("-u"),
			OsString::from("-m"),
			OsString::from("http.server"),
			OsString::from(web_port.to_string()),
			OsString::from("--bind"),
			OsString::from("127.0.0.1"),
			OsString::from("--directory"),
			site.into_os_string(),
		],
		root,
	);
	let web = match spawn_ready("web bundle server", web, "Serving HTTP on").await {
		Ok(t) => t,
		Err(e) => {
			kestrel::abort!(server);
			return Err(e);
		}
	};

	let mut browser = command("node", ["smoke.mjs"], &smoke_dir);
	browser
		.env("SMOKE_URL", &web_url)
		.env("SMOKE_API", &api_url)
		.env("SMOKE_OUT", &args.out)
		.env("SMOKE_HOSTED", if args.hosted { "1" } else { "0" });
	let result = step("headless-browser smoke", browser).await;

	println!("[smoke] teardown");
	let _ = kestrel::end!(server, web);
	result
}

#[tokio::main]
async fn main() -> ExitCode {
	let root = repo_root();
	let args = match parse_args(&root) {
		Ok(a) => a,
		Err(e) => {
			eprintln!("apa-kan-smoke: {e}");
			return ExitCode::from(2);
		}
	};
	match orchestrate(&root, &args).await {
		Ok(()) => {
			println!("[smoke] passed; artifacts in {}", args.out.display());
			ExitCode::SUCCESS
		}
		Err(e) => {
			eprintln!("[smoke] FAILED: {e}\n[smoke] artifacts in {}", args.out.display());
			ExitCode::FAILURE
		}
	}
}
