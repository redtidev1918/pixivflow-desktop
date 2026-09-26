import test from "node:test";
import assert from "node:assert/strict";
import { deriveView, STATES } from "./state.js";

// Safety floor: deriveView must NEVER throw and must always produce a valid
// state key, so no input can blank the page.
test("deriveView never throws / never blanks for arbitrary input", () => {
  const inputs = [undefined, null, {}, { state: "bogus" }, 42, "x", [], { state: "" }];
  for (const x of inputs) {
    const v = deriveView(x);
    assert.equal(typeof v.keyword, "string");
    assert.ok(STATES[v.keyword], `falls back to a valid state (got ${v.keyword})`);
  }
});

test("starting renders the boot UI (Waiting…) without throwing", () => {
  const v = deriveView({ state: "starting", port: 3000, pid: null });
  assert.equal(v.keyword, "starting");
  assert.equal(v.badgeText, "Starting");
  assert.equal(v.port, "3000");
  assert.equal(v.health, "Waiting…");
  assert.equal(v.showError, false);
});

test("running + healthy renders Running / Healthy", () => {
  const v = deriveView({ state: "running", healthy: true, port: 3000, pid: 123 });
  assert.equal(v.keyword, "running");
  assert.equal(v.badgeText, "Running");
  assert.equal(v.pid, "123");
  assert.equal(v.health, "Healthy");
  assert.equal(v.showError, false);
});

test("running + not healthy shows 'Waiting for backend…' (health stall, no blank)", () => {
  const v = deriveView({ state: "running", healthy: false, port: 3000, pid: 7 });
  assert.equal(v.line, "Waiting for backend…");
});

test("stopped renders Stopped", () => {
  const v = deriveView({ state: "stopped" });
  assert.equal(v.badgeText, "Stopped");
  assert.equal(v.line, "Backend stopped");
});

test("start failure renders 'Backend failed / Reason' error surface", () => {
  const v = deriveView({ state: "error", message: "spawn mock backend: No such file or directory" });
  assert.equal(v.keyword, "error");
  assert.equal(v.showError, true);
  assert.equal(v.errorTitle, "Backend failed");
  assert.equal(v.errorLabel, "Reason:");
});

test("missing runtime renders 'PixivFlow runtime not found'", () => {
  const v = deriveView({ state: "error", message: "PixivFlow runtime not found" });
  assert.equal(v.showError, true);
  assert.equal(v.errorTitle, "PixivFlow runtime not found");
});
