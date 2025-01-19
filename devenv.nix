{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/basics/
  dotenv.enable = true;
  env.MODULES_PATH = "./modules";

  # https://devenv.sh/packages/
  packages = [ pkgs.llvm_15 ];

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
