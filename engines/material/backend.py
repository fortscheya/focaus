"""Minimal PEP 517 backend for the dependency-free sidecar package.

The repository runs the sidecar from its checkout during development.  This
backend deliberately refuses to build a wheel because the host packages the
sidecar with its own runtime and the package has no third-party dependencies.
"""

__all__ = ["build_editable", "build_sdist", "build_wheel", "get_requires_for_build_editable", "get_requires_for_build_sdist", "get_requires_for_build_wheel", "prepare_metadata_for_build_editable", "prepare_metadata_for_build_wheel"]


def _unsupported(*_args, **_kwargs):
    raise RuntimeError("The material sidecar is launched from its managed source package")


build_editable = _unsupported
build_sdist = _unsupported
build_wheel = _unsupported
get_requires_for_build_editable = lambda *_args, **_kwargs: []
get_requires_for_build_sdist = lambda *_args, **_kwargs: []
get_requires_for_build_wheel = lambda *_args, **_kwargs: []
prepare_metadata_for_build_editable = _unsupported
prepare_metadata_for_build_wheel = _unsupported
