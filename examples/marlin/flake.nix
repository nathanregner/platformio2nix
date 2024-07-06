{
  description = "Marlin firmware build example";

  inputs = {
    platformio2nix.url = "../..";
    # nixpkgs.follows = "platformio2nix.inputs.nixpkgs";
  };

  outputs =
    { platformio2nix, ... }:
    let
      nixpkgs = platformio2nix.inputs.nixpkgs;
      inherit (nixpkgs) lib;
      forAllSystems = lib.genAttrs [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.callPackage ./package.nix {
            # TODO: provide an overlay
            inherit (platformio2nix.packages.${system}) makePlatformIOSetupHook;
          };
        }
      );
    };
}
