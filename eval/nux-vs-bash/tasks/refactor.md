This folder is a small Node.js project (ES modules); its tests run with `node --test`.

Rename the function `getUser` (exported from `src/users.js`) to `fetchUser`, everywhere it
is used in `src/` and `test/`, so that no code refers to it by its old name any more.

The RPC name `"getUser"` in `data/routes.json` is a public API that clients call: it must
not change, and calling it must keep working. When you are done, all tests pass.
