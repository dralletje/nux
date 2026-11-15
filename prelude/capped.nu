# capped: run an external command with a hard memory limit and a time limit.
#
#     use nux/capped
#     capped --mem 1500MB --time 40sec pnpm exec tsc -b
#
# The memory limit is enforced by the kernel (a cgroup made with `systemd-run --user
# --scope`), for the command and everything it starts: when they go over it together, the
# kernel kills the biggest one. Nothing is polled, so a fast spike cannot slip through.
# The time limit is `timeout`: SIGTERM at the limit, SIGKILL two seconds later.
#
# Returns a record instead of failing, so a runaway is a result you can look at:
#     exit_code    the command's exit code (137 when killed, 124 when out of time)
#     killed_by    "memory", "time" or null
#     peak_memory  the most memory the command and its children used together
#     duration     how long it ran
#     stdout, stderr
#
# Only for external commands: a Nushell closure runs inside the session's own process,
# which cannot be put in a cgroup of its own.

# Run an external command under a memory and a time limit; see the top of this file.
export def --wrapped main [
  --mem (-m): filesize = 2GB    # memory limit for the command and its children together
  --time (-t): duration = 2min  # time limit
  ...command: string            # the external command and its arguments
]: nothing -> record {
  if ($command | is-empty) {
    error make {msg: "capped: no command given", help: "capped --mem 1GB --time 30sec <command> [args...]"}
  }
  let runtime_dir = ($env.XDG_RUNTIME_DIR? | default $"/run/user/(^id -u | str trim)")
  if not ($runtime_dir | path join "systemd" | path exists) {
    error make {
      msg: "capped: no systemd user session to create the memory limit with"
      help: $"Looked for ($runtime_dir)/systemd. Without it there is no hard memory limit; `timeout` alone still limits time."
    }
  }
  let stats_file = (mktemp --tmpdir capped.XXXXXX)

  # Runs inside the cgroup. After the command it writes what the cgroup measured: the
  # command's exit code, the peak memory in bytes, and how often the kernel killed for memory.
  # When the kernel kills for memory, the wrapper has to outlive that to report. Two things
  # make it so (tried without each: the result then has no numbers): OOMPolicy=continue stops
  # systemd from killing the whole scope, and the wrapper catches the SIGTERM it still gets
  # (a handler, unlike `ignore`, is not inherited by the command).
  let inner = '
    trap : TERM
    timeout --kill-after=2 "$CAPPED_SECONDS" "$@"
    code=$?
    cgroup=/sys/fs/cgroup$(cut -d: -f3- /proc/self/cgroup)
    echo "$code $(cat "$cgroup/memory.peak") $(grep -w oom_kill "$cgroup/memory.events" | cut -d" " -f2)" > "$CAPPED_STATS"
    exit $code
  '
  let started = (date now)
  let run = (with-env {
    XDG_RUNTIME_DIR: $runtime_dir
    CAPPED_SECONDS: ($time / 1sec | into string)
    CAPPED_STATS: $stats_file
  } {
    (^systemd-run --user --scope --quiet --collect
      -p $"MemoryMax=($mem | into int)" -p MemorySwapMax=0 -p OOMPolicy=continue
      -- sh -c $inner capped ...$command) | complete
  })
  let duration = ((date now) - $started)

  # No stats means the wrapper itself was killed (it is in the cgroup too).
  let stats = (open --raw $stats_file | str trim | split row " ")
  rm -f $stats_file
  let measured = (($stats | length) == 3)
  let exit_code = if $measured { $stats.0 | into int } else { $run.exit_code }
  let out_of_memory = if $measured { ($stats.2 | into int) > 0 } else { true }

  {
    exit_code: $exit_code
    killed_by: (if $out_of_memory { "memory" } else if $exit_code == 124 { "time" } else { null })
    peak_memory: (if $measured { $stats.1 | into int | into filesize } else { null })
    duration: $duration
    stdout: $run.stdout
    stderr: $run.stderr
  }
}
