// Preflight for `bun run build`: the wasm bindings are generated, not checked
// in, so a fresh clone fails deep inside tsc with a cryptic missing-module
// error. Fail early with the actual instruction instead.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const pkg = fileURLToPath(new URL("../src/wasm/flame_core.js", import.meta.url));
if (!existsSync(pkg)) {
  console.error(
    "src/wasm/ is missing — the Rust renderer has not been built.\n" +
      "Run `bun run wasm` first (requires Rust with the wasm32-unknown-unknown " +
      "target and wasm-pack).",
  );
  process.exit(1);
}
