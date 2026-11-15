/// Runs JavaScript code for the `js` command of nux (see prelude.nu).
///
///     node js_runner.mjs <code>     with the input as NUON on stdin
///
/// The input is available to the code as `data`, and `Nu.FileSize` makes a file size. Needs
/// Node 26 or newer, for Temporal. The value of the last expression is written to stdout as
/// NUON (see nuon.mjs); a promise is awaited first. Code that uses `await` at the
/// top level is run as the body of an async function instead, and then gives its value with
/// `return`. `require` and `import()` look for packages from the working directory.
/// Anything the code logs goes to stderr, to keep stdout for the value.

import { Console } from "node:console";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";
import vm from "node:vm";
import { dumps, loads, Nu } from "./nuon.mjs";

const CODE_NAME = "<js>";

/// Node announces that the loader used for `import()` below is experimental, on every run.
const emit_warning = process.emitWarning;
process.emitWarning = (warning, ...rest) => {
  if (String(warning?.message ?? warning).includes("USE_MAIN_CONTEXT_DEFAULT_LOADER")) return;
  return emit_warning.call(process, warning, ...rest);
};

const read_stdin = async () => {
  process.stdin.setEncoding("utf8");
  let text = "";
  for await (const chunk of process.stdin) text += chunk;
  return text;
};

/// Packages are looked up as if the code were a file in the working directory.
const require_from_cwd = createRequire(path.join(process.cwd(), CODE_NAME));

const import_from_cwd = (specifier) => {
  if (specifier.startsWith("node:") || /^[a-z][a-z0-9+.-]*:/i.test(specifier)) return import(specifier);
  if (specifier.startsWith(".") || path.isAbsolute(specifier)) {
    return import(pathToFileURL(path.resolve(specifier)).href);
  }
  try {
    return import(pathToFileURL(require_from_cwd.resolve(specifier)).href);
  } catch {
    /// A package that can only be imported, not required: let Node look for it.
    return import(specifier);
  }
};

/// Newer Node versions can load `import()` in a script themselves, resolving from the working
/// directory (and refuse a callback without a flag); older ones need the callback.
const IMPORTER = vm.constants?.USE_MAIN_CONTEXT_DEFAULT_LOADER ?? import_from_cwd;

const compile = (source, line_offset = 0) =>
  new vm.Script(source, { filename: CODE_NAME, lineOffset: line_offset, importModuleDynamically: IMPORTER });

const run = (code) => {
  /// `{a: 1}` alone would be read as a block, not as an object.
  if (code.trimStart().startsWith("{")) {
    try {
      return compile(`(${code}\n)`).runInThisContext();
    } catch (error) {
      if (!(error instanceof SyntaxError)) throw error;
    }
  }
  let script;
  try {
    script = compile(code);
  } catch (error) {
    if (!(error instanceof SyntaxError) || !/\bawait\b|\breturn\b/i.test(error.message)) throw error;
    /// Top-level `await` (or `return`): run it as the body of an async function.
    script = compile(`(async () => {\n${code}\n})()`, -1);
  }
  return script.runInThisContext();
};

/// The code's own frames (not this runner's), then `Name: message` as the last line.
const report = (error) => {
  if (!(error instanceof Error)) return `Thrown: ${typeof error === "string" ? error : dumps(error)}`;
  const lines = String(error.stack ?? "").split("\n");
  const title = `${error.name}: ${error.message}`;
  /// A syntax error starts with the place in the code: "<js>:1", the line, a marker.
  const title_at = lines.findIndex((line) => line.startsWith(`${error.name}:`));
  const place = error instanceof SyntaxError && title_at > 0 ? lines.slice(0, title_at).filter((line) => line.trim() !== "") : [];
  const frames = lines.filter((line) => /^\s+at /.test(line) && line.includes(CODE_NAME));
  const [first_line, ...more_lines] = title.split("\n");
  return [...place, ...frames, ...more_lines, first_line].join("\n");
};

const main = async () => {
  if (typeof Temporal === "undefined") {
    process.stderr.write(
      `js needs Node 26 or newer (durations are Temporal.Duration); this is Node ${process.version} at ${process.execPath}. ` +
        `Put a newer node first on PATH, or set NUX_NODE to one.\n`,
    );
    process.exit(1);
  }
  const code = process.argv[2] ?? "";
  const raw = await read_stdin();
  let text;
  try {
    Object.assign(globalThis, {
      data: raw.trim() === "" ? null : loads(raw),
      require: require_from_cwd,
      Nu,
      console: new Console(process.stderr, process.stderr),
    });
    text = dumps(await run(code));
  } catch (error) {
    process.stderr.write(report(error) + "\n");
    process.exit(1);
  }
  process.stdout.write(text, () => process.exit(0));
};

main();
