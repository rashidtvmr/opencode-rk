# BASE-004 - Singleton daemon with PID lock

Done. `crates/server/src/daemon.rs` implements PidLock + SingletonDaemon.

PidLock: acquire writes PID, fails AlreadyRunning if live PID in file, reclaims stale (dead PID ok), is_held checks /proc liveness, Drop removes file. SingletonDaemon::bind takes pid + socket path, removes stale socket, binds tokio UnixListener. accept_clients biased select loop tracks count via AtomicUsize, spawns hold task per client. shutdown flags AtomicBool + Notify.

Deviations: no tokio-util in server deps so shutdown uses AtomicBool+Notify not CancellationToken (same semantics, single method swap when dep approved). Stale check via /proc/<pid> exists (Linux, keeps forbid(unsafe_code), no libc dep). hold_client keeps conn open via readable loop; EOF drops decrement count.

Tests: pid_lock_acquire_release, pid_lock_double_acquire_fails, pid_lock_stale_reclaim (u32::MAX dead PID), daemon_accept_client (client_count 1 then 0), daemon_shutdown_closes (loop ends in 2s).

Verify: cargo test -p opencode-rk-server (10 pass), cargo check --workspace clean.
