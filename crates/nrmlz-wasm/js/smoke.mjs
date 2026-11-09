// Smoke test of the WebAssembly build under bun or Node: `bun crates/nrmlz-wasm/js/smoke.mjs`.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, "..", "pkg");
const { normalize, normalizeWith, warmUp, version, Lexicon, Stream } = await import(join(pkg, "nrmlz_wasm.js"));

let failed = 0;
function eq(got, want, what) {
  if (got !== want) {
    failed++;
    console.error(`FAIL ${what}\n  want ${JSON.stringify(want)}\n  got  ${JSON.stringify(got)}`);
  } else {
    console.log(`ok   ${what}`);
  }
}

let t = performance.now();
warmUp();
const warm = performance.now() - t;

// the same six number sentences the browser runtime failed on, against the Python reference outputs
const cases = JSON.parse(readFileSync(join(here, "cases.json"), "utf8"));
for (const c of cases) {
  eq(normalizeWith("de", c.text, "reference"), c.expected, `reference: ${c.text}`);
  eq(normalize("de", c.text), c.expected.replaceAll("Komma,", "Komma"), `nrmlz: ${c.text}`);
}
eq(normalize("en", "It costs $5.50 and weighs 3 kg."), "It costs five dollars and fifty cents and weighs three kilograms.", "English");
eq(normalize("de", "Dichte in kg/m³."), "Dichte in Kilogramm pro Kubikmeter.", "unit without number");
eq(normalizeWith("de", "3,5 kg.", "nrmlz", true), "drei Komma, fünf Kilogramm.", "pause opt-in");
const finalLexicon = Lexicon.fromJson('{"terms":{"OK":"Okay 7"}}');
eq(finalLexicon.normalize("de", "OK!"), "Okay 7!", "lexicon is final");
eq(normalize("de", "Hier [en]5 books[/en]."), "Hier [en]five books[/en].", "code switching");

const lex = Lexicon.fromJson('{"terms": {"Kubernetes": "Kubernetis"}}');
eq(lex.normalize("de", "Kubernetes auf 3 Knoten."), "Kubernetis auf drei Knoten.", "lexicon before");
lex.setStage("after");
eq(lex.normalize("de", "Kubernetes 2"), "Kubernetis zwei.", "lexicon after");

const s = new Stream();
let streamed = "";
for (const ch of "Er kommt am 3. Mai um 9:05 Uhr. Dann z. B. weiter.") streamed += s.push(ch);
streamed += s.flush();
eq(streamed, "Er kommt am dritten Mai um neun Uhr fünf. Dann zum Beispiel weiter.", "stream");

let threw = false;
try { normalize("xx", "a"); } catch { threw = true; }
eq(threw, true, "unknown language throws");

const text = cases.map((c) => c.text).join(" ");
const n = 200;
t = performance.now();
for (let i = 0; i < n; i++) normalize("de", text);
const per = (performance.now() - t) / n / cases.length;
console.log(`nrmlz-wasm ${version()}: warm-up ${warm.toFixed(1)} ms, ${per.toFixed(3)} ms per sentence`);
if (failed) {
  console.error(`${failed} failed`);
  process.exit(1);
}

// The browser build (pkg-web, --target web) loads from bytes too.
const web = await import(join(here, "..", "pkg-web", "nrmlz_wasm.js"));
web.initSync({ module: readFileSync(join(here, "..", "pkg-web", "nrmlz_wasm_bg.wasm")) });
if (web.normalize("de", "am 3. Mai") !== "am dritten Mai.") {
  console.error("FAIL web build");
  process.exit(1);
}
console.log("ok   web build (initSync)");
// All committed, independently authored gold cases through the web bindings as well.
let gold = 0;
for (const lang of ["de", "en"]) {
  const path = join(here, "../../../tests/gold", `${lang}.tsv`);
  const inputs = [];
  for (const line of readFileSync(path, "utf8").split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const [cls, input, expected] = line.split("\t");
    if (cls.startsWith("gap:")) continue;
    const got = web.normalize(lang, input);
    if (got !== expected) { console.error(`FAIL web gold ${lang}/${cls}: ${input}`); process.exit(1); }
    gold++;
    inputs.push(input);
  }
  const begin = performance.now();
  for (let i = 0; i < 10; i++) for (const input of inputs) web.normalize(lang, input);
  console.log(`web ${lang}: ${((performance.now() - begin) / inputs.length / 10).toFixed(3)} ms/text (${inputs.length} gold inputs)`);
}
console.log(`ok   ${gold} web gold cases`);
