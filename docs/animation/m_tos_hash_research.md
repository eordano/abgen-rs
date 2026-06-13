# AnimatorController m_TOS: hash solved, serialization order not

**Why it matters:** An AnimatorController serializes an `m_TOS` table of
`(hash, name)` pairs. To match Unity byte-for-byte we must reproduce both the
hash value of each entry and the order they are written in. Getting either wrong
diverges the AnimatorController object from prod.

**How it works:** The hash is plain CRC32 over the UTF-8 bytes of the name, which
abgen-rs already computes correctly. The entries are *not* transform/bone paths,
as the original task framing assumed; for the emote and wearable controllers we
emit they are the strings the animator graph itself produces — parameter names,
clip names, the layer name, state and transition identifiers, plus the empty
string at hash zero. No transform-hierarchy walk is needed, and abgen-rs already
produces the correct *set* of pairs.

**Negative finding on ordering:** The remaining divergence is purely the
iteration order of the pairs, and it is unresolved. The order is a stable
function of the hash set (identical inputs give identical prod order), but it does
not match any sort or hash-table layout tried — ascending/descending sorts, a
wide range of open-addressing and bucket-hash schemes, name-based orderings, and
more all failed. The most likely explanation is a non-standard internal hashtable
in Unity's serialization path whose layout can only be confirmed from Unity
source or disassembly, which is out of scope. It is treated as a blocked residual
until Unity-internal information is available.
