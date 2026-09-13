# GitHub setup

The project repository is [imSlicedBread/OpenStructure](https://github.com/imSlicedBread/OpenStructure).

## Clone and build

```powershell
git clone https://github.com/imSlicedBread/OpenStructure.git
cd OpenStructure
cargo build -p os-app --locked
cargo run -p os-app --locked
```

The repository is private, so authenticate with a GitHub account that has access.
Git Credential Manager can open browser authentication when cloning or pushing.
Never place a password or access token in the remote URL.

## Contribute changes

```powershell
git switch -c codex/your-change
# Make and verify your changes.
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
git add <changed-files>
git commit -m "area: describe the change"
git push -u origin codex/your-change
```

Open a pull request against `main` on GitHub. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Continuous integration

[The CI workflow](.github/workflows/ci.yml) runs on pushes and pull requests.
It includes Windows/Linux formatting, Clippy, tests, independent plugin probes,
a dependency audit, and IFC validation. Results appear in the repository's Actions tab.

## Optional repository settings

After the first successful CI run, consider protecting `main`, requiring pull
requests and passing checks, and enabling dependency alerts and Discussions.
These settings are managed in GitHub and are not configured by this document.

The license decision remains pending; see [LICENSE.pending](LICENSE.pending).
