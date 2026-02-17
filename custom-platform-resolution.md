# Custom Platform Resolution

## Overview

When `platform = <url>` is set in `platformio.ini`, PlatformIO bypasses the registry lookup and instead installs the platform directly from a VCS/URI source. The raw value is parsed into a `PackageSpec` with `spec.external = True`, which routes the install through the VCS checkout path rather than the standard registry path.

Example triggers:

```ini
[env:myenv]
platform = https://github.com/platformio/platform-espressif32.git
platform = git+https://github.com/platformio/platform-espressif32.git#master
platform = https://github.com/platformio/platform-espressif32
```

---

## Code Path

```
platformio.ini
  └─ ProjectConfig.get()                        [platformio/project/config.py]
       └─ PlatformFactory.from_env()            [platformio/platform/factory.py:95]
            └─ PlatformFactory.new(spec)        [platformio/platform/factory.py:44]
                 └─ PlatformPackageManager().install(spec)
                      └─ BasePackageManager._install(spec)  [platformio/package/manager/_install.py:54]
                           ├─ PackageSpec._parse_uri()      [platformio/package/meta.py:380]
                           │    → sets spec.uri, spec.external = True
                           └─ install_from_uri(spec.uri)    [platformio/package/manager/_install.py:173]
                                └─ VCSClientFactory.new(tmp_dir, uri)  [platformio/package/vcsclient.py:30]
                                     └─ GitClient.export()             [platformio/package/vcsclient.py:185]
                                          → git clone --recursive [--depth 1] [--branch <tag>] <url> <tmp_dir>
                                └─ _install_tmp_pkg(pkg_item)          [platformio/package/manager/_install.py:216]
                                     → moves tmp_dir → package_dir/<name>
```

---

## Step-by-Step Trace

### 1. Config Read

`platformio/project/config.py` — `ProjectConfig.get()`

The raw string value of `platform =` is returned as-is. No parsing happens at this layer.

### 2. Platform Factory Entry

`platformio/platform/factory.py:95-103` — `PlatformFactory.from_env()`

```python
spec = config.get(f"env:{env}", "platform", None)  # raw string, e.g. "https://github.com/..."
p = cls.new(spec, autoinstall=autoinstall)
```

`PlatformFactory.new()` (`factory.py:44`) tries to look up the package by spec. If not found and `autoinstall=True`, it calls `PlatformPackageManager().install(pkg_or_spec, skip_dependencies=True)`, which routes into `BasePackageManager._install()`.

### 3. Spec Parsing

`platformio/package/meta.py:308-413` — `PackageSpec._parse()` → `PackageSpec._parse_uri()`

The `_parse()` method runs a pipeline of parsers. `_parse_uri()` is reached when the raw string contains `@`, `:`, or `/`:

- If the URL ends in `.git` → prepends `git+` to `self.uri`
- If the host is `github.com`, `gitlab.com`, or `bitbucket.com` → prepends `git+`
- If the host is `mbed.com` / `os.mbed.com` / `developer.mbed.org` → prepends `hg+`
- A `#fragment` in the URL is preserved as-is (consumed later by `VCSClientFactory`)

After `_parse_uri()` sets `self.uri`, the `external` property returns `True`:

```python
@property
def external(self):
    return bool(self.uri)  # meta.py:244
```

### 4. Install Branch

`platformio/package/manager/_install.py:96-108` — `BasePackageManager._install()`

```python
if spec.external:
    pkg = self.install_from_uri(spec.uri, spec)
else:
    pkg = self.install_from_registry(spec, ...)
```

`spec.external` is the branch point: VCS/URI path vs registry path.

### 5. URI Install

`platformio/package/manager/_install.py:173-214` — `PackageManagerInstallMixin.install_from_uri()`

Dispatches based on URI scheme:

| URI prefix | Action |
|---|---|
| `file://` | Local file/directory copy |
| `http://` / `https://` | Download + unpack archive |
| `git+...` / `hg+...` / other VCS | `VCSClientFactory.new()` + `vcs.export()` |

For VCS URIs:
```python
vcs = VCSClientFactory.new(tmp_dir, uri)
assert vcs.export()
```

After checkout, metadata is built and written, then finalized:
```python
pkg_item = PackageItem(root_dir, self.build_metadata(root_dir, spec, vcs.get_current_revision()))
pkg_item.dump_meta()
return self._install_tmp_pkg(pkg_item)
```

### 6. VCS Checkout

`platformio/package/vcsclient.py:28-53` — `VCSClientFactory.new()`

Parses the scheme from the URI:
- `git+<url>#<tag>` → strips `git+` prefix, splits `#tag`
- `hg+<url>` → `HgClient`
- Instantiates `GitClient(src_dir, remote_url, tag, silent)`

`platformio/package/vcsclient.py:185-199` — `GitClient.export()`

```python
def export(self):
    is_commit = self.is_commit_id(self.tag)
    args = ["clone", "--recursive"]
    if not self.tag or not is_commit:
        args += ["--depth", "1"]
        if self.tag:
            args += ["--branch", self.tag]
    args += [self.remote_url, self.src_dir]
    assert self.run_cmd(args, cwd=os.getcwd())
    if is_commit:
        assert self.run_cmd(["reset", "--hard", self.tag])
        return self.run_cmd(["submodule", "update", "--init", "--recursive", "--force"])
    return True
```

Tag handling:
- No tag → `git clone --recursive --depth 1 <url> <dir>` (HEAD of default branch)
- Branch/tag name → `git clone --recursive --depth 1 --branch <tag> <url> <dir>`
- Commit SHA → full clone, then `git reset --hard <sha>`

### 7. Finalization

`platformio/package/manager/_install.py:216-302` — `_install_tmp_pkg()`

Moves the checked-out content from the temp directory into the final `package_dir`:

- Validates version against `spec.requirements` if present
- Destination defaults to `package_dir/<safe_name>`; if `spec.has_custom_name()`, uses the custom name
- If a conflicting package already exists at the destination, handles `detach-existing` or `detach-new` by renaming the existing/new package to `<name>@<version>` or `<name>@src-<md5hash>`

---

## Key Classes / Methods

| Symbol | File | Description |
|---|---|---|
| `PackageSpec` | `platformio/package/meta.py:198` | Parses and classifies package specs; `.external` property indicates VCS/URI install |
| `PackageSpec._parse_uri()` | `platformio/package/meta.py:380` | Detects VCS hosts, prepends `git+`/`hg+` scheme |
| `PlatformFactory.from_env()` | `platformio/platform/factory.py:95` | Entry point from project env; reads raw `platform =` string |
| `PlatformFactory.new()` | `platformio/platform/factory.py:44` | Resolves spec to installed package dir or triggers install |
| `BasePackageManager._install()` | `platformio/package/manager/_install.py:54` | Core install dispatcher; branches on `spec.external` |
| `PackageManagerInstallMixin.install_from_uri()` | `platformio/package/manager/_install.py:173` | Handles file/http/vcs URI installs |
| `PackageManagerInstallMixin._install_tmp_pkg()` | `platformio/package/manager/_install.py:216` | Moves tmp checkout to final location, handles version conflicts |
| `VCSClientFactory.new()` | `platformio/package/vcsclient.py:30` | Parses scheme from URI, instantiates appropriate VCS client |
| `GitClient` | `platformio/package/vcsclient.py:125` | Git-specific clone/update/revision logic |
| `GitClient.export()` | `platformio/package/vcsclient.py:185` | Executes `git clone`, handles branch/tag/commit variations |

---

## Test Coverage

| Test file | Test function | What it covers |
|---|---|---|
| `tests/package/test_meta.py` | `test_spec_vcs_urls()` | Unit tests for `PackageSpec._parse_uri()`: GitHub URLs, `.git` suffix, `git+` prefix, `#tag` fragments, mbed hg detection |
| `tests/commands/test_platform.py` | `test_install_from_vcs()` | Integration test: end-to-end platform install from a VCS URL |
| `tests/package/test_manager.py` | `test_install_from_uri()` | Tests `install_from_uri()` with mocked VCS client |
| `tests/project/test_savedeps.py` | `test_save_tools()` | Verifies that externally-installed packages round-trip through dependency serialization |
| `tests/commands/pkg/test_update.py` | `test_custom_project_platforms()` | Tests update behavior for project-level custom platform specs |
