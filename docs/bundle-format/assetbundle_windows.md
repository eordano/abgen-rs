# AssetBundle shader-slot position and container key on windows

**Why it matters:** The windows corpus showed a much larger AssetBundle divergence than the historical linux training corpus. Two platform-specific causes dominated: the URP shader external was placed at the wrong position within each material run, and glTF-sourced bundles used the wrong source extension in their container key. Both rewrite AssetBundle typetree fields, blocking byte-identical output.

**How it works:** Two fixes landed.

- **Target-aware shader-slot position.** The shader external's position within a material run (FIRST vs LAST) is determined by Unity's editor-side InstanceID assignment at build time — per-target state abgen-rs cannot reproduce. No closed-form derivation exists (see `assetbundle_shader_slot_rule_v2.md`), but the per-target majority slot is robust and derivable from the build target alone, a public input. Windows prod overwhelmingly places the shader external FIRST, so `ExternalsPosition::for_target` returns `First` for windows (and mac), `Last` otherwise. The glb builder call site consumes this target-aware position.

- **Source-extension-aware container key.** glTF inputs must emit a `.gltf` container key for the glb-prefab entry, not `.glb`. The builder now threads an `is_gltf` flag through to select the correct extension.

A minority of windows bundles still want LAST under the FIRST default; that residual is irreducible statically and only closable via emit-and-verify with a known prod hash (see `assetbundle_expect_hash.md`). The remaining few AssetBundle field-diffs are knock-on effects of cross-bundle external plumbing in textures, materials, and text assets, tracked separately and out of class for these fixes.
