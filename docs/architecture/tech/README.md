# Technical Architecture Directory

This directory owns the technical architecture Canon for the repository.

## Fixed Entry

- [TECH_ARCHITECTURE.md](TECH_ARCHITECTURE.md) — required entry document. Keep summary, status, and links here.

## Active Shards

- [Modules and contracts](TECH-modules-and-contracts.md)
- [Runtime topology](TECH-runtime-topology.md)
- [Security and operations](TECH-security-and-operations.md)
- [Runtime backends, pools, and state materialization](TECH-runtime-backends-and-pools.md)
- [Performance and capacity](TECH-performance-and-capacity.md)
- [Performance and host-capability baseline](TECH-performance-baseline.md)
- [Platform support and host capability](TECH-platform-support.md)
- [E2B capability parity audit](TECH-e2b-capability-parity.md)

## Splitting Rules

- Split large architecture content into sibling shards named `TECH-<kebab-topic>.md`.
- Every shard `MUST` be linked from `TECH_ARCHITECTURE.md`.
- Do not create competing architecture roots such as `docs/architecture/TECH_ARCHITECTURE.md`; that path is retired and redirect-only.

See `DOCUMENTATION_SPEC.md` section 2.2.
