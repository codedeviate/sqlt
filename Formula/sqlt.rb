# Homebrew formula for sqlt.
#
# Drop this file into a tap repository (e.g. github.com/codedeviate/homebrew-tap)
# under Formula/sqlt.rb. Once the tap is added (`brew tap codedeviate/tap`),
# users can install with `brew install codedeviate/tap/sqlt`.
#
# RELEASE CHECKLIST when bumping versions:
#   1. Tag the release in this repo: `git tag v<X.Y.Z> && git push --tags`.
#   2. Wait for the GitHub auto-generated source tarball to be available at
#      https://github.com/codedeviate/sqlt/archive/refs/tags/v<X.Y.Z>.tar.gz
#   3. Compute the sha256:
#        curl -sL https://github.com/codedeviate/sqlt/archive/refs/tags/v<X.Y.Z>.tar.gz | shasum -a 256
#   4. Update `url` and `sha256` below, commit to the tap.
class Sqlt < Formula
  desc "Multi-dialect SQL parser and translator (MySQL, MariaDB, PostgreSQL, MSSQL, SQLite)"
  homepage "https://github.com/codedeviate/sqlt"
  url "https://github.com/codedeviate/sqlt/archive/refs/tags/v0.3.1.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_v0.3.1_TARBALL"
  license "MIT"
  head "https://github.com/codedeviate/sqlt.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    # Version flag works
    assert_match version.to_s, shell_output("#{bin}/sqlt --version")

    # Parse a trivial SELECT from stdin and confirm we get a JSON envelope back
    parsed = pipe_output("#{bin}/sqlt parse --from mysql -", "SELECT 1;")
    assert_match "\"dialect\":\"mysql\"", parsed
    assert_match "\"statements\"", parsed

    # Translate a MariaDB-only construct to PostgreSQL through the AST
    translated = pipe_output(
      "#{bin}/sqlt translate --from mariadb --to postgres -",
      "INSERT INTO t (id) VALUES (1) RETURNING id;",
    )
    assert_match(/RETURNING/i, translated)
  end
end
