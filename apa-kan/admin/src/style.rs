//! Visual language from symbolic.software (no Berkeley Mono).

pub const CSS: &str = r#"
@import url("https://fonts.googleapis.com/css2?family=Expletus+Sans:wght@500;600;700&display=swap");
:root {
  --bg: #f4eee2;
  --bg-alt: #ede5d4;
  --bg-deep: #e6dec9;
  --paper: #fbf7ec;
  --frame: #ebe3d0;
  --ink: #1a1a1a;
  --ink-soft: #2a2620;
  --muted: #5d574a;
  --rule: rgba(26,26,26,.18);
  --purple: #6e5883;
  --green: #3d6652;
  --orange: #a35c3a;
  --yellow: #ecc368;
  --on-yellow: #1a1a1a;
  --red: #a03d33;
  --radius: 4px;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1b1a18;
    --bg-alt: #23211e;
    --bg-deep: #1b1a18;
    --paper: #292621;
    --frame: #3a362f;
    --ink: #eee8dc;
    --ink-soft: #eee8dc;
    --muted: #b8ae9f;
    --rule: rgba(238,232,220,.18);
    --purple: #c2a8df;
    --green: #a3c9ae;
    --orange: #e9ab88;
    --yellow: #e8c778;
    --on-yellow: #1a1a1a;
    --red: #eb9c92;
  }
}
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--ink); }
body {
  font-family: "Iowan Old Style", Charter, Georgia, "Times New Roman", serif;
  line-height: 1.5;
  min-height: 100vh;
}
h1, h2, h3, .brand {
  font-family: "Expletus Sans", "Iowan Old Style", serif;
  font-weight: 600;
  letter-spacing: 0.01em;
}
code, .mono, input[type=text], input[type=number], textarea {
  font-family: ui-monospace, "SF Mono", "Cascadia Mono", "Segoe UI Mono", monospace;
}
a { color: var(--purple); }
hr { border: 0; border-top: 1px solid var(--rule); }
.shell { max-width: 1100px; margin: 0 auto; padding: 28px 22px 64px; }
.top {
  display: flex; justify-content: space-between; align-items: baseline;
  border-bottom: 1px solid var(--rule); padding-bottom: 12px; margin-bottom: 24px;
}
.brand { font-size: 22px; color: var(--ink-soft); text-decoration: none; }
nav a { margin-left: 16px; text-decoration: none; color: var(--muted); }
nav a.active, nav a:hover { color: var(--ink); }
.paper {
  background: var(--paper); border: 1px solid var(--frame); border-radius: var(--radius);
  padding: 18px 20px; margin-bottom: 16px;
}
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
.card {
  background: var(--bg-alt); border: 1px solid var(--frame); border-radius: var(--radius);
  padding: 12px; text-decoration: none; color: inherit; display: block;
}
.card img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: var(--radius); background: var(--bg-deep); }
.muted { color: var(--muted); font-size: 0.92em; }
.row { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
label { display: block; color: var(--muted); font-size: 0.85em; margin-bottom: 4px; }
input, textarea, select, button {
  border-radius: var(--radius); border: 1px solid var(--frame); background: var(--paper);
  color: var(--ink); padding: 8px 10px;
}
button, .btn {
  background: var(--purple); color: var(--paper); border: 0; cursor: pointer;
  font-family: "Expletus Sans", sans-serif;
}
button.secondary { background: var(--bg-alt); color: var(--ink); border: 1px solid var(--frame); }
button.green { background: var(--green); }
button.warn { background: var(--orange); }
.chip {
  display: inline-flex; gap: 6px; align-items: center; padding: 4px 8px;
  background: var(--bg-alt); border: 1px solid var(--frame); border-radius: var(--radius);
  margin: 0 6px 6px 0;
}
.chip.active { border-color: var(--purple); color: var(--purple); }
.err { color: var(--red); }
.ok { color: var(--green); }
table { width: 100%; border-collapse: collapse; font-size: 0.92em; }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--rule); }
.slider-grid { display: grid; grid-template-columns: 160px 1fr 64px; gap: 8px 12px; align-items: center; }
input[type=range] { width: 100%; accent-color: var(--purple); }
"#;
