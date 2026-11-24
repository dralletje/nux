# timeout: the one time limit in nux. Commands that wait (`wait-for ...`) wait until what
# they wait for happens; a limit is put around them, or around any other code, with this.

# Run a block, and stop it when it has not finished within this time: an error then.
#
# The block runs as a job (`job spawn`): when the time is up, the job is ended, and with it
# the external commands it started. Within the time, it gives what the block gives, or fails
# with the block's own error. The block gets the input of `timeout` as its `$in`.
#
# Changes the block makes to $env stay in it, as with `do`. For one external command with a
# hard limit on memory too: `use nux/capped`.
@search-terms "time limit" "give up after" deadline "wait at most"
@example "A dev server, for at most two minutes" { timeout 2min { wait-for url http://localhost:3000/ } }
@example "A slow command, stopped after 10 seconds" { timeout 10sec { ^pnpm outdated --format json | complete } }
@example "Whatever comes first: a failure is the block's own error" { timeout 1min { wait-for file /tmp/build.log --contains "Done in" } }
export def main [
  limit: duration   # How long the block may take
  block: closure    # The code to run: { wait-for url http://localhost:3000/ }
]: any -> any {
  let input = $in
  # Its own tag, so a message of another job is not taken for the block's result.
  let tag = (random int 1..2147483647)
  let id = (job spawn {
    let result = (try { {value: ($input | do $block)} } catch {|error| {error: $error} })
    $result | job send 0 --tag $tag
  })
  let result = (try { job recv --tag $tag --timeout $limit } catch { null })
  if $result == null {
    try { job kill $id }
    # A moment for the block to end: what it says when stopped (wait-for: what it last saw,
    # on stderr) comes before this error.
    try { job recv --tag $tag --timeout 2sec } catch { null }
    error make {
      msg: $"timeout: the block did not finish within ($limit); it was stopped"
      label: {text: $"stopped after ($limit)", span: (metadata $block).span}
      help: "For longer, a larger limit (and give your own tool call as long)."
    }
  }
  if "error" in $result {
    error make $result.error.raw
  }
  $result.value
}
