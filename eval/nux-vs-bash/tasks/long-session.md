This folder is a small Node project (`shop`, ES modules, tests with `node --test`) with a
git history on `main`. Do all of the following; take whatever steps you need.

1. The tests fail. Find and fix the bugs in `src/` (do not change the tests).
2. Find the commit that introduced the bug in how money is rounded. Put its full hash in
   `answer.json` as `rounding_bug_commit`.
3. Rename the function `calcTotal` to `computeTotal` everywhere in the code (`src/`,
   `test/`, `bin/`). `data/api-schema.json` names a public API operation `calcTotal`: that
   name must stay as it is.
4. `node bin/report.js` prints a JSON summary per customer. Add `--format csv`: the same
   rows, in the same order, as CSV with the header `customer,orders,total`, the total with
   two decimals, quoted the standard CSV way where needed. Without the flag it prints the
   JSON as before.
5. `data/sales.csv` is an export of sales (amounts in EUR). In `answer.json`:
   `q1_2026_by_region`, the total amount per region for January to March 2026, as
   `{"North": 1234.56, ...}` rounded to 2 decimals; and `top_rep_2025`, the rep with the
   highest total amount in 2025.
6. Commit your changes to the code on a new branch `report-csv`, made from `main`, as
   commits whose subjects start with `fix:`, `feat:` and `refactor:` (at least one of
   each), like the repository's own history. Leave `main` as it is, and leave
   `answer.json` out of git.

`answer.json` is one JSON object with the keys named above.
