# Contributing to OpenStructure

OpenStructure welcomes contributions to an independent, open-source BIM platform.

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

The project will adopt a documented contributor policy before accepting substantial external contributions. Until then, contributors should retain the right to license their submitted work under the project’s selected license.
