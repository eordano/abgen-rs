# CIDv0 CAB hashes are computed from the lowercased bundle filename

**Why it matters:** A bundle's CAB name and its sibling references are derived by SpookyHashing the bundle filename. On legacy CIDv0 entities the filename is a mixed-case `Qm…` CID, and the converter lowercased that name before hashing. abgen-rs hashed the original mixed-case name, so every CAB-derived string on a `Qm` bundle diverged: the bundle's own `CAB-<hash>` filename, each `externals[i].path` pointing at a sibling content bundle, and the `m_Dependencies` entries in the AssetBundle object. That made byte-identical output impossible for the entire CIDv0 corpus.

**How it works:** The CAB hash function lowercases its input before hashing, exactly as the converter does. This is the durable rule: CAB hashing is case-insensitive with respect to the bundle filename. CIDv1 names (`bafkrei…`/`bafybei…`) are already all-lowercase ASCII, so lowercasing is a no-op there and the modern corpus is unaffected; the change only moves the CIDv0 bundles into agreement with prod.

A separate, unrelated source of externals divergence remains on multi-external CIDv1 bundles, where the slot ordering of the externals list differs from prod. That is an ordering problem, independent of CAB hashing, and is not addressed here.
