#!/usr/bin/env python3
import os
import sys
import glob
from copy import deepcopy

TRIGGER_PATHS = [
    "**/*.rs",
    "**/Cargo.lock",
    "**/Cargo.toml",
    ".cargo/config.toml",
    "assets/fonts/**/*",
    "assets/icon/*",
    "ci/deploy.sh",
]

TRIGGER_PATHS_MAC = [
    "assets/macos/**/*",
    "ci/macos-entitlement.plist",
    "get-deps",
    "ci/tag-name.sh",
]


def yv(v, depth=0):
    if v is True:
        return "true"
    if v is False:
        return "false"
    if v is None:
        return "nil"

    if isinstance(v, str):
        if "\n" in v:
            indent = "  " * depth
            result = ""
            for l in v.splitlines():
                result = result + "\n" + (f"{indent}{l}" if l else "")
            return "|" + result
        # This is hideous
        if '"' in v:
            return "'" + v + "'"
        return '"' + v + '"'

    return v


class Step(object):
    def render(self, f, depth=0):
        raise NotImplementedError(repr(self))


class RunStep(Step):
    def __init__(self, name, run, shell="bash", env=None, condition=None):
        self.name = name
        self.run = run
        self.shell = shell
        self.env = env
        self.condition = condition

    def render(self, f, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        if self.condition:
            f.write(f"{indent}  if: {self.condition}\n")
        if self.env:
            f.write(f"{indent}  env:\n")
            keys = list(self.env.keys())
            keys.sort()
            for k in keys:
                v = self.env[k]
                f.write(f"{indent}    {k}: {v}\n")
        if self.shell:
            f.write(f"{indent}  shell: {self.shell}\n")

        run = self.run

        f.write(f"{indent}  run: {yv(run, depth + 2)}\n")


class ActionStep(Step):
    def __init__(self, name, action, params=None, env=None, condition=None, id=None):
        self.name = name
        self.action = action
        self.params = params
        self.env = env
        self.condition = condition
        self.id = id

    def render(self, f, depth=0):
        indent = "  " * depth
        f.write(f"{indent}- name: {yv(self.name)}\n")
        f.write(f"{indent}  uses: {self.action}\n")
        if self.id:
            f.write(f"{indent}  id: {self.id}\n")
        if self.condition:
            f.write(f"{indent}  if: {self.condition}\n")
        if self.params:
            f.write(f"{indent}  with:\n")
            for k, v in self.params.items():
                f.write(f"{indent}    {k}: {yv(v, depth + 3)}\n")
        if self.env:
            f.write(f"{indent}  env:\n")
            for k, v in self.env.items():
                f.write(f"{indent}    {k}: {yv(v, depth + 3)}\n")


class CacheStep(ActionStep):
    def __init__(self, name, path, key, id=None):
        super().__init__(
            name, action="actions/cache@v4", params={"path": path, "key": key}, id=id
        )


class SccacheStep(ActionStep):
    def __init__(self, name):
        super().__init__(name, action="mozilla-actions/sccache-action@v0.0.9")


class CheckoutStep(ActionStep):
    def __init__(self, name="checkout repo", submodules=True, container=None):
        params = {}
        if submodules:
            params["submodules"] = "recursive"
        super().__init__(name, action=f"actions/checkout@v5", params=params)


class InstallCrateStep(ActionStep):
    def __init__(self, crate: str, key: str, version=None):
        params = {"crate": crate, "cache-key": key}
        if version is not None:
            params["version"] = version
        super().__init__(
            f"Install {crate} from Cargo",
            action="baptiste0928/cargo-install@v3",
            params=params,
        )


class Job(object):
    def __init__(self, runs_on, container=None, steps=None, env=None):
        self.runs_on = runs_on
        self.container = container
        self.steps = steps
        self.env = env

    def render(self, f, depth=0):
        f.write("\n    steps:\n")
        for s in self.steps:
            s.render(f, depth)


class Target(object):
    """A macOS build target. This fork only builds for macOS."""

    def __init__(self, name="macos", os="macos-latest", is_tag=False):
        self.name = name
        self.os = os
        self.env = {}
        self.is_tag = is_tag

    def render_env(self, f, depth=0):
        self.global_env()
        if self.env:
            indent = "    "
            f.write(f"{indent}env:\n")
            for k, v in self.env.items():
                f.write(f"{indent}  {k}: {yv(v, depth + 3)}\n")

    def install_rust(self, cache=True):
        steps = [
            RunStep(
                name="Install Rust (ARM)",
                run="rustup target add aarch64-apple-darwin",
            ),
            RunStep(
                name="Install Rust (Intel)",
                run="rustup target add x86_64-apple-darwin",
            ),
        ]
        if cache:
            steps += [
                SccacheStep(name="Compile with sccache"),
                # Cache vendored dependencies
                CacheStep(
                    name="Cache Rust Dependencies",
                    path="vendor\n.cargo/config",
                    key="cargo-deps-${{ hashFiles('**/Cargo.lock') }}",
                    id="cache-cargo-vendor",
                ),
                # Vendor dependencies
                RunStep(
                    name="Vendor dependencies",
                    condition="steps.cache-cargo-vendor.outputs.cache-hit != 'true'",
                    run="cargo vendor --locked --versioned-dirs >> .cargo/config",
                ),
            ]
        return steps

    def install_system_deps(self):
        return [
            RunStep(
                name="Install System Deps",
                run="env CI=yes PATH=$PATH ./get-deps",
            )
        ]

    def build_all_release(self):
        bin_crates = [
            "wezterm",
            "wezterm-gui",
            "wezterm-mux-server",
            "strip-ansi-escapes",
        ]
        steps = []
        for bin in bin_crates:
            steps += [
                RunStep(
                    name=f"Build {bin} (Release mode Intel)",
                    run=f"cargo build --target x86_64-apple-darwin -p {bin} --release",
                ),
                RunStep(
                    name=f"Build {bin} (Release mode ARM)",
                    run=f"cargo build --target aarch64-apple-darwin -p {bin} --release",
                ),
            ]
        return steps

    def test_all(self):
        target = " --target=x86_64-apple-darwin"
        return [
            # Install cargo-nextest
            InstallCrateStep("cargo-nextest", key=self.name),
            RunStep(name="Test", run="cargo nextest run --all --no-fail-fast" + target),
            RunStep(
                name="Test wezterm-escape-parser (no_std)",
                run="cargo nextest run -p wezterm-escape-parser" + target,
            ),
        ]

    def package(self, trusted=False):
        deploy_env = None
        if trusted:
            deploy_env = {
                "MACOS_CERT": "${{ secrets.MACOS_CERT }}",
                "MACOS_CERT_PW": "${{ secrets.MACOS_CERT_PW }}",
                "MACOS_TEAM_ID": "${{ secrets.MACOS_TEAM_ID }}",
                "MACOS_APPLEID": "${{ secrets.MACOS_APPLEID }}",
                "MACOS_APP_PW": "${{ secrets.MACOS_APP_PW }}",
            }
        return [RunStep("Package", "bash ci/deploy.sh", env=deploy_env)]

    def asset_patterns(self):
        return ["WezTerm-*.zip"]

    def upload_artifact(self, retention_days=None):
        params = {"name": self.name, "path": "\n".join(self.asset_patterns())}
        if retention_days:
            params["retention-days"] = retention_days
        return [
            ActionStep(
                "Upload artifact",
                action="actions/upload-artifact@v7",
                params=params,
            ),
        ]

    def checksum_step(self, patterns):
        return RunStep(
            "Checksum",
            f"for f in {' '.join(patterns)} ; do sha256sum $f > $f.sha256 ; done",
        )

    def upload_asset_nightly(self):
        patterns = self.asset_patterns()
        checksum = self.checksum_step(patterns)
        glob = " ".join(patterns + ["*.sha256"])
        return [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v8",
                params={"name": self.name},
            ),
            checksum,
            RunStep(
                "Upload to Nightly Release",
                f"bash ci/retry.sh gh release upload --clobber nightly {glob}",
                env={"GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}"},
            ),
        ]

    def upload_asset_tag(self):
        patterns = self.asset_patterns()
        checksum = self.checksum_step(patterns)
        glob = " ".join(patterns + ["*.sha256"])
        return [
            ActionStep(
                "Download artifact",
                action="actions/download-artifact@v8",
                params={"name": self.name},
            ),
            checksum,
            RunStep(
                "Create pre-release",
                "bash ci/retry.sh bash ci/create-release.sh $(ci/tag-name.sh)",
                env={
                    "GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                },
            ),
            RunStep(
                "Upload to Tagged Release",
                f"bash ci/retry.sh gh release upload --clobber $(ci/tag-name.sh) {glob}",
                env={
                    "GITHUB_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                },
            ),
        ]

    def update_homebrew_tap(self):
        return [
            ActionStep(
                "Checkout homebrew tap",
                action="actions/checkout@v5",
                params={
                    "repository": "wez/homebrew-wezterm",
                    "path": "homebrew-wezterm",
                    "token": "${{ secrets.GH_PAT }}",
                },
            ),
            RunStep(
                "Update homebrew tap formula",
                "cp wezterm.rb homebrew-wezterm/Casks/wezterm.rb",
            ),
            ActionStep(
                "Commit homebrew tap changes",
                action="stefanzweifel/git-auto-commit-action@v5",
                params={
                    "commit_message": "Automated update to match latest tag",
                    "repository": "homebrew-wezterm",
                },
            ),
        ]

    def global_env(self):
        self.env["CARGO_INCREMENTAL"] = "0"
        self.env["SCCACHE_GHA_ENABLED"] = "true"
        self.env["RUSTC_WRAPPER"] = "sccache"
        self.env["MACOSX_DEPLOYMENT_TARGET"] = "14.0"

    def prep_environment(self):
        steps = self.checkout()
        steps += self.install_rust()
        steps += self.install_system_deps()
        return steps

    def checkout(self, submodules=True):
        return [CheckoutStep(submodules=submodules)]

    def job(self, steps):
        return Job(runs_on=self.os, steps=steps, env=self.env)

    def pull_request(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package()
        steps += self.upload_artifact()
        return (self.job(steps), None)

    def continuous(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package(trusted=True)
        steps += self.upload_artifact(retention_days=5)

        self.env["BUILD_REASON"] = "Schedule"

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=self.checkout(submodules=False) + self.upload_asset_nightly(),
        )
        return (self.job(steps), uploader)

    def tag(self):
        steps = self.prep_environment()
        steps += self.build_all_release()
        steps += self.test_all()
        steps += self.package(trusted=True)
        steps += self.upload_artifact()

        uploader = Job(
            runs_on="ubuntu-latest",
            steps=self.checkout(submodules=False)
            + self.update_homebrew_tap()
            + self.upload_asset_tag(),
        )
        return (self.job(steps), uploader)


TARGETS = [
    Target(),
]


def generate_actions(namer, jobber, trigger, is_tag=False):
    for t in TARGETS:
        # Clone the definition, as some Target methods called
        # in the body below have side effects that we don't
        # want to bleed across into different schedule types
        t = deepcopy(t)
        t.is_tag = is_tag
        name = namer(t)
        print(name)
        job, uploader = jobber(t)

        file_name = f".github/workflows/gen_{name}.yml"

        trigger_paths = [file_name] + TRIGGER_PATHS + TRIGGER_PATHS_MAC
        trigger_paths = "- " + "\n      - ".join(yv(p) for p in sorted(trigger_paths))
        trigger_with_paths = trigger.replace("@PATHS@", trigger_paths)

        with open(file_name, "w") as f:
            f.write(
                f"""name: {name}
{trigger_with_paths}
jobs:
  build:
    runs-on: {yv(job.runs_on)}
"""
            )

            t.render_env(f)

            job.render(f, 3)

            # We upload using a native runner as github API access
            # can be unreliable and result in broken releases
            # <https://github.com/cli/cli/issues/4863>
            if uploader:
                f.write(
                    """
  upload:
    runs-on: ubuntu-latest
    needs: build
    if: github.repository == 'wezterm/wezterm'
    permissions:
      contents: write
      pages: write
      id-token: write
"""
                )
                uploader.render(f, 3)

        # Sanity check the yaml, if pyyaml is available
        try:
            import yaml

            with open(file_name) as f:
                yaml.safe_load(f)
        except ImportError:
            pass


def generate_pr_actions():
    generate_actions(
        lambda t: f"{t.name}",
        lambda t: t.pull_request(),
        trigger="""
on:
  pull_request:
    branches:
      - main
    paths:
      @PATHS@
""",
    )


def continuous_actions():
    generate_actions(
        lambda t: f"{t.name}_continuous",
        lambda t: t.continuous(),
        trigger="""
on:
  schedule:
    - cron: "10 3 * * *"
  push:
    branches:
      - main
    paths:
      @PATHS@
""",
    )


def tag_actions():
    generate_actions(
        lambda t: f"{t.name}_tag",
        lambda t: t.tag(),
        trigger="""
on:
  push:
    tags:
      - "20*"
""",
        is_tag=True,
    )


def remove_gen_actions():
    for name in glob.glob(".github/workflows/gen_*.yml"):
        os.remove(name)


remove_gen_actions()
generate_pr_actions()
continuous_actions()
tag_actions()
