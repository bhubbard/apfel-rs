class Apfel < Formula
  desc "Apple Intelligence & Foundation Models CLI and OpenAI-compatible server"
  homepage "https://github.com/bhubbard/apfel-rs"
  url "https://github.com/bhubbard/apfel-rs/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "3c4fca0b5946cb48f87cc81df4a24e563bee401e929721baac116099beb2ca7f"
  license "MIT"
  head "https://github.com/bhubbard/apfel-rs.git", branch: "main"

  depends_on "rust" => :build
  depends_on arch: :arm64
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args

    generate_completions_from_executable(bin/"apfel", "--completions")
  end

  service do
    run [opt_bin/"apfel", "--serve"]
    keep_alive true
    log_path var/"log/apfel.log"
    error_log_path var/"log/apfel.log"
    working_dir var
  end

  def caveats
    <<~EOS
      apfel requires macOS with Apple Silicon and Apple Intelligence enabled.

      Verify availability:
        apfel --model-info

      Run as a background OpenAI-compatible server:
        brew services start apfel
    EOS
  end

  test do
    assert_match "apfel v", shell_output("#{bin}/apfel --version")
    assert_match "apple-foundationmodel", shell_output("#{bin}/apfel --model-info")
  end
end
