# Contributing to OpenStructure

OpenStructure welcomes contributions and independently authored plugins. The
project is licensed under the Apache License 2.0; read [LICENSE](LICENSE) and
[NOTICE](NOTICE) before redistributing it. Contributions are accepted under the
same license, as described below.

## Before contributing

- Read `README.md` and `docs/architecture.md`.
- Keep changes focused and testable.
- Do not submit Autodesk/Revit code, assets, screenshots, proprietary documentation, or reverse-engineered proprietary schemas.
- Use public standards and independently authored examples.

## Development expectations

- Keep the workspace compiling.
- Add tests for behavior changes.
- Document public APIs.
- Preserve transaction boundaries and stable IDs.
- Do not bypass the plugin API or write directly to project storage from plugins.
- Record significant architectural decisions in `docs/decisions/`.

## Commit guidance

Use clear, focused commits. A useful format is:

```text
area: short description
```

Examples:

```text
model: add stable wall identity
plugin: validate manifest permissions
storage: add project schema version
```

## Contributions and licensing

Contributors retain ownership of their original work. By intentionally
submitting a contribution for inclusion, you offer it under Apache License 2.0,
as described in Section 5 of that license. You confirm that you have the right to
submit the work under those terms. No separate contributor agreement is required
for contributions accepted under this policy.

An independently authored plugin is not a contribution to the host merely
because it uses the published plugin interface. Plugin authors choose the terms
for their original plugin code. OpenStructure's API and examples are themselves
available under Apache 2.0.
