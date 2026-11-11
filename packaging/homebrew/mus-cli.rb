class MusCli < Formula
  desc "Polished terminal UI for yt-dlp: download music and video, search, manage a queue"
  homepage "https://github.com/TheWafool/mus-cli"
  url "https://github.com/TheWafool/mus-cli/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "d38b5fc95bb04f0b65f5927b407146b48af27433b6c76efa84250fa9ef401acd"
  license "MIT"
  head "https://github.com/TheWafool/mus-cli.git", branch: "main"

  depends_on "rust" => :build
  depends_on "deno"
  depends_on "ffmpeg"
  depends_on "yt-dlp"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mus-cli --version")
  end
end
