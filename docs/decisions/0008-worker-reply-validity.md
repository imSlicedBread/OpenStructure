# ADR 0008: asynchronous plugin replies belong to a session and revision

Keep the v1 JSON wire compatible while adding a host-owned request ticket around
Wasm execution. Tickets capture a fresh document-session ID (not the persistent
project ID), model revision, optional view/settings revision, and plugin load
generation. Reopening the same file must invalidate previous replies even if its
project ID and initial revision match. Tickets and session IDs are not persisted.

Run only the bounded Wasm adapter on worker threads; do not pretend arbitrary
built-ins are safe background jobs. One execution per loaded Wasm instance and
at most four outstanding jobs per host, including buffered replies and
retired/unloaded instances. A slot is released only after both the worker and
reply owner release it. Workers
retain their module until execution exits; unload removes registrations immediately
and invalidates replies, but never frees executing code. Dependencies prevent
unloading a plugin still required by another loaded plugin.

Cancellation/deadlines revoke result acceptance without blocking the caller.
Wasm's existing fuel/memory caps bound guest execution after revocation. This is
not hard OS thread termination, nor a compiler-time ceiling. Reject stale results
before parsing/applying them. Command replies are applied atomically during poll;
geometry carries a checked stamp for its later consumer. Settled jobs cannot replay.
Keep desktop activation gated until generic SDK, worker loading, lifecycle UI and
plugin data durability acceptance are complete. Design optional view context now
for the required linked 2D workflow, without claiming a plan provider exists.
