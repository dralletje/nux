`logs/` holds three days of access logs, one JSON object per line, with the fields `ts`
(UTC time), `method`, `path`, `status`, `ms` (response time) and `user`.

Write `answer.json` in this folder, one JSON object with these keys:

- `top_5xx_paths`: the three paths with the most responses of status 500 or higher, most
  first (a tie: alphabetical), as a list of paths.
- `p95_ms_get_orders`: the 95th percentile of `ms` for `GET /api/orders`, by nearest rank:
  sort the values ascending and take the one at position ceil(0.95 × n), counting from 1.
  A number.
- `users_with_4xx`: how many distinct users got at least one 4xx response. A number.
- `busiest_hour`: the hour with the most requests over all three days, as the first 13
  characters of `ts` (e.g. `2026-02-10T14`). A tie: the earliest.
