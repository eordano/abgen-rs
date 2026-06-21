# abgen-rs convenience targets.
#
# parity-gate runs the per-category byte-identical regression gate against the
# checked-in floors in dev/parity_floors.json. It does NOT build corpora -- it
# verifies pre-existing output dirs (defaulting to the ones recorded per set in
# the floors file) against their Unity references. See dev/parity_gate.sh.

.PHONY: parity-gate parity-gate-val300 parity-gate-val600

# Blocking tier: val300 windows (deterministic fork-parity recipe).
parity-gate: parity-gate-val300

parity-gate-val300:
	./dev/parity_gate.sh val300-windows

# Report-only tier: val600 scenes + wearables/emotes (refs live in /tmp).
parity-gate-val600:
	./dev/parity_gate.sh val600-scenes-windows
	./dev/parity_gate.sh val600-we-windows
