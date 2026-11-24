This folder is a Node.js project with a large test suite; it runs with `node --test`. Do
not change any file except `answer.json`.

Run the tests and write `answer.json` in this folder:

```json
{
  "failing": [{"file": "test/....test.js", "test": "...", "message": "..."}],
  "skipped": 0,
  "todo": 0
}
```

- `failing`: every test that fails: its file, its name exactly as the runner names it, and
  the message of the assertion that failed (the text the test gives, not the stack).
- `skipped`: how many tests were skipped. `todo`: how many tests are marked todo.
