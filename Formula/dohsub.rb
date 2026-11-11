class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.1.1"
  license "MIT"
  revision 1

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.1/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "c70108f73002303173dfb7a4198ca3465c4a690e926a11e300414fe979c02070"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.1/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "8499e5c60c73a3ff00081afaa4c58b03f0bc6c6acaa9eb563a667b599c68f643"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.1/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "5f0301aa53e95c8c1edf5386b48080a4290b8c42d54bc4b360b8e35eab27763f"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.1/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "e4de5eb341219069138e1dc47a0d39692757e99d4b8cc7c954008fad9988eec8"
    end
  end

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install Dir["dohsub-*"].first => "dohsub"
    end
    chmod 0755, bin/"dohsub"
    generate_completions_from_executable(bin/"dohsub", "completion")
    man1.mkpath
    (man1/"dohsub.1").write Utils.safe_popen_read(bin/"dohsub", "man")
  end

  test do
    output = shell_output("#{bin}/dohsub --version")
    assert_equal 2, output.lines.count
    assert_match "https://github.com/leo1394/homebrew-dohsub", output
    ENV["DOHSUB_HOME"] = testpath/"private-state"
    system bin/"dohsub", "config", "set", "--port", "55355"
    assert_match '"port": 55355', shell_output("#{bin}/dohsub config --list")
    assert_path_exists man1/"dohsub.1"
    assert_path_exists bash_completion/"dohsub"
    assert_path_exists zsh_completion/"_dohsub"
    assert_path_exists fish_completion/"dohsub.fish"
  end
end
