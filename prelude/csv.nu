# CSV and TSV as text: every value as it is written in the file.
#
# Nushell's own `from csv` guesses a type for each cell on its own: 02134 becomes 2134,
# +31612345678 becomes 31612345678, 12.50 becomes 12.5, 1e3 becomes 1000.0, and a column can
# hold an int in one row and a float or text in the next. Nothing says so. Here they stay
# text (Nushell's --no-infer, always), and a column becomes numbers when asked:
# `into float amount`, `into int count`. `open x.csv` and `open x.tsv` parse with these too.

alias nushell-from-csv = from csv
alias nushell-from-tsv = from tsv

# Parse CSV text into a table. Every value stays text, as written: 02134 stays "02134".
#
# For numbers, convert the columns that hold them: `into float amount price`, `into int n`.
# (Nushell's own `from csv` guesses a type per cell, which drops leading zeros and the `+` of
# phone numbers, and can give one column ints and texts.) --no-infer is accepted and changes
# nothing.
@search-terms csv "comma separated" spreadsheet
@example "Text as written, then a column as numbers" { "item,amount\na,12.50\nb,7\n" | from csv | into float amount | sort-by amount }
@example "Another separator, no header row" { open --raw data.txt | from csv --separator ';' --noheaders }
export def "from csv" [
  --separator (-s): string  # A character to separate columns, defaults to ','
  --comment (-c): string    # A comment character to ignore lines starting with it
  --quote (-q): string      # A quote character to ignore separators in strings, defaults to '"'
  --escape (-e): string     # An escape character for strings containing the quote character
  --noheaders (-n)          # Don't treat the first row as column names
  --flexible                # Allow the number of fields in records to be variable
  --no-infer                # Changes nothing: values are always text here
  --trim (-t): string       # Drop whitespace around headers and/or values: all, headers, fields
]: string -> table {
  # A flag that was not given is null, which Nushell's own takes as not given.
  $in | nushell-from-csv --no-infer --separator $separator --comment $comment --quote $quote --escape $escape --noheaders=$noheaders --flexible=$flexible --trim $trim
}

# Parse TSV text into a table. Every value stays text, as written: 02134 stays "02134".
#
# For numbers, convert the columns that hold them: `into float amount price`, `into int n`.
# --no-infer is accepted and changes nothing.
@search-terms tsv "tab separated"
@example "Text as written, then a column as numbers" { "item\tamount\na\t12.50\n" | from tsv | into float amount }
export def "from tsv" [
  --comment (-c): string    # A comment character to ignore lines starting with it
  --quote (-q): string      # A quote character to ignore separators in strings, defaults to '"'
  --escape (-e): string     # An escape character for strings containing the quote character
  --noheaders (-n)          # Don't treat the first row as column names
  --flexible                # Allow the number of fields in records to be variable
  --no-infer                # Changes nothing: values are always text here
  --trim (-t): string       # Drop whitespace around headers and/or values: all, headers, fields
]: string -> table {
  $in | nushell-from-tsv --no-infer --comment $comment --quote $quote --escape $escape --noheaders=$noheaders --flexible=$flexible --trim $trim
}
