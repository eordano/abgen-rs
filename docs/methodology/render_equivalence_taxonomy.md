# Render-equivalence taxonomy: classifying bundles by what the client sees

**Why it matters:** Byte parity is the strongest guarantee abgen can give, but it is not the goal. The goal is that the Decentraland client — a Unity binary — loads a bundle and renders it the way the user is supposed to see it. A bundle can differ from the reference byte-for-byte and still render pixel-identically; it can also match the source image perfectly and still render *wrong* if its sampler state is off. So bundles need a second classification, orthogonal to the byte taxonomy, organized around what reaches the screen.

## The three things a texture bundle controls

When the client renders a texture, three independent parts of the bundle decide the result, and a bundle's render-tier is set by the worst of them:

- **Sampler state** — the texture's format, color space, wrap and filter modes, anisotropy, and mip count. These fields tell the GPU *how to sample* the payload, so they change the rendered result even when the decoded pixels are byte-identical. A texture flagged linear instead of sRGB renders too dark or washed out with no pixel difference at all; the wrong wrap mode tiles incorrectly at the seams; a missing mip chain shimmers under minification.
- **Decoded pixels** — what the compressed payload actually decodes to. This is judged alpha-weighted: RGB under a fully transparent texel is never sampled, so a large difference there is invisible and must not count. Perceptibility is judged by the weighted *mean* error across the image, not the single worst texel — one divergent texel at a hard edge says nothing about whether a human sees a difference.
- **Binding and structure** — whether the bundle loads at all and whether the material's pointer resolves to the texture object. A broken pointer or a missing object renders as the engine's default (often magenta or untextured), which is the most severe failure even though it may be a tiny byte difference.

## The tiers

From "the user cannot possibly see a difference" to "broken on screen":

- **G1 — byte-identical.** Identical bytes load identically; the render is identical by construction. Nothing to verify.
- **G2 — decode-identical.** Bytes differ (different encoder endpoint choices, preload ordering, compression), but every sampled pixel decodes to the same value and the sampler state matches. The client cannot tell these apart.
- **G3 — imperceptible.** Sampler state matches; decoded pixels differ by less than about half a level on average. This is the encoder's floating-point-order residual. Invisible in use.
- **G3b — marginal.** A larger slice of the same residual, roughly half a level to two levels average. Visible only by flipping between the two images at full zoom; not in-world.
- **G4 — non-texture value noise.** A mesh or animation bundle with no texture to render; its byte differences are mesh/clip value noise that does not touch a sampler.
- **G5 — sampler-state divergence.** Format, color space, wrap, filter, or mip count differs from the reference. Even if the pixels match, the GPU samples them differently, so the render differs. This is the tier the byte taxonomy hid inside "structural," and it deserves its own bucket because a pixel-identical texture can still land here and render wrong.
- **G6 — visible pixel divergence.** Alpha-weighted mean error past roughly two levels, or real alpha-channel divergence — a difference a person could notice. After the encoder and decode fixes, what remains here is the encoder partition wall on dense, high-frequency art, plus engine quirks the client itself reproduces (see below).
- **G7 — structural / binding.** Object count or class set differs, a pointer fails to resolve, or dimensions mismatch. The texture or material does not bind and the client renders a default. The most severe tier.
- **G8 — undecodable.** A payload that does not decode in a format the client supports. Renders broken.

## Quirk-faithful is not a defect

A texture can land in G5 or G6 against one reference and still be correct, because the client *is* Unity and reproduces the converter's import quirks. A texture used as both a color map and a normal map imports as a normal map (the converter's importer type is sticky and never downgrades), so it ships swizzled with a linear color space — a G5 difference against an older converter generation, but exactly what the current engine expects and renders as intended. The same holds for ignored EXIF orientation. When triaging G5/G6, the question is never "does this match the source image" but "does this match what the converter does with the bytes." Matching the reference's quirk is the correct outcome.

## The tool

`examples/render_assess <ours> <ref>` emits one line per bundle pair: whether the bytes are identical, whether the structure matches, the count of textures, which sampler-state fields differ, and the alpha-weighted pixel statistics (max, mean, and the fraction of samples past eight levels) plus the raw maximum and alpha-channel maximum. Decoding covers BC7, BC5, BC1, BC3, and the uncompressed formats, reading both inline and streamed payloads. A driver over a reference corpus turns those lines into the tier histogram above.

## What this measures, and what it does not

This classification reaches the decoded top-mip image and the sampler state. It does not run an actual GPU sampler, and it checks the deeper mips only at the byte level rather than re-rendering them. A texture in G6 is flagged as *potentially* visible; confirming whether a human actually sees it in-world is the job of a real client render, which is the one verification step that genuinely needs to launch the engine. Everything below G6 is safe to treat as render-equivalent without that step; G6 is the short list worth a rendered-frame check.
