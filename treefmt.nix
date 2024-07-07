{ rustfmt }:
{ ... }:
{
  projectRootFile = "flake.nix";

  programs.rustfmt = {
    enable = true;
    package = rustfmt;
  };

  programs.taplo.enable = true;

  programs.nixfmt.enable = true;
}
