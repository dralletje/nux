This folder has `orders.csv` (one order per row; it is standard CSV: a field may be
quoted, and a quoted field may hold commas, quotes and line breaks), `customers.json`
(id, country, signup date) and `rates.json` (what one unit of each currency is in EUR).

Write `answer.json` in this folder, one JSON object with these keys:

- `top_countries`: the three countries with the highest total order amount in EUR,
  counting only orders dated in 2026 by customers who signed up in 2025. Highest first, as
  a list of `{"country": "NL", "eur": 1234.56}` (eur rounded to 2 decimals).
- `notes_with_line_breaks`: how many orders have a note that contains a line break.
- `customers_without_orders`: how many customers have no order at all.
