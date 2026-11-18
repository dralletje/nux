// JSON on stdin -> TOON on stdout, with the reference implementation installed in the
// directory given as the first argument.
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const deps = process.argv[2];
const require = createRequire(join(deps, "package.json"));
const { encode } = await import(pathToFileURL(require.resolve("@toon-format/toon")).href);

let input = "";
for await (const chunk of process.stdin) input += chunk;
process.stdout.write(encode(JSON.parse(input)));
