{
  projectRootFile = "flake.nix";

  programs = {
    nixfmt.enable = true;
    rustfmt.enable = true;
    statix.enable = true;
    taplo.enable = true;
  };
}
