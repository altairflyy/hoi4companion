# Change-driven live updates

The live monitor publishes state changes through SSE. Heartbeats maintain transport health without turning old observations into fresh state. A disconnected stream or invalid sample must invalidate dependent recommendations.

Change-driven delivery does not mean the game exposes a public event API or that sampling has zero latency. Native observations and watch plans still have acquisition costs. Compare identity, sequence, freshness, and reconnect behavior separately.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/change-driven-verification.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
