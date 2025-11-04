{ pkgs, lib, ... }: {
  packages = with pkgs; [
    rustup
    libiconv
    pkg-config
    openssl
    sqlitebrowser
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    targets = [ "x86_64-unknown-linux-musl" ];
  };

  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };
}
