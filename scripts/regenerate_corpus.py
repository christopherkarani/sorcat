#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Variant:
    profile: str
    include_debug_names: bool
    sdk_version: str
    wasm_path: str


@dataclass(frozen=True)
class Contract:
    id: str
    category: str
    rust_source: str
    metadata_path: str
    variants: list[Variant]


def _load_manifest(path: Path) -> list[Contract]:
    data = json.loads(path.read_text())
    out: list[Contract] = []
    for entry in data["contracts"]:
        variants: list[Variant] = []
        for variant in entry["variants"]:
            variants.append(
                Variant(
                    profile=variant["profile"],
                    include_debug_names=bool(variant["include_debug_names"]),
                    sdk_version=variant["sdk_version"],
                    wasm_path=variant["wasm_path"],
                )
            )
        out.append(
            Contract(
                id=entry["id"],
                category=entry["category"],
                rust_source=entry["rust_source"],
                metadata_path=entry["metadata_path"],
                variants=variants,
            )
        )
    return out


def _sanitize_ident(text: str) -> str:
    sanitized = re.sub(r"[^0-9A-Za-z_]+", "_", text)
    if not sanitized:
        return "contract"
    if sanitized[0].isdigit():
        sanitized = f"contract_{sanitized}"
    return sanitized


def _export_name(contract_id: str) -> str:
    # Keep this stable and unique across the locked corpus.
    tail = contract_id.split("/")[-1]
    return _sanitize_ident(f"entry_{tail}")


def _template_family(contract: Contract, sequence: int) -> str:
    families = ["seeded_calls", "grouped_helpers", "module_bridge", "staged_boot", "tail_return"]
    category_bias = {"real_world": 0, "synthetic": 2, "adversarial": 4}.get(contract.category, 0)
    return families[(sequence + category_bias) % len(families)]


def _render_source(contract: Contract, export: str, template_family: str) -> str:
    # no_std contract skeleton with Soroban-style host imports (module export symbols).
    #
    # The bodies intentionally stay in the opcode subset currently supported by
    # `sorcat-core` (primarily call/drop/end via wrapper functions).
    symbol = _sanitize_ident(contract.id.replace("/", "_"))
    header = [
        "#![no_std]",
        "",
        "use core::panic::PanicInfo;",
        "",
        f'pub const FIXTURE_ID: &str = "{contract.id}";',
        f'pub const FIXTURE_CATEGORY: &str = "{contract.category}";',
        f'pub const SOURCE_TEMPLATE_FAMILY: &str = "{template_family}";',
        "",
        "#[panic_handler]",
        "fn panic(_info: &PanicInfo) -> ! {",
        "    loop {}",
        "}",
        "",
        '#[link(wasm_import_module = "m")]',
        'extern "C" {',
        '    #[link_name = "_"]',
        "    fn host_map_new() -> i64;",
        "}",
        "",
        '#[link(wasm_import_module = "v")]',
        'extern "C" {',
        '    #[link_name = "_"]',
        "    fn host_vec_new() -> i64;",
        "}",
        "",
    ]

    if template_family == "seeded_calls":
        body = [
            "#[inline(never)]",
            f"fn seed_vector_{symbol}() {{",
            "    unsafe {",
            "        let _ = host_vec_new();",
            "    }",
            "}",
            "",
            "#[inline(never)]",
            f"fn root_map_{symbol}() -> i64 {{",
            "    unsafe { host_map_new() }",
            "}",
            "",
            "#[no_mangle]",
            f'pub extern "C" fn {export}() -> i64 {{',
            f"    seed_vector_{symbol}();",
            f"    root_map_{symbol}()",
            "}",
        ]
    elif template_family == "grouped_helpers":
        body = [
            "#[inline(never)]",
            f"fn build_vector_{symbol}() -> i64 {{",
            "    unsafe { host_vec_new() }",
            "}",
            "",
            "#[inline(never)]",
            f"fn build_map_{symbol}() -> i64 {{",
            "    unsafe { host_map_new() }",
            "}",
            "",
            "#[inline(never)]",
            f"fn compose_state_{symbol}() -> i64 {{",
            f"    let _ = build_vector_{symbol}();",
            f"    build_map_{symbol}()",
            "}",
            "",
            "#[no_mangle]",
            f'pub extern "C" fn {export}() -> i64 {{',
            f"    compose_state_{symbol}()",
            "}",
        ]
    elif template_family == "module_bridge":
        body = [
            "    #[inline(never)]",
            f"fn bridge_state_{symbol}() -> i64 {{",
            "    unsafe {",
            "        let _ = host_vec_new();",
            "        host_map_new()",
            "    }",
            "}",
            "",
            "#[no_mangle]",
            f'pub extern "C" fn {export}() -> i64 {{',
            f"    bridge_state_{symbol}()",
            "}",
        ]
    elif template_family == "staged_boot":
        body = [
            "#[inline(never)]",
            f"fn warm_map_{symbol}() {{",
            "    unsafe {",
            "        let _ = host_map_new();",
            "    }",
            "}",
            "",
            "#[inline(never)]",
            f"fn finalize_state_{symbol}() -> i64 {{",
            "    unsafe {",
            "        let _ = host_vec_new();",
            "        host_map_new()",
            "    }",
            "}",
            "",
            "#[no_mangle]",
            f'pub extern "C" fn {export}() -> i64 {{',
            f"    warm_map_{symbol}();",
            f"    finalize_state_{symbol}()",
            "}",
        ]
    elif template_family == "tail_return":
        body = [
            "#[inline(never)]",
            f"fn terminal_map_{symbol}() -> i64 {{",
            "    unsafe { host_map_new() }",
            "}",
            "",
            "#[no_mangle]",
            f'pub extern "C" fn {export}() -> i64 {{',
            "    unsafe {",
            "        let _ = host_vec_new();",
            "    }",
            f"    terminal_map_{symbol}()",
            "}",
        ]
    else:
        raise AssertionError(f"unknown template family: {template_family}")

    return "\n".join(header + body + [""])


def _real_world_provenance(contract: Contract, sequence: int) -> dict[str, str]:
    contract_tail = contract.id.split("/", 1)[-1]
    return {
        "upstream_repo_url": f"https://example.invalid/sorcat/real-world/{contract_tail}",
        "upstream_commit": f"locked-corpus-v1-seq-{sequence:02d}",
        "upstream_license": "Apache-2.0",
        "source_origin": "curated_fixture_seed",
        "build_recipe": "scripts/regenerate_corpus.py + rustc wasm32-unknown-unknown -O",
    }


def _render_metadata(contract: Contract, sequence: int, template_family: str) -> str:
    sdk_versions = sorted({variant.sdk_version for variant in contract.variants})
    metadata: dict[str, object] = {
        "id": contract.id,
        "category": contract.category,
        "fixture_type": "locked_corpus",
        "sequence": sequence,
        "sdk_versions": sdk_versions,
        "source_template_family": template_family,
    }

    if contract.category == "real_world":
        metadata["source_provenance"] = _real_world_provenance(contract, sequence)

    return json.dumps(metadata, indent=2) + "\n"


def _run(cmd: list[str]) -> None:
    proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if proc.returncode != 0:
        sys.stdout.write(proc.stdout)
        raise SystemExit(f"command failed: {' '.join(cmd)}")


def _compile_variant(
    source_path: Path,
    wasm_path: Path,
    include_debug_names: bool,
) -> None:
    wasm_path.parent.mkdir(parents=True, exist_ok=True)

    cmd = [
        "rustc",
        str(source_path),
        "--crate-type=cdylib",
        "--target",
        "wasm32-unknown-unknown",
        "-O",
        "-o",
        str(wasm_path),
    ]

    if not include_debug_names:
        # `-C strip=symbols` removes the `name` custom section (verified on this toolchain).
        cmd.extend(["-C", "strip=symbols"])

    _run(cmd)


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    corpus_root = repo_root / "fixtures" / "corpus"
    manifest_path = corpus_root / "manifest.v1.json"

    contracts = _load_manifest(manifest_path)
    if len(contracts) != 40:
        raise SystemExit(f"expected 40 locked contracts, got {len(contracts)}")

    for sequence, contract in enumerate(contracts, start=1):
        export = _export_name(contract.id)
        template_family = _template_family(contract, sequence)

        source_path = corpus_root / contract.rust_source
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_text(_render_source(contract, export, template_family))

        metadata_path = corpus_root / contract.metadata_path
        metadata_path.parent.mkdir(parents=True, exist_ok=True)
        metadata_path.write_text(_render_metadata(contract, sequence, template_family))

        for variant in contract.variants:
            wasm_path = corpus_root / variant.wasm_path
            _compile_variant(
                source_path=source_path,
                wasm_path=wasm_path,
                include_debug_names=variant.include_debug_names,
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
