This folder is a git repository. Answer these questions about its history and write the
answers to `answer.json` in this folder, as one JSON object with these keys:

- `breaking_commits`: the hashes (full, or at least 7 characters) of every commit that is
  not a merge and whose message body (not its subject line) has a line that starts with
  `BREAKING CHANGE:`. A list, in any order.
- `renames`: every file rename in the history, as a list of `{"from": "...", "to": "..."}`
  (paths as git prints them), in any order.
- `api_commits_by_author`: for each author name (as git prints it, e.g. `Alice Example`),
  how many commits that are not merges changed at least one file under `src/api/`. An
  object of name → number; leave out authors with none.
- `feature_lines_deleted`: the total number of lines deleted by the commits that came in
  with the merge of the branch `feature/export` (the commits of that branch, not the
  merge itself). A number.

Do not change the repository.
