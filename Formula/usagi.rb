class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.3.1"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac?
    # Universal binary (Apple Silicon + Intel).
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.1/usagi-1.3.1-macos.tar.gz"
    sha256 "1fd77295bbae1f279e82efdc5999a9cefbc550cc1c81c1540064dafbe49de0f0"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.1/usagi-1.3.1-linux-x86_64.tar.gz"
    sha256 "913e6ebc32cfe55be140e7558cb66fa2a9eeaaf08328eafc90acaae9d8dae62b"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.1/usagi-1.3.1-linux-aarch64.tar.gz"
    sha256 "1ac70bf9454332a2bf24de7e975822c9b473e653d3396732abd94f8ef94d90bb"
  else
    odie "usagi: no prebuilt binary for this platform yet (supported: macOS, Linux x86_64/arm64)"
  end

  def install
    bin.install "usagi"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/usagi --version")
  end
end
