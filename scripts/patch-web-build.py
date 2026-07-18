#!/usr/bin/env python3
from __future__ import annotations

import glob
import os
import re
import sys

STAGING = os.environ.get("TRUNK_STAGING_DIR", "dist")

JS_REPLACEMENTS = [
    ("arg0.exitPointerLock();", "if (arg0.exitPointerLock) arg0.exitPointerLock();"),
    ("arg0.requestPointerLock();", "if (arg0.requestPointerLock) arg0.requestPointerLock();"),
    ("arg0.exitFullscreen();", "if (arg0.exitFullscreen) arg0.exitFullscreen();"),
    (
        "arg0.webkitExitFullscreen();",
        "if (arg0.webkitExitFullscreen) arg0.webkitExitFullscreen();",
    ),
    (
        "arg0.webkitRequestFullscreen();",
        "if (arg0.webkitRequestFullscreen) arg0.webkitRequestFullscreen();",
    ),
    (
        "const ret = arg0.requestFullscreen();",
        "const ret = arg0.requestFullscreen ? arg0.requestFullscreen() : Promise.resolve();",
    ),
]


def patch_js(path: str) -> None:
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    for old, new in JS_REPLACEMENTS:
        text = text.replace(old, new)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


def patch_html(path: str) -> None:
    with open(path, encoding="utf-8") as handle:
        html = handle.read()

    module_pattern = re.compile(
        r'(<script type="module"[^>]*>)(.*?)(</script>)',
        re.DOTALL,
    )
    match = module_pattern.search(html)
    if not match:
        print("patch-web-build: module script not found", file=sys.stderr)
        return

    body = match.group(2).strip()
    import_match = re.match(
        r"^(import\s+init,\s*\*\s*as\s+bindings\s+from\s+[^;]+;\s*)",
        body,
        re.DOTALL,
    )
    if not import_match:
        print("patch-web-build: import line not found", file=sys.stderr)
        return

    import_line = import_match.group(1)
    rest = body[import_match.end() :].strip()

    # Drop any legacy try/catch wrapper from older patches.
    if rest.startswith("try {"):
        rest = rest[len("try {") :].strip()
        catch_idx = rest.rfind("} catch (e) {")
        if catch_idx != -1:
            rest = rest[:catch_idx].strip()

    if "Downloading game engine" in rest:
        patched_body = f"{import_line}\n\n{rest}"
    else:
        rest = rest.replace(
            "const wasm = await init",
            (
                "document.getElementById('boot-status') && "
                "(document.getElementById('boot-status').textContent = "
                "'Downloading game engine (~21MB)…');\n"
                "const wasm = await init"
            ),
        )
        rest = rest.replace(
            'dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));',
            (
                "document.getElementById('boot-status')?.setAttribute('hidden', '');\n"
                'dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));'
            ),
        )
        patched_body = f"{import_line}\n\n{rest}"

    patched = f"{match.group(1)}{patched_body}{match.group(3)}"
    html = html[: match.start()] + patched + html[match.end() :]

    with open(path, "w", encoding="utf-8") as handle:
        handle.write(html)


def main() -> None:
    for js_path in glob.glob(os.path.join(STAGING, "lunar-*.js")):
        patch_js(js_path)

    html_path = os.path.join(STAGING, "index.html")
    if os.path.isfile(html_path):
        patch_html(html_path)


if __name__ == "__main__":
    main()
