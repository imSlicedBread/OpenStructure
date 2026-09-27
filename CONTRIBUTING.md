# Contributing to OpenStructure

OpenStructure welcomes proposed contributions and independently authored plugins.
The public host repository is proprietary; read [LICENSE](LICENSE) before using
its code. The plugin API and independent examples have a limited permission for
compatible plugin development.

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

## Contributor agreement

Open issues and pull requests are welcome for review. Contributors keep ownership
of their original work. Before merging a code or documentation contribution,
the contributor and maintainers must agree in writing on permission to use,
modify, distribute, and sublicense that contribution as part of OpenStructure.
Opening a pull request alone does not grant those rights or guarantee acceptance.
Do not submit material that you cannot authorize for this use.

An independently authored plugin is not a contribution to the host merely
because it uses the published plugin interface. Plugin authors choose their own
terms for their original plugin code, subject to the limited SDK permission in
[LICENSE](LICENSE) if they use the provided API or examples.
