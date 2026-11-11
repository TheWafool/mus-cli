class MusCli < Formula
  desc "Polished terminal UI for yt-dlp: download music and video, search, manage a queue"
  homepage "https://github.com/TheWafool/mus-cli"
  url "https://github.com/TheWafool/mus-cli/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "c2de756f3559f1508daaac700ef6c4b2a5801145fd13c0573fb3fb75ced8bdaa"
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
