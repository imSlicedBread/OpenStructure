"""Validate Cargo license metadata; this is not legal approval or license selection."""
import json
import sys

metadata = json.load(sys.stdin)
missing = [p["name"] for p in metadata["packages"] if not (p.get("license") or p.get("license_file"))]
publishable = [p["name"] for p in metadata["packages"] if p["id"] in metadata["workspace_members"] and p.get("publish") != []]
if missing or publishable:
    sys.exit(f"Missing license metadata: {missing}; workspace packages must remain non-publishable: {publishable}")
print(f"PASS: {len(metadata['packages'])} packages have license metadata; workspace publishing disabled.")
print("Project license selection and dependency legal review remain pending.")
