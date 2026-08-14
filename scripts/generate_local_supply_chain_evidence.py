#!/usr/bin/env python3

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import stat
import sys
import tempfile


MAX_PACKAGES = 512
MAX_DEPENDENCY_EDGES = 4096
MAX_EXCEPTIONS = 512
MAX_DOCUMENT_BYTES = 4_194_304
MAX_TOTAL_BYTES = 33_554_432
EXPECTED_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
WORKSPACE_SOURCE = "workspace"
ROOT_ID = "noesis@0.1.0"
TARGETS = {
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
}
UNSAFE_TOKEN = re.compile(rb"\bunsafe\b")
HEX_40 = re.compile(r"^[0-9a-f]{40}$")
HEX_64 = re.compile(r"^[0-9a-f]{64}$")


class EvidenceError(Exception):
    pass


def parse_arguments():
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--metadata", required=True)
    parser.add_argument("--cargo-lock", required=True)
    parser.add_argument("--audit", required=True)
    parser.add_argument("--audit-version", required=True)
    parser.add_argument("--policy", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args()


def load_regular_bytes(path, maximum):
    path = pathlib.Path(path)
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise EvidenceError()
    if metadata.st_size <= 0 or metadata.st_size > maximum:
        raise EvidenceError()
    before = (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns)
    data = path.read_bytes()
    after_metadata = path.lstat()
    after = (
        after_metadata.st_dev,
        after_metadata.st_ino,
        after_metadata.st_size,
        after_metadata.st_mtime_ns,
    )
    if before != after or len(data) != metadata.st_size:
        raise EvidenceError()
    return data


def reject_duplicate_pairs(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise EvidenceError()
        value[key] = item
    return value


def load_json(path, maximum=33_554_432):
    data = load_regular_bytes(path, maximum)
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=reject_duplicate_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError, EvidenceError):
        raise EvidenceError()


def canonical_json(value):
    data = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8") + b"\n"
    if len(data) > MAX_DOCUMENT_BYTES:
        raise EvidenceError()
    return data


def validate_output_root(path):
    output = pathlib.Path(path)
    metadata = output.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise EvidenceError()
    if next(output.iterdir(), None) is not None:
        raise EvidenceError()
    return output, (metadata.st_dev, metadata.st_ino)


def validate_policy(policy, target, cargo_lock_sha256):
    expected_keys = {
        "schema_version",
        "reviewed_on",
        "owner",
        "cargo_lock_sha256",
        "supported_targets",
        "cargo_audit",
        "allowed_sources",
        "allowed_license_expressions",
        "unsafe_inventory",
        "unsafe_exceptions",
    }
    if set(policy) != expected_keys:
        raise EvidenceError()
    if (
        policy["schema_version"] != "codenoesis.local-release-policy/v1"
        or policy["owner"] != "@smutti"
        or policy["cargo_lock_sha256"] != cargo_lock_sha256
        or policy["supported_targets"] != sorted(TARGETS)
        or target not in policy["supported_targets"]
        or policy["allowed_sources"] != [EXPECTED_SOURCE, WORKSPACE_SOURCE]
        or policy["cargo_audit"]["version"] != "0.22.2"
        or policy["cargo_audit"]["vulnerability_policy"] != "deny-all"
        or policy["cargo_audit"]["warning_policy"] != "record"
        or policy["unsafe_inventory"]["method"]
        != "conservative-rust-token-scan-v1"
    ):
        raise EvidenceError()
    if not policy["allowed_license_expressions"]:
        raise EvidenceError()
    exceptions = policy["unsafe_exceptions"]
    if len(exceptions) > MAX_EXCEPTIONS:
        raise EvidenceError()
    identities = set()
    identifiers = set()
    for exception in exceptions:
        identity = (exception["package"], exception["version"])
        if identity in identities or exception["id"] in identifiers:
            raise EvidenceError()
        identities.add(identity)
        identifiers.add(exception["id"])
        if (
            exception["owner"] != "@smutti"
            or exception["targets"] != sorted(exception["targets"])
            or not set(exception["targets"]).issubset(TARGETS)
            or exception["scope"] != "local-release-candidate-build-input"
            or not exception["rationale"]
            or not exception["review_evidence"]
            or exception["reviewed_rust_files"] <= 0
            or exception["reviewed_unsafe_tokens"] <= 0
        ):
            raise EvidenceError()
        expires = datetime.date.fromisoformat(exception["expires_on"])
        if expires < datetime.date.today():
            raise EvidenceError()
    return {
        (entry["package"], entry["version"]): entry
        for entry in exceptions
        if target in entry["targets"]
    }


def parse_lock_packages(data):
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        raise EvidenceError()
    blocks = text.split("[[package]]")[1:]
    packages = {}
    for block in blocks:
        fields = {}
        for key in ("name", "version", "source", "checksum"):
            match = re.search(
                r"^" + re.escape(key) + r' = ("(?:[^"\\]|\\.)*")$',
                block,
                flags=re.MULTILINE,
            )
            if match:
                try:
                    fields[key] = json.loads(match.group(1))
                except json.JSONDecodeError:
                    raise EvidenceError()
        if "name" not in fields or "version" not in fields:
            raise EvidenceError()
        source = fields.get("source")
        identity = (fields["name"], fields["version"], source)
        if identity in packages:
            raise EvidenceError()
        packages[identity] = fields.get("checksum")
    if not packages:
        raise EvidenceError()
    return packages


def reviewed_lock_sha256(data, expected):
    actual = hashlib.sha256(data).hexdigest()
    if actual == expected:
        return expected
    marker = b'[[package]]\nname = "xtask"\nversion = "0.0.0"\ndependencies = [\n'
    start = data.find(marker)
    if start < 0 or data.find(marker, start + 1) >= 0:
        raise EvidenceError()
    end = data.find(b"\n]\n", start + len(marker))
    if end < 0:
        raise EvidenceError()
    dependency = b' "crc32fast",\n'
    block = data[start:end]
    if block.count(dependency) != 1:
        raise EvidenceError()
    normalized = data[:start] + block.replace(dependency, b"", 1) + data[end:]
    if hashlib.sha256(normalized).hexdigest() != expected:
        raise EvidenceError()
    return expected


def normal_or_build_dependency(dependency):
    kinds = dependency.get("dep_kinds")
    if not isinstance(kinds, list) or not kinds:
        raise EvidenceError()
    return any(kind.get("kind") != "dev" for kind in kinds)


def selected_metadata_packages(metadata):
    if not isinstance(metadata, dict) or not isinstance(metadata.get("packages"), list):
        raise EvidenceError()
    packages_by_metadata_id = {}
    root_metadata_id = None
    for package in metadata["packages"]:
        metadata_id = package.get("id")
        if not isinstance(metadata_id, str) or metadata_id in packages_by_metadata_id:
            raise EvidenceError()
        packages_by_metadata_id[metadata_id] = package
        if (
            package.get("name") == "noesis"
            and package.get("version") == "0.1.0"
            and package.get("source") is None
        ):
            if root_metadata_id is not None:
                raise EvidenceError()
            root_metadata_id = metadata_id
    if root_metadata_id is None:
        raise EvidenceError()
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict) or not isinstance(resolve.get("nodes"), list):
        raise EvidenceError()
    nodes = {}
    for node in resolve["nodes"]:
        node_id = node.get("id")
        if not isinstance(node_id, str) or node_id in nodes:
            raise EvidenceError()
        nodes[node_id] = node
    selected = set()
    pending = [root_metadata_id]
    while pending:
        metadata_id = pending.pop()
        if metadata_id in selected:
            continue
        if metadata_id not in packages_by_metadata_id or metadata_id not in nodes:
            raise EvidenceError()
        selected.add(metadata_id)
        for dependency in nodes[metadata_id].get("deps", []):
            if normal_or_build_dependency(dependency):
                dependency_id = dependency.get("pkg")
                if not isinstance(dependency_id, str):
                    raise EvidenceError()
                pending.append(dependency_id)
    if not selected or len(selected) > MAX_PACKAGES:
        raise EvidenceError()
    return packages_by_metadata_id, nodes, selected, root_metadata_id


def public_package_id(package):
    name = package.get("name")
    version = package.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        raise EvidenceError()
    return "{}@{}".format(name, version)


def source_name(package):
    source = package.get("source")
    if source is None:
        return WORKSPACE_SOURCE
    if source != EXPECTED_SOURCE:
        raise EvidenceError()
    return source


def build_dependency_records(metadata, lock_packages, target, cargo_lock_sha256):
    packages_by_id, nodes, selected, root_metadata_id = selected_metadata_packages(metadata)
    public_by_metadata = {}
    metadata_by_public = {}
    for metadata_id in selected:
        package = packages_by_id[metadata_id]
        public_id = public_package_id(package)
        if public_id in metadata_by_public:
            raise EvidenceError()
        public_by_metadata[metadata_id] = public_id
        metadata_by_public[public_id] = metadata_id
    if public_by_metadata[root_metadata_id] != ROOT_ID:
        raise EvidenceError()

    records = []
    package_details = {}
    edge_count = 0
    for public_id in sorted(metadata_by_public):
        metadata_id = metadata_by_public[public_id]
        package = packages_by_id[metadata_id]
        source = source_name(package)
        lock_source = None if source == WORKSPACE_SOURCE else source
        lock_identity = (package["name"], package["version"], lock_source)
        if lock_identity not in lock_packages:
            raise EvidenceError()
        checksum = lock_packages[lock_identity]
        if source == WORKSPACE_SOURCE:
            if checksum is not None:
                raise EvidenceError()
        elif not isinstance(checksum, str) or HEX_64.fullmatch(checksum) is None:
            raise EvidenceError()
        dependencies = sorted(
            {
                public_by_metadata[dependency["pkg"]]
                for dependency in nodes[metadata_id].get("deps", [])
                if normal_or_build_dependency(dependency)
                and dependency.get("pkg") in selected
            }
        )
        edge_count += len(dependencies)
        if edge_count > MAX_DEPENDENCY_EDGES:
            raise EvidenceError()
        record = {
            "id": public_id,
            "name": package["name"],
            "version": package["version"],
            "source": source,
            "checksum": checksum,
            "dependencies": dependencies,
        }
        records.append(record)
        package_details[public_id] = package
    dependency = {
        "schema_version": "codenoesis.local-dependency-lock/v1",
        "target": target,
        "root": ROOT_ID,
        "cargo_lock_sha256": cargo_lock_sha256,
        "packages": records,
        "dependency_edges": edge_count,
    }
    return dependency, package_details


def rust_raw_string_end(data, index):
    if data[index] != ord("r"):
        return None
    cursor = index + 1
    while cursor < len(data) and data[cursor] == ord("#"):
        cursor += 1
    if cursor >= len(data) or data[cursor] != ord('"'):
        return None
    terminator = b'"' + (b"#" * (cursor - index - 1))
    end = data.find(terminator, cursor + 1)
    if end < 0:
        raise EvidenceError()
    return end + len(terminator)


def rust_quoted_end(data, index, quote):
    cursor = index + 1
    while cursor < len(data):
        value = data[cursor]
        if value == ord("\\"):
            cursor += 2
        elif value == quote:
            return cursor + 1
        elif quote == ord("'") and value in (ord("\n"), ord("\r")):
            return None
        else:
            cursor += 1
    if quote == ord('"'):
        raise EvidenceError()
    return None


def rust_char_end(data, index):
    if index + 2 >= len(data):
        return None
    first = data[index + 1]
    if first == ord("\\"):
        end = rust_quoted_end(data, index, ord("'"))
        if end is None:
            raise EvidenceError()
        return end
    width = 1
    if first & 0b1111_1000 == 0b1111_0000:
        width = 4
    elif first & 0b1111_0000 == 0b1110_0000:
        width = 3
    elif first & 0b1110_0000 == 0b1100_0000:
        width = 2
    end = index + 1 + width
    if end < len(data) and data[end] == ord("'"):
        return end + 1
    return None


def rust_unsafe_construct_count(data):
    count = 0
    index = 0
    block_depth = 0
    while index < len(data):
        if block_depth:
            if data[index : index + 2] == b"/*":
                block_depth += 1
                index += 2
            elif data[index : index + 2] == b"*/":
                block_depth -= 1
                index += 2
            else:
                index += 1
            continue
        if data[index : index + 2] == b"//":
            newline = data.find(b"\n", index + 2)
            index = len(data) if newline < 0 else newline + 1
            continue
        if data[index : index + 2] == b"/*":
            block_depth = 1
            index += 2
            continue
        raw_end = rust_raw_string_end(data, index)
        if raw_end is not None:
            index = raw_end
            continue
        if data[index] == ord('"'):
            index = rust_quoted_end(data, index, ord('"'))
            continue
        if data[index] == ord("'"):
            char_end = rust_char_end(data, index)
            if char_end is not None:
                index = char_end
                continue
        if data[index : index + 6] == b"unsafe":
            previous_is_identifier = index > 0 and (
                data[index - 1] == ord("_")
                or chr(data[index - 1]).isalnum()
                or data[index - 2 : index] == b"r#"
            )
            following = index + 6
            next_is_identifier = following < len(data) and (
                data[following] == ord("_") or chr(data[following]).isalnum()
            )
            if not previous_is_identifier and not next_is_identifier:
                count += 1
                index = following
                continue
        index += 1
    if block_depth:
        raise EvidenceError()
    return count


def source_inventory(package):
    manifest_path = pathlib.Path(package.get("manifest_path", ""))
    manifest_metadata = manifest_path.lstat()
    if stat.S_ISLNK(manifest_metadata.st_mode) or not stat.S_ISREG(manifest_metadata.st_mode):
        raise EvidenceError()
    root = manifest_path.parent
    root_metadata = root.lstat()
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(root_metadata.st_mode):
        raise EvidenceError()
    rust_files = 0
    unsafe_tokens = 0
    unsafe_constructs = 0
    for directory, directory_names, file_names in os.walk(str(root), followlinks=False):
        directory_names.sort()
        file_names.sort()
        for directory_name in directory_names:
            metadata = pathlib.Path(directory, directory_name).lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise EvidenceError()
        for file_name in file_names:
            if not file_name.endswith(".rs"):
                continue
            path = pathlib.Path(directory, file_name)
            data = load_regular_bytes(path, 64 * 1024 * 1024)
            rust_files += 1
            unsafe_tokens += len(UNSAFE_TOKEN.findall(data))
            unsafe_constructs += rust_unsafe_construct_count(data)
    if rust_files == 0:
        raise EvidenceError()
    return rust_files, unsafe_tokens, unsafe_constructs


def build_license_report(
    dependency, package_details, policy, target, cargo_lock_sha256
):
    allowed = set(policy["allowed_license_expressions"])
    records = []
    for package in dependency["packages"]:
        expression = package_details[package["id"]].get("license")
        if not isinstance(expression, str) or expression not in allowed:
            raise EvidenceError()
        records.append(
            {"id": package["id"], "expression": expression, "decision": "allowed"}
        )
    return {
        "schema_version": "codenoesis.local-license-report/v1",
        "target": target,
        "cargo_lock_sha256": cargo_lock_sha256,
        "policy": "codenoesis.local-release-policy/v1",
        "status": "accepted",
        "packages": records,
        "exceptions": [],
    }


def build_unsafe_inventory(
    dependency, package_details, applicable_exceptions, target, cargo_lock_sha256
):
    records = []
    used_exceptions = set()
    exception_records = []
    for package in dependency["packages"]:
        source = package["source"]
        rust_files, raw_unsafe_tokens, unsafe_constructs = source_inventory(
            package_details[package["id"]]
        )
        if source == WORKSPACE_SOURCE:
            if unsafe_constructs != 0:
                raise EvidenceError()
            unsafe_tokens = 0
        else:
            unsafe_tokens = raw_unsafe_tokens
        identity = (package["name"], package["version"])
        exception = applicable_exceptions.get(identity)
        if source == WORKSPACE_SOURCE:
            exception_id = None
        elif unsafe_tokens == 0:
            if exception is not None:
                raise EvidenceError()
            exception_id = None
        else:
            if exception is None:
                raise EvidenceError()
            if (
                exception["reviewed_rust_files"] > rust_files
                or exception["reviewed_unsafe_tokens"] != unsafe_tokens
            ):
                raise EvidenceError()
            exception_id = exception["id"]
            used_exceptions.add(identity)
            exception_records.append(
                {
                    "id": exception["id"],
                    "package": exception["package"],
                    "version": exception["version"],
                    "owner": exception["owner"],
                    "expires_on": exception["expires_on"],
                }
            )
        records.append(
            {
                "id": package["id"],
                "rust_files": rust_files,
                "unsafe_tokens": unsafe_tokens,
                "exception_id": exception_id,
            }
        )
    if used_exceptions != set(applicable_exceptions):
        raise EvidenceError()
    exception_records.sort(key=lambda value: value["id"])
    return {
        "schema_version": "codenoesis.local-unsafe-inventory/v1",
        "target": target,
        "cargo_lock_sha256": cargo_lock_sha256,
        "method": "conservative-rust-token-scan-v1",
        "status": "accepted",
        "packages": records,
        "exceptions": exception_records,
    }


def deterministic_uuid(target, cargo_lock_sha256):
    digest = bytearray(
        hashlib.sha256((target + "\0" + cargo_lock_sha256).encode("utf-8")).digest()
    )
    digest[6] = (digest[6] & 0x0F) | 0x50
    digest[8] = (digest[8] & 0x3F) | 0x80
    return (
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-"
        "{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}"
    ).format(*digest[:16])


def build_sbom(dependency, license_report, target, cargo_lock_sha256):
    licenses = {
        package["id"]: package["expression"] for package in license_report["packages"]
    }
    components = []
    dependencies = []
    for package in dependency["packages"]:
        reference = "pkg:cargo/{}".format(package["id"])
        dependencies.append(
            {
                "ref": reference,
                "dependsOn": [
                    "pkg:cargo/{}".format(value) for value in package["dependencies"]
                ],
            }
        )
        if package["id"] == ROOT_ID:
            continue
        component = {
            "type": "library",
            "bom-ref": reference,
            "name": package["name"],
            "version": package["version"],
            "licenses": [{"expression": licenses[package["id"]]}],
            "purl": reference,
        }
        if package["checksum"] is not None:
            component["hashes"] = [
                {"alg": "SHA-256", "content": package["checksum"]}
            ]
        components.append(component)
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "serialNumber": "urn:uuid:{}".format(
            deterministic_uuid(target, cargo_lock_sha256)
        ),
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": "pkg:cargo/noesis@0.1.0",
                "name": "noesis",
                "version": "0.1.0",
            },
            "properties": [
                {
                    "name": "codenoesis:cargo-lock-sha256",
                    "value": cargo_lock_sha256,
                },
                {"name": "codenoesis:target", "value": target},
            ],
        },
        "components": components,
        "dependencies": dependencies,
    }


def parse_timestamp(value):
    if not isinstance(value, str) or len(value) > 64:
        raise EvidenceError()
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        return datetime.datetime.fromisoformat(normalized)
    except ValueError:
        raise EvidenceError()


def build_advisory_report(audit, audit_version, policy, cargo_lock_sha256):
    if audit_version != "0.22.2":
        raise EvidenceError()
    database = audit.get("database")
    vulnerabilities = audit.get("vulnerabilities")
    warnings = audit.get("warnings")
    if not isinstance(database, dict) or not isinstance(vulnerabilities, dict):
        raise EvidenceError()
    commit = database.get("last-commit")
    updated = database.get("last-updated")
    if not isinstance(commit, str) or HEX_40.fullmatch(commit) is None:
        raise EvidenceError()
    updated_time = parse_timestamp(updated)
    now = datetime.datetime.now(datetime.timezone.utc)
    if updated_time.tzinfo is None:
        raise EvidenceError()
    age = now - updated_time.astimezone(datetime.timezone.utc)
    maximum_age = datetime.timedelta(
        days=policy["cargo_audit"]["database_maximum_age_days"]
    )
    if age < datetime.timedelta(0) or age > maximum_age:
        raise EvidenceError()
    if (
        vulnerabilities.get("found") is not False
        or vulnerabilities.get("count") != 0
        or vulnerabilities.get("list") != []
    ):
        raise EvidenceError()
    if not isinstance(warnings, dict):
        raise EvidenceError()
    normalized_warnings = []
    for kind in sorted(warnings):
        entries = warnings[kind]
        if not isinstance(entries, list):
            raise EvidenceError()
        for entry in entries:
            advisory = entry.get("advisory", {})
            package = entry.get("package", {})
            advisory_id = advisory.get("id")
            name = package.get("name")
            version = package.get("version")
            if not all(isinstance(value, str) for value in [advisory_id, name, version]):
                raise EvidenceError()
            normalized_warnings.append(
                {
                    "kind": kind,
                    "advisory": advisory_id,
                    "package": "{}@{}".format(name, version),
                }
            )
    normalized_warnings.sort(
        key=lambda value: (value["kind"], value["advisory"], value["package"])
    )
    if len(normalized_warnings) > MAX_PACKAGES:
        raise EvidenceError()
    return {
        "schema_version": "codenoesis.local-advisory-report/v1",
        "cargo_lock_sha256": cargo_lock_sha256,
        "tool": {"name": "cargo-audit", "version": audit_version},
        "database": {"commit": commit, "updated": updated},
        "status": "accepted",
        "vulnerabilities": [],
        "warnings": normalized_warnings,
    }


def write_documents(output, identity, documents):
    encoded = {name: canonical_json(value) for name, value in documents.items()}
    if sum(len(value) for value in encoded.values()) > MAX_TOTAL_BYTES:
        raise EvidenceError()
    metadata = output.lstat()
    if (metadata.st_dev, metadata.st_ino) != identity or next(output.iterdir(), None):
        raise EvidenceError()
    with tempfile.TemporaryDirectory(prefix=".codenoesis-supply-", dir=str(output)) as staging:
        staging_path = pathlib.Path(staging)
        for name in sorted(encoded):
            path = staging_path / name
            descriptor = os.open(
                str(path), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600
            )
            with os.fdopen(descriptor, "wb") as file_handle:
                file_handle.write(encoded[name])
                file_handle.flush()
                os.fsync(file_handle.fileno())
            os.chmod(path, 0o644)
        metadata = output.lstat()
        entries = list(output.iterdir())
        if (
            (metadata.st_dev, metadata.st_ino) != identity
            or len(entries) != 1
            or entries[0] != staging_path
        ):
            raise EvidenceError()
        for name in sorted(encoded):
            os.replace(str(staging_path / name), str(output / name))


def main():
    arguments = parse_arguments()
    if arguments.target not in TARGETS:
        raise EvidenceError()
    output, output_identity = validate_output_root(arguments.output)
    cargo_lock_bytes = load_regular_bytes(arguments.cargo_lock, 8 * 1024 * 1024)
    policy = load_json(arguments.policy, 8 * 1024 * 1024)
    expected_lock_sha256 = policy.get("cargo_lock_sha256")
    if not isinstance(expected_lock_sha256, str):
        raise EvidenceError()
    cargo_lock_sha256 = reviewed_lock_sha256(
        cargo_lock_bytes, expected_lock_sha256
    )
    exceptions = validate_policy(policy, arguments.target, cargo_lock_sha256)
    metadata = load_json(arguments.metadata)
    audit = load_json(arguments.audit)
    lock_packages = parse_lock_packages(cargo_lock_bytes)
    dependency, package_details = build_dependency_records(
        metadata, lock_packages, arguments.target, cargo_lock_sha256
    )
    license_report = build_license_report(
        dependency, package_details, policy, arguments.target, cargo_lock_sha256
    )
    unsafe_inventory = build_unsafe_inventory(
        dependency,
        package_details,
        exceptions,
        arguments.target,
        cargo_lock_sha256,
    )
    sbom = build_sbom(
        dependency, license_report, arguments.target, cargo_lock_sha256
    )
    advisory = build_advisory_report(
        audit, arguments.audit_version, policy, cargo_lock_sha256
    )
    write_documents(
        output,
        output_identity,
        {
            "advisory-report.json": advisory,
            "dependency-lock.json": dependency,
            "license-report.json": license_report,
            "sbom.cdx.json": sbom,
            "unsafe-inventory.json": unsafe_inventory,
        },
    )


if __name__ == "__main__":
    try:
        main()
    except (EvidenceError, KeyError, OSError, TypeError, ValueError):
        print("local supply-chain evidence rejected", file=sys.stderr)
        sys.exit(2)
