"""Verify a Stage 4 portable bundle and preserve the selected project source.

Uses only the standard library. Does not install software or grant application
authority. Run after acceptance, with no concurrent build or source mutation.
"""

import argparse
import base64
import hashlib
import json
import pathlib
import struct
import subprocess


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(4 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def save_new(path, data):
    with path.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(data, output, indent=2)
        output.write("\n")


def pe(path):
    data = path.read_bytes()
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[:2] != b"MZ" or data[offset : offset + 4] != b"PE\0\0":
        raise ValueError("Native PE required")
    optional = offset + 24
    flags = struct.unpack_from("<H", data, optional + 70)[0]
    directory = optional + 112
    forbidden = [
        b"CUSTODIAN_EDITOR_FAULT_ROOT",
        b"CUSTODIAN_APP_WORKFLOW_ROOT",
        b"CUSTODIAN_SESSION_FIXTURE",
        b"CUSTODIAN_APP_WORKER_FIXTURE",
        b"fixture-identity.json",
        b"crash-recover",
    ]
    hooks = [
        name.decode()
        for name in forbidden
        if name in data or name.decode().encode("utf-16le") in data
    ]
    if hooks:
        raise ValueError("Test-only fixture hooks in native artifact: " + repr(hooks))
    return {
        "sha256": sha(path),
        "bytes": len(data),
        "pe64": struct.unpack_from("<H", data, optional)[0] == 0x20B,
        "aslr": bool(flags & 0x40),
        "dep": bool(flags & 0x100),
        "guardCF": bool(flags & 0x4000),
        "certificateTablePresent": bool(struct.unpack_from("<II", data, directory + 32)[1]),
        "checkedTestHooksAbsent": [name.decode() for name in forbidden],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("--installer", type=pathlib.Path)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[1]
    bundle = args.bundle.resolve(strict=True)
    if args.output.exists() or not args.output.is_absolute():
        parser.error("Fresh absolute evidence directory required")
    args.output.mkdir()
    runtime = bundle / "runtime"
    manifest = json.loads((runtime / "runtime-manifest.json").read_text())
    worker = runtime / "bin/custodian-appworker.exe"
    if sha(worker) != manifest["applicationWorker"]["sha256"]:
        raise ValueError("Packaged worker pin mismatch")
    model = runtime / "models" / manifest["model"]
    if sha(model) != manifest["sha256"]:
        raise ValueError("Packaged model pin mismatch")
    for name in ["package.json", "extension.cjs", "wire.cjs"]:
        if sha(runtime / "editor-adapter" / name) != sha(root / "services/editor-adapter" / name):
            raise ValueError("Packaged editor extension differs from selected source")
    modules = json.loads((root / "docs/EVIDENCE/stage-3/agent-modules.json").read_text())
    for module in modules:
        for directory in [root / "services/agent/dist", runtime / "agent"]:
            if sha(directory / module["file"]) != module["sha256"].lower():
                raise ValueError("Accepted agent module changed: " + module["file"])
    frozen = root / "docs/EVIDENCE/stage-4/app-acceptance-fixtures.json"
    lock = json.loads(frozen.with_name("app-acceptance-fixtures-lock.json").read_text())
    if sha(frozen) != lock["sha256"]:
        raise ValueError("Frozen acceptance expectations changed")
    original = json.loads((root / "target/stage-4/user-work-before-review.json").read_text())
    protected_names = {
        ".dockerignore",
        "Dockerfile",
        "Makefile",
        "services/orchestrator/src/workflows.ts",
        "apps/desktop/src-tauri/src/assistant/apps.rs",
        "apps/desktop/src-tauri/src/assistant/reply.rs",
    }
    protected = [entry for entry in original["files"] if entry["path"] in protected_names]
    if len(protected) != len(protected_names):
        raise ValueError("Protected user baseline incomplete")
    verified = []
    for entry in protected:
        if sha(root / entry["path"]) != entry["sha256"].lower():
            raise ValueError("Protected user work changed: " + entry["path"])
        verified.append(entry["path"])
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if head != original["head"]:
        raise ValueError("Git base changed")
    historical = {
        "target/release/custodian-desktop.exe": (
            "1841cb92fc7d0c7e7948d3572b858e1595097003a84be5f91903c523b79889ad"
        ),
        "target/release/custodian-appworker.exe": (
            "831087c0ba746178f85082518d87e26d56f19fabf4945de7eb15c371322b77dc"
        ),
        "target/stage-0/stage-3-accepted-source-snapshot.json": (
            "c9988443209ad50760c7c400c52b38c857e9e3efa07642efb08033739eb49699"
        ),
        "target/stage-4/foundation-source-snapshot.json": (
            "346b189ff85eb628c02542c8facd5a64be1e1a28ffd057f0f671a76ee3c555a6"
        ),
        "target/stage-4-worker/source-snapshot.json": (
            "2d0f0bcd5e9c456949597a103af351af5ab050973ff1fff8335dcb63f2eaff77"
        ),
        "target/stage-4-privacy/source-snapshot.json": (
            "aafddb72f005f63f9e93d9fcd605883b5eb350afeac87b84ea8d2bd657081e04"
        ),
    }
    for name, expected in historical.items():
        if sha(root / name) != expected:
            raise ValueError("Historical accepted artifact changed: " + name)
    names = (
        subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z"], cwd=root)
        .decode()
        .split("\0")
    )
    sources = []
    for name in sorted(set(filter(None, names))):
        relative = pathlib.PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Unsafe source name")
        path = root / relative
        if not path.is_file() or path.is_symlink():
            continue
        # Evidence logs are delivered separately; preserve source and frozen contracts.
        if (
            name.startswith("docs/EVIDENCE/")
            and name != frozen.relative_to(root).as_posix()
            and name
            != frozen.with_name("app-acceptance-fixtures-lock.json").relative_to(root).as_posix()
        ):
            continue
        content = path.read_bytes()
        sources.append(
            {
                "path": name,
                "bytes": len(content),
                "sha256": hashlib.sha256(content).hexdigest(),
                "base64": base64.b64encode(content).decode(),
            }
        )
    snapshot = args.output / "source-snapshot.json"
    save_new(
        snapshot,
        {
            "version": 1,
            "baseCommit": head,
            "description": (
                "Selected dirty source and locks; excludes dependencies, model and generated "
                "evidence. Not a clean Git checkout."
            ),
            "files": sources,
        },
    )
    restored = args.output / "restored-source"
    restored.mkdir()
    for entry in json.loads(snapshot.read_text())["files"]:
        target = restored / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as output:
            output.write(base64.b64decode(entry["base64"], validate=True))
        if sha(target) != entry["sha256"] or sha(root / entry["path"]) != entry["sha256"]:
            raise ValueError("Source changed during snapshot or restore")
    files = []
    for path in sorted(runtime.rglob("*")):
        if path.is_symlink():
            raise ValueError("Runtime link requires separate qualification")
        if path.is_file():
            files.append(
                {
                    "path": path.relative_to(bundle).as_posix(),
                    "bytes": path.stat().st_size,
                    "sha256": sha(path),
                }
            )
    tree = args.output / "runtime-files.json"
    save_new(tree, files)
    result = {
        "desktop": pe(bundle / "Custodian.exe"),
        "worker": pe(worker),
        "runtimeFiles": len(files),
        "runtimeManifestSha256": sha(tree),
        "modelPinVerified": True,
        "workerPinVerified": True,
        "unchangedAgentModules": len(modules),
        "protectedUserFilesVerified": verified,
        "historicalArtifactsVerified": historical,
        "baseCommit": head,
        "frozenAcceptanceSha256": sha(frozen),
        "sourceFiles": len(sources),
        "sourceSnapshotSha256": sha(snapshot),
        "sourceRestoredAndVerified": True,
        "installationAccepted": False,
        "installationNote": "Artifact verification does not execute or accept installation.",
    }
    if args.installer:
        result["installer"] = {
            "path": str(args.installer),
            "sha256": sha(args.installer),
            "bytes": args.installer.stat().st_size,
        }
    save_new(args.output / "artifact-verification.json", result)
    print(
        json.dumps(
            {
                key: result[key]
                for key in [
                    "runtimeFiles",
                    "unchangedAgentModules",
                    "sourceFiles",
                    "sourceRestoredAndVerified",
                    "installationAccepted",
                ]
            }
        )
    )


if __name__ == "__main__":
    main()
