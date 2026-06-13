# External fileID numbering: first PPtr use in serialization order

**Why it matters:** A bundle that depends on other bundles (the shader
bundle plus any cross-bundle textures) carries an externals table in its
serialized file, and every cross-bundle PPtr selects a slot in it through
`m_FileID`. abgen used to number the slots in the order it *built* the
materials, so whenever that order differed from Unity's the whole table
came out permuted: same dependency set, different order, and a one-byte
`m_FileID` divergence inside every Material and preload entry that touched
a moved slot.

**The rule (derived from reference bytes):** Unity assigns externals-table
slots on the first PPtr *write* during serialization. Objects serialize in
ascending PathID order and fields in typetree order, so walking the
reference file that way always yields the fid sequence 1, 2, .., n —
verified on every bundle in the val300 windows reference (probe:
`examples/extorder`). When the first external references live in Material
objects with negative PathIDs (which serialize before the AssetBundle
object), the numbering is fully content-derivable. When the AssetBundle
object itself is the first user, the numbering follows its preload-table
order, which inherits the preload-ordering wall.

**How abgen implements it:** at commit time, after every ordering decision
is final, `commit_objects` walks the object trees in serialization order
with a typetree-faithful traversal (`collect_pptr_first_use`), and if the
first-use sequence is not already ascending it remaps every PPtr
`m_FileID` and permutes the externals table to match. The `m_Dependencies`
list on the AssetBundle object is unaffected — it is a lexicographically
sorted set of CAB names, independent of slot numbering.
