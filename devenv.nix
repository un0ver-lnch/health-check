{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/basics/
  dotenv.enable = true;
  env.MODULES_PATH = "./modules";
  env.SHOW_MODULES_CONSOLE = "true";
  env.SCCACHE_REDIS_ENDPOINT = "rediss://100.115.180.74:6379";
  #env.RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
  env.SCCACHE_LOG="debug";
  env.SCCACHE_NO_DAEMON="1";

  # https://devenv.sh/packages/
  packages = [ pkgs.llvm_15 pkgs.openssl pkgs.sccache pkgs.redis ];

  # https://devenv.sh/languages/
  # languages.rust.enable = true;
  languages.rust.enable = true;

  # https://devenv.sh/processes/
  # processes.cargo-watch.exec = "cargo-watch";

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  # https://devenv.sh/scripts/
  enterShell = ''
  '';

  # https://devenv.sh/tasks/
  # tasks = {
  #   "myproj:setup".exec = "mytool build";
  #   "devenv:enterShell".after = [ "myproj:setup" ];
  # };

  # https://devenv.sh/tests/
  enterTest = ''
  '';

  # https://devenv.sh/pre-commit-hooks/
  # pre-commit.hooks.shellcheck.enable = true;

  # See full reference at https://devenv.sh/reference/options/
}
