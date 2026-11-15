"""NUON, Nushell's data notation, for Python: `loads` and `dumps`.

`loads` reads what Nushell's `to nuon` writes (not everything a person could type as NUON);
`dumps` writes what `from nuon` reads. Used by py_runner.py, so that values keep their type
on the way into Python and back:

    Nushell     Python
    datetime    datetime.datetime      (nanoseconds are cut to microseconds)
    duration    datetime.timedelta     (same)
    filesize    Filesize, an int       (bytes)
    binary      bytes
    record      dict
    table       list of dicts
    range       range                  (whole numbers, with an end; also counting down or in
                                        steps. Else a dict start/end/step/inclusive)
"""

import dataclasses
import datetime
import decimal
import enum
import fractions
import math
import os
import re

INT_MIN, INT_MAX = -(2**63), 2**63 - 1


class Filesize(int):
    """A number of bytes that goes back to Nushell as a filesize."""

    def __repr__(self):
        return "Filesize(%d)" % int(self)


class NuonError(ValueError):
    pass


# -------------------------------------------------------------------------------------------
# Reading
# -------------------------------------------------------------------------------------------

# A bare word ends at one of these (a colon does not end it: dates contain colons).
WORD_END = set(" \t\r\n,;[]{}")
KEY_END = WORD_END | {":"}

DURATION_NS = {
    "ns": 1,
    "us": 1_000,
    "µs": 1_000,
    "ms": 1_000_000,
    "sec": 1_000_000_000,
    "min": 60 * 1_000_000_000,
    "hr": 3600 * 1_000_000_000,
    "day": 86400 * 1_000_000_000,
    "wk": 7 * 86400 * 1_000_000_000,
}
FILESIZE_BYTES = {
    "b": 1,
    "kb": 1000,
    "mb": 1000**2,
    "gb": 1000**3,
    "tb": 1000**4,
    "pb": 1000**5,
    "eb": 1000**6,
    "kib": 1024,
    "mib": 1024**2,
    "gib": 1024**3,
    "tib": 1024**4,
    "pib": 1024**5,
    "eib": 1024**6,
}

INT = re.compile(r"-?\d+\Z")
FLOAT = re.compile(r"-?(\d+\.\d*|\.\d+|\d+)([eE][+-]?\d+)?\Z")
WITH_UNIT = re.compile(r"(-?\d+(?:\.\d+)?)([a-zA-Zµ]+)\Z")
DATE = re.compile(r"\d{4}-\d\d-\d\d([T ]\d\d:\d\d(:\d\d(\.\d+)?)?)?(Z|[+-]\d\d:?\d\d)?\Z")
# A range as Nushell writes it: `start..end`, or `start..next..end` when the step is not 1
# (counting down: `5..4..1`), with `<` before an end that is left out, and no end at all
# for one without (`1..`, `1..3..`).
RANGE = re.compile(r"(-?\d+(?:\.\d+)?)?\.\.(?:(-?\d+(?:\.\d+)?)\.\.)?(<|=)?(-?\d+(?:\.\d+)?)?\Z")

ESCAPES = {'"': '"', "'": "'", "\\": "\\", "/": "/", "n": "\n", "r": "\r", "t": "\t", "b": "\b", "f": "\f", "0": "\0"}


def parse_date(word):
    text = word.replace(" ", "T", 1)
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    # Python keeps microseconds; Nushell writes nanoseconds.
    text = re.sub(r"(\.\d{6})\d+", r"\1", text)
    text = re.sub(r"\.(\d{1,5})(?=$|[+-])", lambda m: "." + m.group(1).ljust(6, "0"), text)
    return datetime.datetime.fromisoformat(text)


def parse_number(text):
    return int(text) if INT.match(text) else float(text)


def parse_word(word):
    """The value of a bare (unquoted) word."""
    if word == "null":
        return None
    if word == "true":
        return True
    if word == "false":
        return False
    if INT.match(word):
        return int(word)
    if word in ("NaN", "inf", "-inf"):
        return float(word)
    if FLOAT.match(word):
        return float(word)
    with_unit = WITH_UNIT.match(word)
    if with_unit:
        number, unit = with_unit.groups()
        if unit in DURATION_NS:
            nanoseconds = parse_number(number) * DURATION_NS[unit]
            # Whole microseconds, cut towards zero (exact for the integers Nushell writes).
            sign = -1 if nanoseconds < 0 else 1
            return datetime.timedelta(microseconds=sign * (int(abs(nanoseconds)) // 1000))
        if unit.lower() in FILESIZE_BYTES:
            return Filesize(round(parse_number(number) * FILESIZE_BYTES[unit.lower()]))
    if DATE.match(word):
        try:
            return parse_date(word)
        except ValueError:
            pass
    in_range = RANGE.match(word)
    if in_range:
        start, after, operator, end = in_range.groups()
        inclusive = operator != "<"
        whole = all(part is None or INT.match(part) for part in (start, after, end))
        step = (parse_number(after) - parse_number(start or "0")) if after is not None else 1
        if whole and start is not None and end is not None and step != 0:
            stop = int(end) + ((1 if step > 0 else -1) if inclusive else 0)
            return range(int(start), stop, int(step))
        return {
            "start": None if start is None else parse_number(start),
            "end": None if end is None else parse_number(end),
            "step": step,
            "inclusive": inclusive,
        }
    # Anything else is text: Nushell quotes a string that could be read as something else.
    return word


class Reader:
    def __init__(self, text):
        self.text = text
        self.at = 0

    def fail(self, message):
        line = self.text.count("\n", 0, self.at) + 1
        column = self.at - (self.text.rfind("\n", 0, self.at) + 1) + 1
        near = self.text[self.at : self.at + 30]
        raise NuonError("%s at line %d, column %d, near %r" % (message, line, column, near))

    def peek(self):
        return self.text[self.at] if self.at < len(self.text) else ""

    def skip_space(self):
        text, at = self.text, self.at
        while at < len(text):
            if text[at] in " \t\r\n":
                at += 1
            elif text[at] == "#":
                end = text.find("\n", at)
                at = len(text) if end == -1 else end
            else:
                break
        self.at = at

    def expect(self, char):
        if self.peek() != char:
            self.fail("Expected %r" % char)
        self.at += 1

    def separator(self, closing):
        """Between items: an optional comma. True when the collection goes on."""
        self.skip_space()
        if self.peek() == ",":
            self.at += 1
            self.skip_space()
        if self.peek() == "":
            self.fail("Missing %r" % closing)
        return self.peek() != closing

    def double_quoted(self):
        text = self.text
        self.at += 1
        parts = []
        while True:
            if self.at >= len(text):
                self.fail("A string is not closed")
            char = text[self.at]
            self.at += 1
            if char == '"':
                return "".join(parts)
            if char != "\\":
                parts.append(char)
                continue
            escape = self.peek()
            self.at += 1
            if escape in ESCAPES:
                parts.append(ESCAPES[escape])
            elif escape == "u" and self.peek() == "{":
                end = text.find("}", self.at)
                parts.append(chr(int(text[self.at + 1 : end], 16)))
                self.at = end + 1
            elif escape == "u":
                parts.append(chr(int(text[self.at : self.at + 4], 16)))
                self.at += 4
            else:
                parts.append("\\" + escape)

    def until(self, closing, opening_length):
        """Text up to `closing`, for the string forms without escapes."""
        start = self.at + opening_length
        end = self.text.find(closing, start)
        if end == -1:
            self.fail("A string is not closed")
        self.at = end + len(closing)
        return self.text[start:end]

    def string(self):
        """A quoted string at this position, or None when there is none."""
        char = self.peek()
        if char == '"':
            return self.double_quoted()
        if char == "'":
            return self.until("'", 1)
        if char == "`":
            return self.until("`", 1)
        raw = re.compile(r"r(#+)'").match(self.text, self.at)
        if raw:
            return self.until("'" + raw.group(1), len(raw.group(0)))
        return None

    def word(self, end):
        start = self.at
        text = self.text
        while self.at < len(text) and text[self.at] not in end:
            self.at += 1
        if self.at == start:
            self.fail("Expected a value")
        return text[start : self.at]

    def value(self):
        self.skip_space()
        char = self.peek()
        if char == "[":
            return self.list_or_table()
        if char == "{":
            return self.record()
        if self.text.startswith("0x[", self.at):
            digits = self.until("]", 3)
            return bytes.fromhex(re.sub(r"[\s,]", "", digits))
        text = self.string()
        if text is not None:
            return text
        return parse_word(self.word(WORD_END))

    def record(self):
        self.expect("{")
        result = {}
        self.skip_space()
        while self.separator("}"):
            key = self.string()
            if key is None:
                key = self.word(KEY_END)
            self.skip_space()
            self.expect(":")
            result[key] = self.value()
        self.at += 1
        return result

    def list_or_table(self):
        self.expect("[")
        items = []
        self.skip_space()
        if self.peek() == "[":
            first = self.list_or_table()
            self.skip_space()
            if self.peek() == ";":
                # A table: the first list holds the column names, each next one is a row.
                self.at += 1
                rows = []
                while self.separator("]"):
                    if self.peek() != "[":
                        self.fail("Expected a table row")
                    cells = self.list_or_table()
                    if len(cells) != len(first):
                        self.fail("A table row has %d values for %d columns" % (len(cells), len(first)))
                    rows.append(dict(zip(first, cells)))
                self.at += 1
                return rows
            items.append(first)
        while self.separator("]"):
            items.append(self.value())
        self.at += 1
        return items


def loads(text):
    reader = Reader(text)
    value = reader.value()
    reader.skip_space()
    if reader.at != len(text):
        reader.fail("Unexpected text after the value")
    return value


# -------------------------------------------------------------------------------------------
# Writing
# -------------------------------------------------------------------------------------------

SHORT_ESCAPES = {'"': '\\"', "\\": "\\\\", "\n": "\\n", "\r": "\\r", "\t": "\\t"}


def dump_string(text):
    parts = []
    for char in text:
        if char in SHORT_ESCAPES:
            parts.append(SHORT_ESCAPES[char])
        elif ord(char) < 0x20 or ord(char) == 0x7F:
            parts.append("\\u{%x}" % ord(char))
        else:
            parts.append(char)
    return '"' + "".join(parts) + '"'


def dump_float(number):
    if math.isnan(number):
        return "NaN"
    if math.isinf(number):
        return "inf" if number > 0 else "-inf"
    # float(): a subclass such as numpy.float64 has its own repr.
    text = repr(float(number))
    return text if any(c in text for c in ".e") else text + ".0"


def dumps(value):
    """NUON text for a Python value. Unknown objects become their string form."""
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, Filesize):
        return "%db" % int(value)
    if isinstance(value, enum.Enum):
        return dumps(value.value)
    if isinstance(value, int):
        # Nushell integers are 64 bit; a larger one is kept exact as text.
        return str(int(value)) if INT_MIN <= value <= INT_MAX else dump_string(str(int(value)))
    if isinstance(value, float):
        return dump_float(value)
    if isinstance(value, (decimal.Decimal, fractions.Fraction)):
        return dump_float(float(value))
    if isinstance(value, str):
        return dump_string(value)
    if isinstance(value, (bytes, bytearray, memoryview)):
        return "0x[" + bytes(value).hex() + "]"
    if isinstance(value, datetime.datetime):
        return value.isoformat()
    if isinstance(value, datetime.date):
        return value.isoformat()
    if isinstance(value, datetime.timedelta):
        microseconds = (value.days * 86400 + value.seconds) * 1_000_000 + value.microseconds
        return "%dns" % (microseconds * 1000)
    if isinstance(value, dict):
        fields = ("%s: %s" % (dump_string(str(key)), dumps(item)) for key, item in value.items())
        return "{" + ", ".join(fields) + "}"
    if isinstance(value, (list, tuple, set, frozenset, range)):
        return "[" + ", ".join(dumps(item) for item in value) + "]"
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return dumps(dataclasses.asdict(value))
    if isinstance(value, os.PathLike):
        return dump_string(os.fspath(value))
    # pandas DataFrame: a table.
    if hasattr(value, "to_dict") and hasattr(value, "columns"):
        return dumps(value.to_dict("records"))
    # pandas Series, namedtuple-likes.
    if hasattr(value, "to_dict"):
        return dumps(value.to_dict())
    if hasattr(value, "_asdict"):
        return dumps(value._asdict())
    # numpy arrays and scalars.
    if hasattr(value, "tolist"):
        return dumps(value.tolist())
    if hasattr(value, "item"):
        return dumps(value.item())
    if hasattr(value, "__iter__"):
        return dumps(list(value))
    return dump_string(str(value))
