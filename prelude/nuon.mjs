/// NUON, Nushell's data notation, for JavaScript: `loads` and `dumps`.
///
/// `loads` reads what Nushell's `to nuon` writes (not everything a person could type as
/// NUON); `dumps` writes what `from nuon` reads. Used by js_runner.mjs, so that values keep
/// their type on the way into JavaScript and back. The Python counterpart is nuon.py.
///
///     Nushell     JavaScript
///     datetime    Temporal.ZonedDateTime  (exact, in a time zone that is the datetime's UTC offset)
///     duration    Temporal.Duration       (exact)
///     filesize    FileSize                (below)
///     binary      Uint8Array (a Buffer)
///     record      object
///     table       array of objects
///     int         number, or BigInt when it does not fit a number exactly
///     range       array                   (whole numbers, with an end; also counting down or in
///                                          steps. Else {start, end, step, inclusive})
///
/// Temporal needs Node 26 or newer. Written as a datetime as well: a Date, and Temporal's
/// Instant, PlainDateTime and PlainDate (the last two are taken as UTC by Nushell).

class FileSizeValue {
  constructor(bytes) {
    const number = typeof bytes === "bigint" ? Number(bytes) : +bytes;
    if (!Number.isFinite(number)) throw new TypeError(`FileSize needs a number of bytes, got ${String(bytes)}`);
    /// Whole bytes.
    this.bytes = Math.round(number);
    Object.freeze(this);
  }
  /// Decimal units, as Nushell's kb, mb, gb.
  get kb() {
    return this.bytes / 1000;
  }
  get mb() {
    return this.bytes / 1000 ** 2;
  }
  get gb() {
    return this.bytes / 1000 ** 3;
  }
  /// Binary units, as Nushell's kib, mib, gib.
  get kib() {
    return this.bytes / 1024;
  }
  get mib() {
    return this.bytes / 1024 ** 2;
  }
  get gib() {
    return this.bytes / 1024 ** 3;
  }
  /// So that sizes compare and sort: `a.size > b.size`, `a.size - b.size`.
  valueOf() {
    return this.bytes;
  }
  toString() {
    return `${this.bytes}b`;
  }
  toJSON() {
    return this.bytes;
  }
}

/// A file size: `FileSize(1500)` or `new FileSize(1500)`, in bytes. Read it with `.bytes`,
/// `.kb`, `.mb`, `.gb` (and `.kib`, `.mib`, `.gib`). It goes back to Nushell as a filesize.
export function FileSize(bytes) {
  return new FileSizeValue(bytes);
}
FileSize.prototype = FileSizeValue.prototype;
/// So that a size says it is a FileSize, and `size.constructor` is the function above.
FileSizeValue.prototype.constructor = FileSize;

/// What the code gets as `Nu`.
export const Nu = Object.freeze({ FileSize });

export class NuonError extends Error {
  name = "NuonError";
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

/// A bare word ends at one of these (a colon does not end it: dates contain colons).
const WORD_END = new Set([..." \t\r\n,;[]{}"]);
const KEY_END = new Set([...WORD_END, ":"]);

const SECOND = 1_000_000_000;
const DURATION_NS = {
  ns: 1,
  us: 1_000,
  µs: 1_000,
  ms: 1_000_000,
  sec: SECOND,
  min: 60 * SECOND,
  hr: 3600 * SECOND,
  day: 86400 * SECOND,
  wk: 7 * 86400 * SECOND,
};
const FILESIZE_BYTES = {
  b: 1,
  kb: 1000,
  mb: 1000 ** 2,
  gb: 1000 ** 3,
  tb: 1000 ** 4,
  pb: 1000 ** 5,
  eb: 1000 ** 6,
  kib: 1024,
  mib: 1024 ** 2,
  gib: 1024 ** 3,
  tib: 1024 ** 4,
  pib: 1024 ** 5,
  eib: 1024 ** 6,
};

const INT = /^-?\d+$/;
const FLOAT = /^-?(\d+\.\d*|\.\d+|\d+)([eE][+-]?\d+)?$/;
const WITH_UNIT = /^(-?\d+(?:\.\d+)?)([a-zA-Zµ]+)$/;
const DATE = /^\d{4}-\d\d-\d\d([T ]\d\d:\d\d(:\d\d(\.\d+)?)?)?(Z|[+-]\d\d:?\d\d)?$/;
/// A range as Nushell writes it: `start..end`, or `start..next..end` when the step is not 1
/// (counting down: `5..4..1`), with `<` before an end that is left out, and no end at all
/// for one without (`1..`, `1..3..`).
const RANGE = /^(-?\d+(?:\.\d+)?)?\.\.(?:(-?\d+(?:\.\d+)?)\.\.)?(<|=)?(-?\d+(?:\.\d+)?)?$/;
/// Above this many items a range is not written out as an array.
const RANGE_ARRAY_LIMIT = 1_000_000;

const ESCAPES = { '"': '"', "'": "'", "\\": "\\", "/": "/", n: "\n", r: "\r", t: "\t", b: "\b", f: "\f", 0: "\0" };

/// A duration of this many nanoseconds, spread over hours and smaller (days have no fixed
/// length to Temporal). Exact for every Nushell duration.
const duration_from_ns = (nanoseconds) => {
  if (typeof Temporal === "undefined") {
    throw new NuonError("A duration needs Temporal, which this Node does not have (Node 26 or newer does)");
  }
  const sign = nanoseconds < 0n ? -1 : 1;
  const size = nanoseconds < 0n ? -nanoseconds : nanoseconds;
  const seconds = sign * Number(size / 1_000_000_000n);
  const rest = sign * Number(size % 1_000_000_000n);
  return new Temporal.Duration(0, 0, 0, 0, 0, 0, seconds, 0, 0, rest).round({ largestUnit: "hours", smallestUnit: "nanoseconds" });
};

/// The exact length of a Temporal.Duration in nanoseconds.
const duration_to_ns = (duration) => {
  if (duration.years !== 0 || duration.months !== 0) {
    throw new TypeError("A duration with years or months has no fixed length; give it in weeks, days or smaller");
  }
  const days = BigInt(duration.weeks) * 7n + BigInt(duration.days);
  const seconds = ((days * 24n + BigInt(duration.hours)) * 60n + BigInt(duration.minutes)) * 60n + BigInt(duration.seconds);
  return (
    seconds * 1_000_000_000n +
    BigInt(duration.milliseconds) * 1_000_000n +
    BigInt(duration.microseconds) * 1_000n +
    BigInt(duration.nanoseconds)
  );
};

/// A Nushell datetime is a moment plus the UTC offset it was written with. That is a
/// Temporal.ZonedDateTime whose time zone is that offset: exact to the nanosecond, and the
/// offset survives. (An Instant would drop the offset, a Date the nanoseconds as well.)
const parse_datetime = (word) => {
  if (typeof Temporal === "undefined") {
    throw new NuonError("A datetime needs Temporal, which this Node does not have (Node 26 or newer does)");
  }
  let text = word.replace(" ", "T");
  /// Nushell always writes time and offset; a hand-written date may leave them out (UTC then).
  if (!text.includes("T")) text += "T00:00:00";
  const offset = /(Z|[+-]\d\d:?\d\d)$/.exec(text)?.[1];
  if (offset == null) text += "+00:00";
  const zone = offset == null || offset === "Z" ? "+00:00" : offset.includes(":") ? offset : `${offset.slice(0, 3)}:${offset.slice(3)}`;
  return Temporal.Instant.from(text).toZonedDateTimeISO(zone);
};

const parse_int = (text) => {
  const number = Number(text);
  return Number.isSafeInteger(number) ? number : BigInt(text);
};

/// The value of a bare (unquoted) word.
const parse_word = (word) => {
  if (word === "null") return null;
  if (word === "true") return true;
  if (word === "false") return false;
  if (INT.test(word)) return parse_int(word);
  if (word === "NaN") return NaN;
  if (word === "inf") return Infinity;
  if (word === "-inf") return -Infinity;
  if (FLOAT.test(word)) return Number(word);
  const with_unit = WITH_UNIT.exec(word);
  if (with_unit) {
    const [, number, unit] = with_unit;
    if (unit in DURATION_NS) {
      const exact = INT.test(number) ? BigInt(number) * BigInt(DURATION_NS[unit]) : BigInt(Math.round(Number(number) * DURATION_NS[unit]));
      return duration_from_ns(exact);
    }
    if (unit.toLowerCase() in FILESIZE_BYTES) return FileSize(Number(number) * FILESIZE_BYTES[unit.toLowerCase()]);
  }
  if (DATE.test(word)) {
    try {
      return parse_datetime(word);
    } catch (error) {
      /// Not a real date (month 13): text, as Nushell would have quoted a string like it.
      if (!(error instanceof RangeError)) throw error;
    }
  }
  const range = RANGE.exec(word);
  if (range) {
    const [, start, after, operator, end] = range;
    const inclusive = operator !== "<";
    const step = after != null ? Number(after) - Number(start ?? 0) : 1;
    const whole = [start, after, end].every((part) => part == null || INT.test(part));
    if (whole && start != null && end != null && step !== 0) {
      const first = Number(start);
      const span = (Number(end) - first) / step;
      const length = inclusive ? Math.floor(span) + 1 : Math.ceil(span);
      if (length <= RANGE_ARRAY_LIMIT) return Array.from({ length: Math.max(length, 0) }, (_, index) => first + index * step);
    }
    return { start: start == null ? null : Number(start), end: end == null ? null : Number(end), step, inclusive };
  }
  /// Anything else is text: Nushell quotes a string that could be read as something else.
  return word;
};

class Reader {
  constructor(text) {
    this.text = text;
    this.at = 0;
  }

  fail(message) {
    const before = this.text.slice(0, this.at);
    const line = before.split("\n").length;
    const column = this.at - (before.lastIndexOf("\n") + 1) + 1;
    const near = JSON.stringify(this.text.slice(this.at, this.at + 30));
    throw new NuonError(`${message} at line ${line}, column ${column}, near ${near}`);
  }

  peek() {
    return this.text[this.at] ?? "";
  }

  skip_space() {
    const { text } = this;
    while (this.at < text.length) {
      const char = text[this.at];
      if (char === " " || char === "\t" || char === "\r" || char === "\n") this.at++;
      else if (char === "#") {
        const end = text.indexOf("\n", this.at);
        this.at = end === -1 ? text.length : end;
      } else break;
    }
  }

  expect(char) {
    if (this.peek() !== char) this.fail(`Expected ${JSON.stringify(char)}`);
    this.at++;
  }

  /// Between items: an optional comma. True when the collection goes on.
  separator(closing) {
    this.skip_space();
    if (this.peek() === ",") {
      this.at++;
      this.skip_space();
    }
    if (this.peek() === "") this.fail(`Missing ${JSON.stringify(closing)}`);
    return this.peek() !== closing;
  }

  double_quoted() {
    const { text } = this;
    this.at++;
    let result = "";
    while (true) {
      if (this.at >= text.length) this.fail("A string is not closed");
      const char = text[this.at++];
      if (char === '"') return result;
      if (char !== "\\") {
        result += char;
        continue;
      }
      const escape = text[this.at++] ?? "";
      if (escape in ESCAPES) result += ESCAPES[escape];
      else if (escape === "u" && this.peek() === "{") {
        const end = text.indexOf("}", this.at);
        result += String.fromCodePoint(parseInt(text.slice(this.at + 1, end), 16));
        this.at = end + 1;
      } else if (escape === "u") {
        result += String.fromCharCode(parseInt(text.slice(this.at, this.at + 4), 16));
        this.at += 4;
      } else result += "\\" + escape;
    }
  }

  /// Text up to `closing`, for the string forms without escapes.
  until(closing, opening_length) {
    const start = this.at + opening_length;
    const end = this.text.indexOf(closing, start);
    if (end === -1) this.fail("A string is not closed");
    this.at = end + closing.length;
    return this.text.slice(start, end);
  }

  /// A quoted string at this position, or null when there is none.
  string() {
    const char = this.peek();
    if (char === '"') return this.double_quoted();
    if (char === "'") return this.until("'", 1);
    if (char === "`") return this.until("`", 1);
    const raw = /^r(#+)'/.exec(this.text.slice(this.at, this.at + 12));
    if (raw) return this.until("'" + raw[1], raw[0].length);
    return null;
  }

  word(end) {
    const start = this.at;
    const { text } = this;
    while (this.at < text.length && !end.has(text[this.at])) this.at++;
    if (this.at === start) this.fail("Expected a value");
    return text.slice(start, this.at);
  }

  value() {
    this.skip_space();
    const char = this.peek();
    if (char === "[") return this.list_or_table();
    if (char === "{") return this.record();
    if (this.text.startsWith("0x[", this.at)) {
      return Buffer.from(this.until("]", 3).replace(/[\s,]/g, ""), "hex");
    }
    const text = this.string();
    if (text != null) return text;
    return parse_word(this.word(WORD_END));
  }

  record() {
    this.expect("{");
    const result = {};
    this.skip_space();
    while (this.separator("}")) {
      const key = this.string() ?? this.word(KEY_END);
      this.skip_space();
      this.expect(":");
      /// defineProperty: a key named "__proto__" must become an ordinary field.
      Object.defineProperty(result, key, { value: this.value(), enumerable: true, writable: true, configurable: true });
    }
    this.at++;
    return result;
  }

  list_or_table() {
    this.expect("[");
    const items = [];
    this.skip_space();
    if (this.peek() === "[") {
      const first = this.list_or_table();
      this.skip_space();
      if (this.peek() === ";") {
        /// A table: the first list holds the column names, each next one is a row.
        this.at++;
        const rows = [];
        while (this.separator("]")) {
          if (this.peek() !== "[") this.fail("Expected a table row");
          const cells = this.list_or_table();
          if (cells.length !== first.length) {
            this.fail(`A table row has ${cells.length} values for ${first.length} columns`);
          }
          const row = {};
          first.forEach((column, index) => {
            Object.defineProperty(row, column, { value: cells[index], enumerable: true, writable: true, configurable: true });
          });
          rows.push(row);
        }
        this.at++;
        return rows;
      }
      items.push(first);
    }
    while (this.separator("]")) items.push(this.value());
    this.at++;
    return items;
  }
}

export const loads = (text) => {
  const reader = new Reader(text);
  const value = reader.value();
  reader.skip_space();
  if (reader.at !== text.length) reader.fail("Unexpected text after the value");
  return value;
};

// ---------------------------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------------------------

const INT_MIN = -(2n ** 63n);
const INT_MAX = 2n ** 63n - 1n;
const SHORT_ESCAPES = { '"': '\\"', "\\": "\\\\", "\n": "\\n", "\r": "\\r", "\t": "\\t" };

const dump_string = (text) =>
  '"' +
  text.replace(/["\\\u0000-\u001f\u007f]/g, (char) => SHORT_ESCAPES[char] ?? `\\u{${char.codePointAt(0).toString(16)}}`) +
  '"';

const dump_number = (number) => {
  if (Number.isNaN(number)) return "NaN";
  if (!Number.isFinite(number)) return number > 0 ? "inf" : "-inf";
  /// A whole number is written as an integer: JavaScript does not tell 1 and 1.0 apart.
  return String(number);
};

const dump_bigint = (number) => (number >= INT_MIN && number <= INT_MAX ? String(number) : dump_string(String(number)));

const hex = (bytes) => Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString("hex");

/// NUON text for a JavaScript value. Functions and symbols become their string form.
export const dumps = (value, parents = new Set()) => {
  if (value == null) return "null";
  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";
    case "number":
      return dump_number(value);
    case "bigint":
      return dump_bigint(value);
    case "string":
      return dump_string(value);
    case "function":
    case "symbol":
      return dump_string(String(value));
  }
  if (value instanceof FileSize) return `${value.bytes}b`;
  if (typeof Temporal !== "undefined") {
    if (value instanceof Temporal.Duration) return `${duration_to_ns(value)}ns`;
    if (value instanceof Temporal.Instant || value instanceof Temporal.PlainDate || value instanceof Temporal.PlainDateTime) {
      return value.toString();
    }
    /// With its offset, without the time zone's name.
    if (value instanceof Temporal.ZonedDateTime) return value.toString({ timeZoneName: "never" });
  }
  if (value instanceof Number || value instanceof String || value instanceof Boolean) return dumps(value.valueOf());
  if (value instanceof Date) return Number.isNaN(value.getTime()) ? "null" : value.toISOString();
  if (value instanceof Uint8Array) return `0x[${hex(value)}]`;
  if (value instanceof ArrayBuffer) return `0x[${Buffer.from(value).toString("hex")}]`;
  if (value instanceof Error) return dumps({ name: value.name, message: value.message });

  if (parents.has(value)) throw new TypeError("The result refers to itself (a circular structure), which NUON cannot hold");
  parents.add(value);
  try {
    if (value instanceof Map) {
      return "{" + [...value].map(([key, item]) => `${dump_string(String(key))}: ${dumps(item, parents)}`).join(", ") + "}";
    }
    if (Array.isArray(value) || value instanceof Set || ArrayBuffer.isView(value)) {
      return "[" + Array.from(value, (item) => dumps(item, parents)).join(", ") + "]";
    }
    if (typeof value.toJSON === "function") return dumps(value.toJSON(), parents);
    if (typeof value[Symbol.iterator] === "function") {
      return "[" + Array.from(value, (item) => dumps(item, parents)).join(", ") + "]";
    }
    return (
      "{" +
      Object.entries(value)
        .filter(([, item]) => item !== undefined)
        .map(([key, item]) => `${dump_string(key)}: ${dumps(item, parents)}`)
        .join(", ") +
      "}"
    );
  } finally {
    parents.delete(value);
  }
};
