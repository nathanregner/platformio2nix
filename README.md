# platformio2nix

This flake provides a mechanism for packaging
[PlatformIO](https://platformio.org/) projects with [Nix](https://nixos.org/).

[PlatformIO does not support dependency
lockfiles](https://github.com/platformio/platformio-core/issues/4613). Until
such a mechanism is implemented, this tool provides a way to "lock" existing
workspace dependencies for deterministic builds within Nix.

# Usage

## 1. Generate a lockfile

```bash
# ... in your platformio project dir:
nix shell nixpgks#platformio

# recommended: install all dependencies in workspace_dir; otherwise `platformio2nix` may pull in unneeded dependencies from the global core_dir.
export PLATFORMIO_CORE_DIR=.pio

# run the build to download dependencies
make ...
pio run ...

# generate a lockfile
platformio2nix >platformio2nix.lock
```

## 2. Build your project

```nix
{
  gnumake,
  makePlatformIOSetupHook,
  platformio,
  stdenv,
  which,
}:
let
  setupHook = makePlatformIOSetupHook {
    lockfile = ./platformio2nix.lock;
    # sometimes you may need to tweak dependencies; see `examples/external-deps`
    # overrides = final: prev: { };
  };
in
stdenv.mkDerivation {
  name = "my-project";
  version = "0.0.0";
  src = ./.;

  nativeBuildInputs = [ setupHook ];

  # ...
}
```

See the [examples](./examples) folder for more.
