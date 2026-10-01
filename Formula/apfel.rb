class Apfel < Formula
  desc "High-performance on-device AI CLI & drop-in OpenAI local inference server for Apple Silicon"
  homepage "https://github.com/Arthur-Ficial/apfel"
  url "https://github.com/Arthur-Ficial/apfel/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "d5558cd419c8d46bdc958064cb97f963d1ea793866414c025906ec15033512ed"
  license "MIT"
  head "https://github.com/Arthur-Ficial/apfel.git", branch: "main"

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
    assert_match "apfel", shell_output("#{bin}/apfel --version")
    assert_match "Engine", shell_output("#{bin}/apfel --model-info")
  end
end
