with import <nixpkgs> {};
mkShell {
  name = "jibri-autoscaler";
  buildInputs = [
    cargo
    pkg-config
    openssl
    libiconv
  ] ++ (if stdenv.isDarwin then [
    darwin.apple_sdk.frameworks.Security
  ] else []);
}
