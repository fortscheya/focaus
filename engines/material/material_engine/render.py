"""Constrained page preview rendering helpers."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Dict, Optional, Tuple

from .common import ExtractionContext, svg_placeholder


class PageRenderer:
    """Render pages without changing the source asset.

    Rendering is intentionally best effort.  Structured extraction is kept
    independent from preview rendering so a missing helper cannot turn a
    usable text page into a false success or block another page.
    """

    def __init__(self, ctx: ExtractionContext) -> None:
        self.ctx = ctx
        self.work_root = ctx.staging_root / ".render"
        self.work_root.mkdir(parents=True, exist_ok=True)
        self._pptx_pdf: Optional[Path] = None
        self._diagnostics = []

    @property
    def diagnostics(self):
        return list(self._diagnostics)

    def _executable(self, env_name: str, default_name: str) -> Optional[str]:
        configured = os.environ.get(env_name)
        if configured:
            path = Path(configured).expanduser()
            if path.is_file() and os.access(str(path), os.X_OK):
                return str(path)
            self._diagnostics.append("{} is not executable".format(env_name))
            return None
        return shutil.which(default_name)

    def _run(self, args, timeout: float = 120.0) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(arg) for arg in args],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=timeout,
        )

    def _ensure_pptx_pdf(self) -> Optional[Path]:
        if self._pptx_pdf is not None:
            return self._pptx_pdf
        office = self._executable("FOCAUS_OFFICE_RENDERER", "soffice")
        if not office:
            self._diagnostics.append("office_renderer_unavailable")
            return None
        output_dir = self.work_root / "office"
        output_dir.mkdir(parents=True, exist_ok=True)
        user_profile = self.work_root / "office-profile"
        user_profile.mkdir(parents=True, exist_ok=True)
        # LibreOffice uses a file URL for its isolated profile.  The source
        # path is passed as one argument and never interpolated into a shell.
        profile_url = "file://{}".format(user_profile.resolve())
        try:
            result = self._run(
                [
                    office,
                    "--headless",
                    "--norestore",
                    "--nolockcheck",
                    "-env:UserInstallation={}".format(profile_url),
                    "--convert-to",
                    "pdf",
                    "--outdir",
                    output_dir,
                    self.ctx.source_path,
                ],
                timeout=180.0,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            self._diagnostics.append("office_renderer_failed:{}".format(type(exc).__name__))
            return None
        if result.returncode != 0:
            self._diagnostics.append("office_renderer_exit:{}".format(result.returncode))
            return None
        candidates = sorted(output_dir.glob("*.pdf"))
        if not candidates:
            self._diagnostics.append("office_renderer_no_pdf")
            return None
        self._pptx_pdf = candidates[0]
        return self._pptx_pdf

    def render_page(
        self,
        page_index: int,
        width: float,
        height: float,
        text: str,
        *,
        source_kind: str,
    ) -> Tuple[bytes, str, str]:
        pdf_path = self.ctx.source_path if source_kind == "pdf" else self._ensure_pptx_pdf() if source_kind == "pptx" else None
        renderer = self._executable("FOCAUS_PDF_RENDERER", "pdftoppm")
        if pdf_path is not None and renderer:
            prefix = self.work_root / "page-{}".format(page_index)
            try:
                result = self._run(
                    [
                        renderer,
                        "-singlefile",
                        "-png",
                        "-r",
                        "144",
                        "-f",
                        str(page_index + 1),
                        "-l",
                        str(page_index + 1),
                        pdf_path,
                        prefix,
                    ],
                    timeout=120.0,
                )
            except (OSError, subprocess.TimeoutExpired) as exc:
                self._diagnostics.append("pdf_renderer_failed:{}".format(type(exc).__name__))
            else:
                rendered = prefix.with_suffix(".png")
                if result.returncode == 0 and rendered.is_file():
                    return rendered.read_bytes(), "image/png", "renderer"
                self._diagnostics.append("pdf_renderer_exit:{}".format(result.returncode))
        elif pdf_path is None and source_kind == "pptx":
            self._diagnostics.append("pptx_pdf_conversion_unavailable")
        elif renderer is None:
            self._diagnostics.append("pdf_renderer_unavailable")

        self.ctx.add_warning("preview_renderer_fallback")
        return svg_placeholder(width, height, text, title="Focaus page preview (fallback)"), "image/svg+xml", "placeholder"

    def close(self) -> None:
        # The staging directory belongs to the host.  Only remove our private
        # render scratch tree, leaving resources and checkpoint files intact.
        shutil.rmtree(self.work_root, ignore_errors=True)
