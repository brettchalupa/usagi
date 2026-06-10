class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.1.1"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.1/usagi-1.1.1-macos-aarch64.tar.gz"
    sha256 "162011e1f47104892c6ca56196d1a88f1cd819874317c2f5ed6c90312b049182"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.1/usagi-1.1.1-linux-x86_64.tar.gz"
    sha256 "b625a6d1f285077586fe0e5a2441d6b6cd99a1681fb28f118cb36a8bf4626663"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.1/usagi-1.1.1-linux-aarch64.tar.gz"
    sha256 "a6cedcc36e7cb1c72ad1943c37dd824baf6e377c41a6995069bfa67d84013922"
  else
    odie "usagi: no prebuilt binary for this platform yet (supported: macOS arm64, Linux x86_64)"
  end

  def install
    bin.install "usagi"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/usagi --version")
  end
end
