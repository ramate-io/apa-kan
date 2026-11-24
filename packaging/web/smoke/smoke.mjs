// Headless-browser smoke test for the Apa Kan web bundle.
//
// Normally run by the Kestrel orchestration (`cargo run -p apa-kan-smoke`),
// which builds nothing itself: it serves dist/web (and the API server when
// hosted), waits for readiness, runs this script, then tears everything down.
//
// Env:
//   SMOKE_URL     base URL of the served bundle (required)
//   SMOKE_OUT     directory for screenshots and console.log (default smoke-out)
//   SMOKE_HOSTED  "1" to also run the hosted create/join/move flow (needs the
//                 bundle configured with APAKAN_API_URL and the server up)
//
// The canvas has no DOM, so the game publishes a JSON snapshot on
// #bevy-canvas[data-apakan-smoke] when the URL has ?smoke (see
// apa-kan/game/src/ui/smoke.rs). We read button rects and legal-move click
// points from it and click with real mouse events.
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";

const BASE = process.env.SMOKE_URL;
const OUT = path.resolve(process.env.SMOKE_OUT || "smoke-out");
const HOSTED = process.env.SMOKE_HOSTED === "1";
if (!BASE) {
	console.error("SMOKE_URL is required");
	process.exit(2);
}
fs.mkdirSync(OUT, { recursive: true });

const logLines = [];
const failures = [];
const wakeups = [];
const IGNORED = [/favicon\.ico/, /fonts\.(googleapis|gstatic)\.com/];

function log(msg) {
	const line = `[smoke] ${msg}`;
	console.log(line);
	logLines.push(line);
}

function smokeUrl(raw) {
	const u = new URL(raw, BASE);
	u.searchParams.set("smoke", "1");
	return u.toString();
}

function watch(page, name) {
	page.on("console", (m) => {
		const text = m.text();
		logLines.push(`[${name}] console.${m.type()}: ${text}`);
		if (IGNORED.some((re) => re.test(text))) return;
		if (/apa-kan: frame loop idle/.test(text)) wakeups.push(`${name}: ${text}`);
		if (m.type() === "error" || /panicked at|RuntimeError|unreachable executed/.test(text)) {
			failures.push(`${name}: console ${m.type()}: ${text}`);
		}
	});
	page.on("pageerror", (e) => {
		logLines.push(`[${name}] pageerror: ${e.message}`);
		failures.push(`${name}: page error: ${e.message}`);
	});
	page.on("requestfailed", (r) => {
		const url = r.url();
		logLines.push(`[${name}] requestfailed: ${url} ${r.failure()?.errorText}`);
		if (!IGNORED.some((re) => re.test(url))) failures.push(`${name}: request failed: ${url}`);
	});
}

async function snap(page) {
	const [raw, tick] = await page.evaluate(() => [
		document.getElementById("bevy-canvas")?.getAttribute("data-apakan-smoke"),
		window.__apakanTick,
	]);
	if (!raw) return null;
	const s = JSON.parse(raw);
	s.tick = tick; // bumped every Bevy update, unlike the snapshot (every 10 frames)
	return s;
}

async function waitSnap(page, pred, what, timeout = 60_000) {
	const end = Date.now() + timeout;
	let last = null;
	let seenFrame = -1;
	let seenAt = Date.now();
	while (Date.now() < end) {
		if (failures.length) throw new Error(`aborting on errors while waiting for ${what}`);
		last = await snap(page);
		if (last && pred(last)) return last;
		if (last && last.tick !== seenFrame) {
			seenFrame = last.tick;
			seenAt = Date.now();
		} else if (Date.now() - seenAt > 15_000) {
			// The game must keep ticking without input (bot turns, clock, hosted polling).
			// A frozen frame counter is a hang a player would see: fail, don't nudge.
			const env = await page.evaluate(() => `visibility=${document.visibilityState} focus=${document.hasFocus()} active=${document.activeElement?.id || document.activeElement?.tagName}`).catch(() => "?");
			const stack = await mainThreadStack(page);
			throw new Error(`frame stall: no Bevy update for 15s at tick ${seenFrame} (${env}) while waiting for ${what}${stack}`);
		}
		await page.waitForTimeout(150);
	}
	const stack = await mainThreadStack(page);
	throw new Error(`timed out waiting for ${what}; last snapshot: ${JSON.stringify(last)?.slice(0, 600)}${stack}`);
}

// When the game stops producing frames, pause the page's main thread over CDP and
// report where it is stuck (wasm function names survive in the bundle).
async function mainThreadStack(page) {
	try {
		const cdp = await page.context().newCDPSession(page);
		await cdp.send("Debugger.enable");
		const paused = new Promise((resolve) => cdp.once("Debugger.paused", resolve));
		await cdp.send("Debugger.pause");
		const ev = await Promise.race([paused, new Promise((r) => setTimeout(() => r(null), 5000))]);
		if (!ev) return "\nmain thread: idle (no pause within 5s)";
		const frames = ev.callFrames.slice(0, 40).map((f) => `    at ${f.functionName || "<anon>"} (${f.url.split("/").pop()}:${f.location.lineNumber})`);
		await cdp.send("Debugger.resume").catch(() => {});
		await cdp.detach().catch(() => {});
		return `\nmain thread stack:\n${frames.join("\n")}`;
	} catch (e) {
		return `\n(could not sample the main thread: ${e.message})`;
	}
}

async function shot(page, name) {
	const file = path.join(OUT, `${name}.png`);
	await page.screenshot({ path: file });
	log(`screenshot ${file}`);
}

async function load(page, url, name) {
	await page.goto(smokeUrl(url), { waitUntil: "load" });
	// A 40+ MB wasm compiles on the page's main thread; next to a running game on a
	// small CI runner that can take well over 30s, so wait on the loader, then the canvas.
	await page.waitForSelector("#status.hidden", { state: "attached", timeout: 240_000 });
	await page.waitForSelector("#bevy-canvas", { state: "visible", timeout: 30_000 });
	const box = await page.locator("#bevy-canvas").boundingBox();
	if (!box || box.width < 100 || box.height < 100) throw new Error(`${name}: canvas missing or tiny: ${JSON.stringify(box)}`);
	const gl = await page.evaluate(() => {
		const c = document.createElement("canvas");
		return !!c.getContext("webgl2");
	});
	if (!gl) throw new Error(`${name}: WebGL2 unavailable in this browser`);
	return waitSnap(page, () => true, `${name}: first smoke snapshot`);
}

async function clickButton(page, label) {
	const s = await waitSnap(
		page,
		(s) => s.buttons.some((b) => b.label.trim() === label || b.label.startsWith(label)),
		`button "${label}"`,
		20_000,
	);
	const b = s.buttons.find((b) => b.label.trim() === label) || s.buttons.find((b) => b.label.startsWith(label));
	await page.mouse.click(b.x, b.y);
	log(`clicked "${b.label}" at ${Math.round(b.x)},${Math.round(b.y)}`);
	await page.waitForTimeout(250);
}

// Click one legal human action (setup pick, confirm, or a board move).
async function humanAction(page, name, timeout = 90_000) {
	const end = Date.now() + timeout;
	let s = null;
	let escFrame = -1;
	for (;;) {
		s = await waitSnap(page, (n) => n.screen === "match", `${name}: match`, 30_000);
		if (s.match.over || s.match.clicks.length > 0) break;
		// A half-made selection hides the next move; Esc clears it (once per fresh snapshot,
		// because Esc with nothing selected leaves the match).
		if (s.match.selection && s.match.selection !== "None" && s.frame > escFrame + 20) {
			log(`${name}: clearing selection ${s.match.selection}`);
			await page.keyboard.press("Escape");
			escFrame = s.frame;
		}
		if (Date.now() > end) throw new Error(`timed out waiting for ${name}: human turn; last snapshot: ${JSON.stringify(s).slice(0, 600)}`);
		await page.waitForTimeout(150);
	}
	if (s.match.over) return false;
	const key = (n) => JSON.stringify([n.match?.turn, n.match?.log, n.match?.setup, n.match?.clicks]);
	const before = key(s);
	log(`${name}: clicking ${s.match.clicks.map(([x, y]) => `${Math.round(x)},${Math.round(y)}`).join(" -> ")}`);
	let frame = s.frame;
	for (const [x, y] of s.match.clicks) {
		await page.mouse.click(x, y);
		// One click per rendered frame at least: SwiftShader can run at a few fps, and
		// two clicks inside one frame collapse into a single press at the last position.
		frame = (await waitSnap(page, (n) => n.frame > frame, `${name}: next frame`, 60_000)).frame;
	}
	// Snapshots are written every 10 frames; never act on one taken before the click landed.
	const fresh = await waitSnap(page, (n) => n.frame > frame, `${name}: fresh snapshot`, 60_000);
	const after = key(fresh) !== before ? fresh : await waitSnap(page, (n) => n.screen !== "match" || key(n) !== before, `${name}: response to click`, 45_000);
	if (after.match?.warning) log(`${name}: warning after click: ${after.match.warning}`);
	return true;
}

async function offlineBotGame(browser) {
	const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
	const page = await ctx.newPage();
	watch(page, "offline");
	await load(page, "/", "offline");
	await shot(page, "01-home");
	await clickButton(page, "Head-to-Head");
	await waitSnap(page, (s) => s.buttons.some((b) => b.label.startsWith("Play")), "Head-to-Head page");
	await clickButton(page, "Bot");
	await shot(page, "02-head-to-head");
	await clickButton(page, "Play");
	await waitSnap(page, (s) => s.screen === "match", "match screen");
	// Setup picks + confirm, then a few board moves; the bot answers between them.
	let setupClicks = 0;
	while ((await snap(page))?.match?.setup && setupClicks < 40) {
		const s = await snap(page);
		if (s.match.clicks.length === 0) {
			await page.waitForTimeout(200);
			continue;
		}
		await humanAction(page, "setup");
		setupClicks++;
	}
	const inPlay = await waitSnap(page, (s) => s.screen === "match" && !s.match.setup, "setup finished", 60_000);
	log(`setup done after ${setupClicks} clicks; turn ${inPlay.match.turn}`);
	await shot(page, "03-setup-done");
	const startTurn = inPlay.match.turn;
	let moves = 0;
	for (; moves < 3; moves++) {
		if (!(await humanAction(page, `move ${moves + 1}`))) break;
	}
	const s = await waitSnap(page, (s) => s.match.turn > startTurn + 1 || s.match.over, "bot reply", 60_000);
	log(`made ${moves} moves; turn ${s.match.turn}; log: ${s.match.log}`);
	if (moves < 1) throw new Error("could not make a single move against the bot");
	await shot(page, "04-offline-moves");
	// Leave (Esc returns to the shell when the match came from it), then Settings.
	await page.keyboard.press("Escape");
	await page.waitForTimeout(150);
	await page.keyboard.press("Escape");
	await waitSnap(page, (s) => s.screen === "shell", "back to shell", 20_000);
	const home = await waitSnap(page, (s) => s.buttons.some((b) => b.label === "Settings" || b.label.startsWith("Home") || b.label.startsWith("Back")), "shell nav");
	if (!home.buttons.some((b) => b.label === "Settings")) {
		await clickButton(page, home.buttons.find((b) => b.label.startsWith("Home") || b.label.startsWith("Back")).label);
	}
	await clickButton(page, "Settings");
	await waitSnap(page, (s) => s.buttons.some((b) => b.label.startsWith("Open license")), "Settings page");
	await shot(page, "05-settings");
	await ctx.close();
}

async function hostedJoin(browser) {
	const a = await browser.newContext({ viewport: { width: 1100, height: 760 }, permissions: ["clipboard-read", "clipboard-write"] });
	const b = await browser.newContext({ viewport: { width: 1100, height: 760 } });
	const pa = await a.newPage();
	const pb = await b.newPage();
	watch(pa, "host");
	watch(pb, "guest");
	let invite = null;
	pa.on("console", (m) => {
		// e.g. "Apa Kan invite (seat 2): http://host/#/game/<id>?seat=2&token=…"
		const hit = /invite/i.test(m.text()) && m.text().match(/https?:\/\/\S*\/game\/\S+/);
		if (hit && !invite) invite = hit[0];
	});
	await load(pa, "/", "host");
	await clickButton(pa, "Head-to-Head");
	await clickButton(pa, "Online");
	await shot(pa, "10-host-online");
	await clickButton(pa, "Play");
	const end = Date.now() + 30_000;
	while (!invite && Date.now() < end) {
		await pa.waitForTimeout(200);
	}
	if (!invite) throw new Error("host never produced a /game/<id> invite link");
	log(`invite ${invite}`);
	await waitSnap(pa, (s) => s.screen === "match", "host match screen");
	await shot(pa, "11-host-waiting");
	await load(pb, invite, "guest");
	await waitSnap(pb, (s) => s.screen === "match", "guest match screen");
	await shot(pb, "12-guest-joined");
	// Whoever has clicks moves; the other side must see the turn advance.
	let moved = 0;
	const deadline = Date.now() + 120_000;
	while (moved < 2 && Date.now() < deadline) {
		const [sa, sb] = [await snap(pa), await snap(pb)];
		const mover = sa?.match?.clicks?.length ? pa : sb?.match?.clicks?.length ? pb : null;
		if (!mover) {
			await pa.waitForTimeout(250);
			continue;
		}
		const other = mover === pa ? pb : pa;
		const was = (await snap(other)).match;
		await humanAction(mover, mover === pa ? "host move" : "guest move");
		await waitSnap(other, (s) => JSON.stringify([s.match.turn, s.match.log, s.match.setup]) !== JSON.stringify([was.turn, was.log, was.setup]), "move to reach the other context", 30_000);
		moved++;
		log(`hosted: ${mover === pa ? "host" : "guest"} moved and the other side saw it`);
	}
	if (moved < 2) throw new Error(`hosted: only ${moved} moves crossed between contexts`);
	await shot(pa, "13-host-after-moves");
	await shot(pb, "14-guest-after-moves");
	await a.close();
	await b.close();
}

const browser = await chromium.launch({
	headless: true,
	args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
let code = 0;
try {
	await offlineBotGame(browser);
	if (HOSTED) await hostedJoin(browser);
} catch (e) {
	failures.push(String(e?.stack || e));
} finally {
	await browser.close();
	fs.writeFileSync(path.join(OUT, "console.log"), logLines.join("\n") + "\n");
	if (failures.length) {
		console.error("SMOKE FAILED:\n" + failures.map((f) => `  - ${f}`).join("\n"));
		code = 1;
	} else {
		if (wakeups.length) log(`note: the host page's frame-loop watchdog woke Bevy ${wakeups.length}x:\n  ${wakeups.join("\n  ")}`);
		log(`SMOKE PASSED (${HOSTED ? "offline + hosted" : "offline"}); artifacts in ${OUT}`);
	}
}
process.exit(code);
