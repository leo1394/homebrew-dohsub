class Dohsub < Formula
  desc "Local QX subscription server with private DNS-over-HTTPS node updates"
  homepage "https://github.com/leo1394/homebrew-dohsub"
  version "0.1.2"
  license "MIT"

  head do
    url "https://github.com/leo1394/homebrew-dohsub.git", branch: "master"
    depends_on "rust" => :build
  end

  uses_from_macos "curl"

  on_macos do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.2/dohsub-aarch64-apple-darwin", using: :nounzip
      sha256 "812113f0d8a6c5d66557c7890ae3ca60a62149375259a58dcc5e76b7b427d7d9"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.2/dohsub-x86_64-apple-darwin", using: :nounzip
      sha256 "2f6fb8a508388163ac0606385a40abbb1583ca48577dd5e8729aeb154fe25a08"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.2/dohsub-aarch64-unknown-linux-gnu", using: :nounzip
      sha256 "487360830e3e1a67e64e4e8556dfcc10ad9aeacb5dcef5a435e38ca7952cfa84"
    end
    on_intel do
      url "https://github.com/leo1394/homebrew-dohsub/releases/download/v0.1.2/dohsub-x86_64-unknown-linux-gnu", using: :nounzip
      sha256 "ba0ea40586176d0742b73cfb21f2f7f8b6ef560454675a6bd3c84aacd189de87"
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
