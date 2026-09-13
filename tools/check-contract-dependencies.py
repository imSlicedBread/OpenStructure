"""Verify cargo metadata for the separately resolved contract consumer."""
import json
import sys

metadata = json.load(sys.stdin)
packages = {package["name"] for package in metadata["packages"]}
consumer = sys.argv[1] if len(sys.argv) > 1 else "openstructure-contract-consumer"
required = {"os-plugin-api", consumer}
unexpected = {name for name in packages if name.startswith("os-")} - {"os-plugin-api"}
if not required <= packages or unexpected:
    raise SystemExit(f"FAIL: required={sorted(required - packages)}, host dependencies={sorted(unexpected)}")
if "eframe" in packages or "uuid" in packages or "wasmi" in packages:
    raise SystemExit("FAIL: independent metadata contract pulled in a native/runtime dependency")
print(f"PASS: {len(metadata['packages'])} package versions ({len(packages)} names); contract consumer has no OpenStructure host dependencies")
