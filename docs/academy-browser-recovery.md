# Academy browser recovery follow-up

Native `.fcproj` Save / Save As still request a browser download. A browser cannot confirm that
the person accepted that download or that it reached their chosen folder. This fork therefore
keeps recovery separately from download acknowledgement.

- Adopted native saves queue a clean browser checkpoint. Save a Copy / selection export cannot
  replace the session checkpoint merely because they wrote another `.fcproj` file.
- Dirty work keeps the existing five-second snapshot cadence. `checkpointProject()` can request
  an immediate checkpoint without changing the engine's dirty state, project identity or history.
- Clean and dirty checkpoints are separate single-file records, each with a previous-valid
  fallback. Native projects are validated before writing and before restoring. Corrupt, oversized
  and unsupported records are ignored; the newest valid candidate is restored.
- Original `recovery/meta.json` and `snapshot.fcproj` files are not deleted or rewritten. A valid
  legacy dirty pair remains a fallback if no new record is usable.
- Records are bounded to64MiB. OPFS writes are serialized and same-kind pending work coalesces.
  Failure retains queued bytes and the open project; retry or native download remains possible.
  `flushRecovery()` rejects rather than claiming a failed or pending write completed.
- Missing source-media references remain intact and are reported through normal project opening.
  A native project or checkpoint does not embed imported media or promise portable relinking.
- Project Manager collection is visibly unavailable in WASM because its implementation requires
  desktop filesystem access. No fake browser collection or incomplete media bundle is offered.

Browser API additions:

```js
await filmcraft.checkpointProject(); // live project -> confirmed OPFS checkpoint, no engine save
await filmcraft.flushRecovery();     // wait for posted Save adoption and actual writable close
filmcraft.info().recovery;           // available,pending,failed,completed,error,note
```

These operations acknowledge the browser checkpoint only. Native download dialogs, storage quota,
missing media, and unsupported OPFS remain separate outcomes. A pending checkpoint must finish before
an integrating host navigates away; closing the tab before that acknowledgement cannot be promised
durable. Last-session recovery is per browser origin, not a replacement for a named project library.

Focused native tests cover clean/dirty round trips, ordering, corrupt/missing fallback, schema and
time validation, and preservation of missing-media paths. The existing download/recovery-policy
tests remain. The Academy manual build runs these checks and compiles the actual WASM; browser
save/reload and storage-failure interaction remain separate evidence owned by the game integrator.
